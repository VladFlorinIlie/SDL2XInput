use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};

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

unsafe extern "C" {
    fn NewUSBServer(config: *const USBServerConfig, outHandle: *mut USBServerHandle, logCallback: ViiperLogCallback) -> u8;
    fn CloseUSBServer(handle: USBServerHandle) -> u8;
    fn CreateUSBBus(handle: USBServerHandle, busID: *mut u32) -> u8;
    fn RemoveUSBBus(handle: USBServerHandle, busID: u32) -> u8;
    fn CreateXbox360Device(serverHandle: USBServerHandle, outDeviceHandle: *mut Xbox360DeviceHandle, busID: u32, autoAttachLocalhost: u8, idVendor: u16, idProduct: u16, xinputSubType: u8) -> u8;
    fn SetXbox360DeviceState(handle: Xbox360DeviceHandle, state: Xbox360DeviceState) -> u8;
    fn SetXbox360RumbleCallback(handle: Xbox360DeviceHandle, cb: Xbox360RumbleCallback) -> u8;
    fn RemoveXbox360Device(handle: Xbox360DeviceHandle) -> u8;
}

// --- Callbacks ---

unsafe extern "C" fn viiper_logger(level: i32, msg: *const std::ffi::c_char) {
    if msg.is_null() { return; }
    // SAFETY: We assume msg is a valid null-terminated C string pointing to memory that lasts for the duration of this call
    let s = unsafe { std::ffi::CStr::from_ptr(msg).to_string_lossy() };
    match level {
        8 => tracing::error!("libviiper: {}", s),
        4 => tracing::warn!("libviiper: {}", s),
        0 => tracing::info!("libviiper: {}", s),
        -4 => tracing::debug!("libviiper: {}", s),
        _ => {}
    }
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
        unsafe {
            let c_addr = addr.map(|s| std::ffi::CString::new(s).ok()).flatten();
            let config = USBServerConfig {
                addr: c_addr.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                connection_timeout_ms: 30_000,
                device_handler_connect_timeout_ms: 5_000,
                write_batch_flush_interval_ms: 1,
            };
            let mut server_handle = 0;
            if NewUSBServer(&config, &mut server_handle, Some(viiper_logger)) == 0 {
                bail!("Failed to start USB server");
            }
            Ok(Self { server_handle })
        }
    }

    pub fn create_virtual_xbox_controller(&self) -> Result<(Xbox360DeviceHandle, u32, mpsc::Receiver<(u8, u8)>)> {
        unsafe {
            let mut bus_id = 0;
            if CreateUSBBus(self.server_handle, &mut bus_id) == 0 {
                bail!("Failed to create USB bus");
            }

            let mut handle = 0;
            if CreateXbox360Device(self.server_handle, &mut handle, bus_id, 1, 0, 0, 1) == 0 {
                RemoveUSBBus(self.server_handle, bus_id);
                bail!("Failed to create virtual Xbox controller");
            }
            let (tx, rx) = mpsc::channel();
            rumble_senders().lock().unwrap().insert(handle, tx);
            if SetXbox360RumbleCallback(handle, Some(rumble_callback)) == 0 {
                tracing::warn!("Failed to set rumble callback for handle {}", handle);
            }
            Ok((handle, bus_id, rx))
        }
    }

    pub fn set_xbox360_state(&self, handle: Xbox360DeviceHandle, state: Xbox360DeviceState) -> Result<()> {
        unsafe {
            if SetXbox360DeviceState(handle, state) == 0 {
                bail!("Failed to set device state");
            }
        }
        Ok(())
    }

    pub fn remove_virtual_xbox_controller(&self, handle: Xbox360DeviceHandle, bus_id: u32) -> Result<()> {
        unsafe {
            rumble_senders().lock().unwrap().remove(&handle);
            if RemoveXbox360Device(handle) == 0 {
                tracing::warn!("Failed to remove virtual device (it may already be gone)");
            }
            if RemoveUSBBus(self.server_handle, bus_id) == 0 {
                tracing::warn!("Failed to remove USB bus {}", bus_id);
            }
        }
        Ok(())
    }
}

impl Drop for ViiperManager {
    fn drop(&mut self) {
        unsafe { CloseUSBServer(self.server_handle); }
    }
}
