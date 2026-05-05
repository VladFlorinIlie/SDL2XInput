use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use anyhow::Result;
use sdl3::GamepadSubsystem;
use sdl3::sys::joystick::SDL_JoystickID;
use sdl3::EventPump;
use tray_item::{IconSource, TrayItem};

use crate::Args;
use crate::config::Config;
use crate::viiper_bridge::ViiperManager;
use crate::session::ActiveSession;

/// Microsoft Xbox 360 Controller VID/PID.
/// VIIPER's virtual Xbox 360 device presents with these IDs (required for xusb22.sys binding).
/// Pre-blocked by default so the app ignores its own virtual controllers.
const XBOX360_VID: u16 = 0x045E;
const XBOX360_PID: u16 = 0x028E;

pub struct App {
    args: Args,
    config: Config,
    tick_duration: Duration,
    blocked_devices: HashSet<(u16, u16)>,
    _sdl_context: sdl3::Sdl,
    gamepad_subsystem: GamepadSubsystem,
    event_pump: EventPump,
    viiper_manager: ViiperManager,
    active_sessions: HashMap<u32, ActiveSession>,
    quit_flag: Arc<AtomicBool>,
    _tray: Option<TrayItem>,
}

impl App {
    pub fn new(args: Args) -> Result<Self> {
        let config = Config::load(args.config.as_deref())?;

        // Seed the blocklist with the Xbox 360 VID/PID by default.
        // The virtual controller VIIPER creates is indistinguishable from a real one.
        // Pass --empty-device-filter to opt out (e.g. when HidHide is already masking hardware).
        let mut blocked_devices: HashSet<(u16, u16)> = if args.empty_device_filter {
            HashSet::new()
        } else {
            HashSet::from([(XBOX360_VID, XBOX360_PID)])
        };
        blocked_devices.extend(Self::parse_filter_devices(&args.filter_devices));

        tracing::info!("Initializing SDL3...");
        let sdl_context = sdl3::init()?;
        let gamepad_subsystem = sdl_context.gamepad()?;
        let event_pump = sdl_context.event_pump()?;

        tracing::info!("Starting native VIIPER USBIP Server...");
        let viiper_manager = ViiperManager::connect(args.usb_server_addr.as_deref())?;

        let tick_duration = Duration::from_micros(1_000_000 / args.polling_rate as u64);
        tracing::info!("Polling rate: {} Hz (tick: {:?})", args.polling_rate, tick_duration);
        tracing::info!("Hardware deadzone: {}", args.deadzone);
        tracing::info!("Blocking {} VID:PID pair(s):", blocked_devices.len());
        for (vid, pid) in &blocked_devices {
            tracing::info!("  {:04X}:{:04X}", vid, pid);
        }

        let quit_flag = Arc::new(AtomicBool::new(false));
        let _tray = if !args.no_tray {
            create_tray(&quit_flag)
        } else {
            None
        };

        Ok(Self {
            args,
            config,
            tick_duration,
            blocked_devices,
            _sdl_context: sdl_context,
            gamepad_subsystem,
            event_pump,
            viiper_manager,
            active_sessions: HashMap::new(),
            quit_flag,
            _tray,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        tracing::info!("Ready! Forwarding SDL3 inputs to VIIPER. Press Ctrl+C to exit.");

        loop {
            if self.quit_flag.load(Ordering::SeqCst) {
                tracing::info!("Quit signal received. Exiting...");
                return Ok(());
            }

            while let Some(event) = self.event_pump.poll_event() {
                match event {
                    sdl3::event::Event::ControllerDeviceAdded   { which, .. } => self.handle_device_added(which),
                    sdl3::event::Event::ControllerDeviceRemoved { which, .. } => self.handle_device_removed(which),
                    sdl3::event::Event::Quit                    { .. }        => {
                        tracing::info!("Exiting...");
                        return Ok(());
                    }
                    _ => {}
                }
            }

            self.tick_sessions();
            std::thread::sleep(self.tick_duration);
        }
    }

    fn handle_device_added(&mut self, which: u32) {
        let jid = SDL_JoystickID(which);
        let vid = self.gamepad_subsystem.vendor_for_id(jid).unwrap_or(0);
        let pid = self.gamepad_subsystem.product_for_id(jid).unwrap_or(0);

        if self.blocked_devices.contains(&(vid, pid)) {
            let name = self.gamepad_subsystem.name_for_id(jid).unwrap_or_else(|_| "unknown".to_string());
            tracing::info!("Ignoring filtered device ({:04X}:{:04X}) - {}", vid, pid, name);
            return;
        }

        if self.active_sessions.len() >= self.args.max_controllers {
            tracing::info!(
                "Ignoring additional controller (limit {} reached): ID {} ({:04X}:{:04X})",
                self.args.max_controllers, which, vid, pid
            );
            return;
        }

        if self.active_sessions.contains_key(&which) {
            return;
        }

        match self.gamepad_subsystem.open(jid) {
            Ok(gp) => {
                tracing::info!("Opened physical gamepad: {}", gp.name().unwrap_or_else(|| "unknown".to_string()));
                match self.viiper_manager.create_virtual_xbox_controller() {
                    Ok((dev_handle, bus_id, rumble_rx)) => {
                        self.active_sessions.insert(which, ActiveSession::new(gp, dev_handle, bus_id, rumble_rx));
                    }
                    Err(e) => tracing::error!("Failed to create virtual device: {}", e),
                }
            }
            Err(e) => tracing::error!("Failed to open gamepad: {}", e),
        }
    }

    fn handle_device_removed(&mut self, which: u32) {
        if let Some(session) = self.active_sessions.remove(&which) {
            if let Err(e) = self.viiper_manager.remove_virtual_xbox_controller(session.dev_handle, session.bus_id) {
                tracing::error!("Failed to remove virtual device: {}", e);
            }
            tracing::info!("Gamepad removed: ID {}", which);
        }
    }

    fn tick_sessions(&mut self) {
        for session in self.active_sessions.values_mut() {
            session.apply_rumble();
            session.update_and_send(&self.config, self.args.deadzone, &self.viiper_manager);
        }
    }

    fn parse_filter_devices(raw: &[String]) -> HashSet<(u16, u16)> {
        raw.iter()
            .filter_map(|s| {
                let (l, r) = s.split_once(':')?;
                let vid = u16::from_str_radix(l.trim_start_matches("0x"), 16).ok()?;
                let pid = u16::from_str_radix(r.trim_start_matches("0x"), 16).ok()?;
                Some((vid, pid))
            })
            .collect()
    }
}

/// Loads the embedded .ico and creates the system tray icon.
/// Returns `None` (with a warning) if the tray cannot be initialised.
fn create_tray(quit_flag: &Arc<AtomicBool>) -> Option<TrayItem> {
    let hicon = unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::LoadImageW(
            windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
            1 as *const u16, // Resource ID 1 (compiled via build.rs)
            windows_sys::Win32::UI::WindowsAndMessaging::IMAGE_ICON,
            0, 0,
            windows_sys::Win32::UI::WindowsAndMessaging::LR_DEFAULTCOLOR,
        )
    };

    let icon_source = if hicon != std::ptr::null_mut() {
        IconSource::RawIcon(hicon as _)
    } else {
        tracing::warn!("Failed to load embedded icon from resources; tray will have no icon");
        IconSource::Resource("")
    };

    match TrayItem::new("SDL2XInput", icon_source) {
        Ok(mut tray) => {
            tray.add_label("SDL2XInput Running").unwrap_or_default();
            let flag = quit_flag.clone();
            tray.add_menu_item("Quit", move || flag.store(true, Ordering::SeqCst)).unwrap_or_default();
            Some(tray)
        }
        Err(e) => {
            tracing::warn!("Failed to create tray icon: {}", e);
            None
        }
    }
}
