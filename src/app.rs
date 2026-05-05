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
/// We pre-block this pair by default so the app ignores its own virtual controllers.
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
    _tray: Option<TrayItem>, // Keep the tray item alive
}

impl App {
    pub async fn new(args: Args) -> Result<Self> {
        let config = Config::load(args.config.as_deref())?;

        // Seed the blocklist with the Xbox 360 VID/PID by default since the virtual
        // controller VIIPER creates is indistinguishable from a real one by name or VID/PID.
        // Pass --no-default-filter to opt out (e.g. when HidHide is already masking hardware).
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

        tracing::info!("Connecting to Viiper at {}...", args.viiper_address);
        let viiper_manager = ViiperManager::connect(&args.viiper_address).await?;
        
        let tick_duration = Duration::from_micros(1_000_000 / args.polling_rate as u64);
        tracing::info!("Polling rate: {} Hz (tick: {:?})", args.polling_rate, tick_duration);

        tracing::info!("Blocking {} VID:PID pair(s):", blocked_devices.len());
        for (vid, pid) in &blocked_devices {
            tracing::info!("  {:04X}:{:04X}", vid, pid);
        }

        let quit_flag = Arc::new(AtomicBool::new(false));
        let mut _tray = None;

        if !args.no_tray {
            let mut tray = TrayItem::new("SDL2XInput", IconSource::Resource("icon")).unwrap();
            tray.add_label("SDL2XInput Running").unwrap();
            
            let q_flag = quit_flag.clone();
            tray.add_menu_item("Quit", move || {
                q_flag.store(true, Ordering::SeqCst);
            }).unwrap();

            _tray = Some(tray);
        }
        
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

    /// Parse "VID:PID" hex strings (e.g. "045E:028E") into (u16, u16) pairs.
    fn parse_filter_devices(raw: &[String]) -> HashSet<(u16, u16)> {
        raw.iter()
            .filter_map(|s| {
                let parts: Vec<&str> = s.splitn(2, ':').collect();
                if parts.len() != 2 {
                    tracing::warn!("Warning: ignoring invalid --filter-device value '{}' (expected VID:PID hex)", s);
                    return None;
                }
                let vid = u16::from_str_radix(parts[0].trim_start_matches("0x"), 16).ok()?;
                let pid = u16::from_str_radix(parts[1].trim_start_matches("0x"), 16).ok()?;
                Some((vid, pid))
            })
            .collect()
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Ready! Reading SDL3 inputs and forwarding to Viiper...");
        tracing::info!("Press Ctrl+C to exit.");

        loop {
            if self.quit_flag.load(Ordering::SeqCst) {
                tracing::info!("Quit signal received from tray. Exiting...");
                return Ok(());
            }

            while let Some(event) = self.event_pump.poll_event() {
                match event {
                    sdl3::event::Event::ControllerDeviceAdded { which, .. } => {
                        self.handle_device_added(which).await;
                    }
                    sdl3::event::Event::ControllerDeviceRemoved { which, .. } => {
                        self.handle_device_removed(which);
                    }
                    sdl3::event::Event::Quit { .. } => {
                        tracing::info!("Exiting...");
                        return Ok(());
                    }
                    _ => {}
                }
            }

            self.tick_sessions().await;
            tokio::time::sleep(self.tick_duration).await;
        }
    }

    async fn handle_device_added(&mut self, which: u32) {
        let jid = SDL_JoystickID(which);

        // Check VID/PID against the blocklist (always includes our own virtual VID/PID).
        let vid = self.gamepad_subsystem.vendor_for_id(jid).unwrap_or(0);
        let pid = self.gamepad_subsystem.product_for_id(jid).unwrap_or(0);
        if self.blocked_devices.contains(&(vid, pid)) {
            let name = self.gamepad_subsystem.name_for_id(jid)
                .map(|n| n)
                .unwrap_or_else(|_| "unknown".to_string());
            tracing::info!("Ignoring filtered device ({:04X}:{:04X}) - {}", vid, pid, name);
            return;
        }

        if self.active_sessions.len() >= self.args.max_controllers {
            tracing::info!(
                "Ignoring additional controller (Max limit {} reached): ID {} ({:04X}:{:04X})",
                self.args.max_controllers, which, vid, pid
            );
            if let Ok(path) = self.gamepad_subsystem.path_for_id(jid) {
                tracing::info!("  Device Path: {}", path);
            }
            return;
        }

        if !self.active_sessions.contains_key(&which) {
            match self.gamepad_subsystem.open(jid) {
                Ok(gp) => {
                    tracing::info!("Opened physical gamepad: {:?}", gp.name());
                    match self.viiper_manager.create_virtual_xbox_controller("Virtual Steam Controller").await {
                        Ok((dev_stream, rumble_rx)) => {
                            self.active_sessions.insert(which, ActiveSession {
                                gamepad: gp,
                                dev_stream,
                                rumble_rx,
                            });
                        }
                        Err(e) => tracing::error!("Failed to create virtual device: {}", e),
                    }
                }
                Err(e) => tracing::error!("Failed to open gamepad: {}", e),
            }
        }
    }

    fn handle_device_removed(&mut self, which: u32) {
        if self.active_sessions.remove(&which).is_some() {
            tracing::info!("Gamepad Removed: ID {} (Virtual controller destroyed)", which);
        }
    }

    async fn tick_sessions(&mut self) {
        for session in self.active_sessions.values_mut() {
            session.apply_rumble().await;
            session.update_and_send(&self.config).await;
        }
    }
}
