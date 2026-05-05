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

// --- Internal API Table ---

struct ViiperApi {
    new_usb_server:              unsafe extern "C" fn(*const USBServerConfig, *mut USBServerHandle, ViiperLogCallback) -> u8,
    close_usb_server:            unsafe extern "C" fn(USBServerHandle) -> u8,
    create_usb_bus:              unsafe extern "C" fn(USBServerHandle, *mut u32) -> u8,
    remove_usb_bus:              unsafe extern "C" fn(USBServerHandle, u32) -> u8,
    create_xbox360_device:       unsafe extern "C" fn(USBServerHandle, *mut Xbox360DeviceHandle, u32, u8, u16, u16, u8) -> u8,
    set_xbox360_device_state:    unsafe extern "C" fn(Xbox360DeviceHandle, Xbox360DeviceState) -> u8,
    set_xbox360_rumble_callback: unsafe extern "C" fn(Xbox360DeviceHandle, Xbox360RumbleCallback) -> u8,
    remove_xbox360_device:       unsafe extern "C" fn(Xbox360DeviceHandle) -> u8,
}

fn get_api() -> Result<&'static ViiperApi> {
    static API: OnceLock<Result<ViiperApi>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let lib = Library::new("libviiper.dll").map_err(anyhow::Error::from)?;
        let lib = Box::leak(Box::new(lib)); // Leak to prevent segfault on exit
        Ok(ViiperApi {
            new_usb_server:              *lib.get(b"NewUSBServer")?,
            close_usb_server:            *lib.get(b"CloseUSBServer")?,
            create_usb_bus:              *lib.get(b"CreateUSBBus")?,
            remove_usb_bus:              *lib.get(b"RemoveUSBBus")?,
            create_xbox360_device:       *lib.get(b"CreateXbox360Device")?,
            set_xbox360_device_state:    *lib.get(b"SetXbox360DeviceState")?,
            set_xbox360_rumble_callback: *lib.get(b"SetXbox360RumbleCallback")?,
            remove_xbox360_device:       *lib.get(b"RemoveXbox360Device")?,
        })
    }).as_ref().map_err(|e| anyhow::anyhow!("Failed to load libviiper.dll: {}", e))
}

// --- Rumble Registry ---

fn rumble_senders() -> &'static Mutex<HashMap<Xbox360DeviceHandle, mpsc::Sender<(u8, u8)>>> {
    static MAP: OnceLock<Mutex<HashMap<Xbox360DeviceHandle, mpsc::Sender<(u8, u8)>>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "C" fn rumble_callback(handle: Xbox360DeviceHandle, left: u8, right: u8) {
    if let Ok(map) = rumble_senders().lock() {
        if let Some(tx) = map.get(&handle) {
            let _ = tx.send((left, right));
        }
    }
}

// --- Manager ---

pub struct ViiperManager {
    server_handle: USBServerHandle,
}

impl ViiperManager {
    pub fn connect(addr: Option<&str>) -> Result<Self> {
        let api = get_api()?;
        unsafe {
            let c_addr = addr.map(|s| std::ffi::CString::new(s).ok()).flatten();
            let config = USBServerConfig {
                addr: c_addr.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                connection_timeout_ms: 30_000,
                device_handler_connect_timeout_ms: 5_000,
                write_batch_flush_interval_ms: 1,
            };
            let mut server_handle = 0;
            if (api.new_usb_server)(&config, &mut server_handle, None) == 0 {
                bail!("Failed to start USB server");
            }
            Ok(Self { server_handle })
        }
    }

    pub fn create_virtual_xbox_controller(&self) -> Result<(Xbox360DeviceHandle, u32, mpsc::Receiver<(u8, u8)>)> {
        let api = get_api()?;
        unsafe {
            let mut bus_id = 0;
            if (api.create_usb_bus)(self.server_handle, &mut bus_id) == 0 {
                bail!("Failed to create USB bus");
            }

            let mut handle = 0;
            if (api.create_xbox360_device)(self.server_handle, &mut handle, bus_id, 1, 0, 0, 1) == 0 {
                (api.remove_usb_bus)(self.server_handle, bus_id);
                bail!("Failed to create virtual Xbox controller");
            }
            let (tx, rx) = mpsc::channel();
            rumble_senders().lock().unwrap().insert(handle, tx);
            if (api.set_xbox360_rumble_callback)(handle, Some(rumble_callback)) == 0 {
                tracing::warn!("Failed to set rumble callback for handle {}", handle);
            }
            Ok((handle, bus_id, rx))
        }
    }

    pub fn set_xbox360_state(&self, handle: Xbox360DeviceHandle, state: Xbox360DeviceState) -> Result<()> {
        let api = get_api()?;
        unsafe {
            if (api.set_xbox360_device_state)(handle, state) == 0 {
                bail!("Failed to set device state");
            }
        }
        Ok(())
    }

    pub fn remove_virtual_xbox_controller(&self, handle: Xbox360DeviceHandle, bus_id: u32) -> Result<()> {
        let api = get_api()?;
        unsafe {
            rumble_senders().lock().unwrap().remove(&handle);
            if (api.remove_xbox360_device)(handle) == 0 {
                tracing::warn!("Failed to remove virtual device (it may already be gone)");
            }
            if (api.remove_usb_bus)(self.server_handle, bus_id) == 0 {
                tracing::warn!("Failed to remove USB bus {}", bus_id);
            }
        }
        Ok(())
    }
}

impl Drop for ViiperManager {
    fn drop(&mut self) {
        if let Ok(api) = get_api() {
            unsafe { (api.close_usb_server)(self.server_handle); }
        }
    }
}
