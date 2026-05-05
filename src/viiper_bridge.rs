use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};
use libloading::Library;

// --- C FFI types matching libviiper.h ---

#[repr(C)]
pub struct USBServerConfig {
    pub addr: *const std::ffi::c_char,
    pub connection_timeout_ms: u64,
    pub device_handler_connect_timeout_ms: u64,
    pub write_batch_flush_interval_ms: u32,
}

pub type USBServerHandle = usize;
pub type Xbox360DeviceHandle = usize;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Xbox360DeviceState {
    pub buttons: u32,
    pub lt: u8,
    pub rt: u8,
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
    pub reserved: [u8; 6],
}

type ViiperLogCallback   = Option<unsafe extern "C" fn(i32, *const std::ffi::c_char)>;
type Xbox360RumbleCallback = Option<unsafe extern "C" fn(Xbox360DeviceHandle, u8, u8)>;

// --- Global rumble sender registry ---
//
// libVIIPER's rumble callback has no userdata/context pointer, so we maintain a
// process-global map from Xbox360DeviceHandle → mpsc::Sender.  The static
// `extern "C"` function below looks up the sender by handle and forwards the
// rumble event into Rust.

fn rumble_senders() -> &'static Mutex<HashMap<Xbox360DeviceHandle, mpsc::Sender<(u8, u8)>>> {
    static MAP: OnceLock<Mutex<HashMap<Xbox360DeviceHandle, mpsc::Sender<(u8, u8)>>>> =
        OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The C-compatible rumble callback registered with libVIIPER.
///
/// SAFETY: called from libVIIPER's internal thread; the global map is protected
/// by a Mutex so concurrent access is safe.
unsafe extern "C" fn rumble_callback(handle: Xbox360DeviceHandle, left: u8, right: u8) {
    if let Ok(map) = rumble_senders().lock() {
        if let Some(tx) = map.get(&handle) {
            let _ = tx.send((left, right));
        }
    }
}

// --- Dynamic library function table ---

struct ViiperApi {
    _lib: Library, // must outlive all function pointers
    new_usb_server:              unsafe extern "C" fn(*const USBServerConfig, *mut USBServerHandle, ViiperLogCallback) -> u8,
    close_usb_server:            unsafe extern "C" fn(USBServerHandle) -> u8,
    create_usb_bus:              unsafe extern "C" fn(USBServerHandle, *mut u32) -> u8,
    create_xbox360_device:       unsafe extern "C" fn(USBServerHandle, *mut Xbox360DeviceHandle, u32, u8, u16, u16, u8) -> u8,
    set_xbox360_device_state:    unsafe extern "C" fn(Xbox360DeviceHandle, Xbox360DeviceState) -> u8,
    set_xbox360_rumble_callback: unsafe extern "C" fn(Xbox360DeviceHandle, Xbox360RumbleCallback) -> u8,
    remove_xbox360_device:       unsafe extern "C" fn(Xbox360DeviceHandle) -> u8,
}

impl ViiperApi {
    fn load() -> Result<Self> {
        // SAFETY: libviiper.dll exports a stable C ABI. The Library handle keeps
        // the DLL mapped for the entire lifetime of this struct.
        unsafe {
            let lib = Library::new("libviiper.dll")?;
            Ok(Self {
                new_usb_server:              *lib.get(b"NewUSBServer")?,
                close_usb_server:            *lib.get(b"CloseUSBServer")?,
                create_usb_bus:              *lib.get(b"CreateUSBBus")?,
                create_xbox360_device:       *lib.get(b"CreateXbox360Device")?,
                set_xbox360_device_state:    *lib.get(b"SetXbox360DeviceState")?,
                set_xbox360_rumble_callback: *lib.get(b"SetXbox360RumbleCallback")?,
                remove_xbox360_device:       *lib.get(b"RemoveXbox360Device")?,
                _lib: lib,
            })
        }
    }
}

// --- Public manager ---

pub struct ViiperManager {
    api: ViiperApi,
    server_handle: USBServerHandle,
    bus_id: u32,
}

impl ViiperManager {
    pub fn connect() -> Result<Self> {
        let api = ViiperApi::load()?;

        // SAFETY: all pointers are valid for the duration of the call.
        unsafe {
            let config = USBServerConfig {
                addr: std::ptr::null(), // use default 0.0.0.0:3241
                connection_timeout_ms: 30_000,
                device_handler_connect_timeout_ms: 5_000,
                write_batch_flush_interval_ms: 1,
            };

            let mut server_handle = 0;
            if (api.new_usb_server)(&config, &mut server_handle, None) == 0 {
                bail!("Failed to start native libVIIPER server");
            }

            let mut bus_id = 0;
            if (api.create_usb_bus)(server_handle, &mut bus_id) == 0 {
                (api.close_usb_server)(server_handle);
                bail!("Failed to create USB bus");
            }

            Ok(Self { api, server_handle, bus_id })
        }
    }

    pub fn create_virtual_xbox_controller(&mut self) -> Result<(Xbox360DeviceHandle, mpsc::Receiver<(u8, u8)>)> {
        // SAFETY: server_handle and bus_id are valid for the lifetime of ViiperManager.
        unsafe {
            let mut dev_handle = 0;
            if (self.api.create_xbox360_device)(
                self.server_handle, &mut dev_handle, self.bus_id,
                1, // autoAttachLocalhost = true
                0, 0, // idVendor / idProduct — use defaults
                1, // xinputSubType = gamepad
            ) == 0 {
                bail!("Failed to create virtual Xbox 360 controller");
            }

            // Register a sender in the global map so the static callback can reach Rust.
            let (tx, rx) = mpsc::channel();
            rumble_senders()
                .lock()
                .expect("rumble map poisoned")
                .insert(dev_handle, tx);

            if (self.api.set_xbox360_rumble_callback)(dev_handle, Some(rumble_callback)) == 0 {
                tracing::warn!("Failed to register rumble callback (rumble will not work)");
            } else {
                tracing::debug!("Rumble callback registered for device handle {}", dev_handle);
            }

            Ok((dev_handle, rx))
        }
    }

    pub fn set_xbox360_state(&self, handle: Xbox360DeviceHandle, state: Xbox360DeviceState) -> Result<()> {
        // SAFETY: handle was obtained from create_virtual_xbox_controller and is still valid.
        unsafe {
            if (self.api.set_xbox360_device_state)(handle, state) == 0 {
                bail!("Failed to update virtual Xbox 360 device state");
            }
        }
        Ok(())
    }

    pub fn remove_virtual_xbox_controller(&self, handle: Xbox360DeviceHandle) -> Result<()> {
        // SAFETY: handle was obtained from create_virtual_xbox_controller and is still valid.
        unsafe {
            if (self.api.remove_xbox360_device)(handle) == 0 {
                bail!("Failed to remove virtual Xbox 360 device");
            }
        }
        // Remove the sender from the global map so the callback stops forwarding.
        rumble_senders()
            .lock()
            .expect("rumble map poisoned")
            .remove(&handle);
        Ok(())
    }
}

impl Drop for ViiperManager {
    fn drop(&mut self) {
        // SAFETY: server_handle is valid until this point.
        unsafe {
            (self.api.close_usb_server)(self.server_handle);
        }
    }
}
