use std::collections::HashMap;
use std::time::Duration;
use anyhow::Result;
use sdl3::GamepadSubsystem;
use sdl3::sys::joystick::SDL_JoystickID;
use sdl3::EventPump;

use crate::Args;
use crate::config::Config;
use crate::viiper_bridge::ViiperManager;
use crate::session::ActiveSession;

pub struct App {
    args: Args,
    config: Config,
    tick_duration: Duration,
    _sdl_context: sdl3::Sdl,
    gamepad_subsystem: GamepadSubsystem,
    event_pump: EventPump,
    viiper_manager: ViiperManager,
    active_sessions: HashMap<u32, ActiveSession>,
}

impl App {
    pub async fn new(args: Args) -> Result<Self> {
        let config = Config::load(args.config.as_deref())?;

        println!("Initializing SDL3...");
        let sdl_context = sdl3::init()?;
        let gamepad_subsystem = sdl_context.gamepad()?;
        let event_pump = sdl_context.event_pump()?;

        println!("Connecting to Viiper at {}...", args.viiper_address);
        let viiper_manager = ViiperManager::connect(&args.viiper_address).await?;
        
        let tick_duration = Duration::from_micros(1_000_000 / args.polling_rate as u64);
        println!("Polling rate: {} Hz (tick: {:?})", args.polling_rate, tick_duration);
        
        Ok(Self {
            args,
            config,
            tick_duration,
            _sdl_context: sdl_context,
            gamepad_subsystem,
            event_pump,
            viiper_manager,
            active_sessions: HashMap::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("Ready! Reading SDL3 inputs and forwarding to Viiper...");
        println!("Press Ctrl+C to exit.");

        loop {
            while let Some(event) = self.event_pump.poll_event() {
                match event {
                    sdl3::event::Event::ControllerDeviceAdded { which, .. } => {
                        self.handle_device_added(which).await;
                    }
                    sdl3::event::Event::ControllerDeviceRemoved { which, .. } => {
                        self.handle_device_removed(which);
                    }
                    sdl3::event::Event::Quit { .. } => {
                        println!("Exiting...");
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

        if let Ok(name) = self.gamepad_subsystem.name_for_id(jid) {
            if name.contains("Virtual Steam Controller") || name.contains("Xbox 360") {
                println!("Ignoring Xbox 360 Controller (to prevent loop): {}", name);
                return;
            }
        }

        if self.active_sessions.len() >= self.args.max_controllers {
            println!(
                "Ignoring additional controller (Max limit {} reached): ID {}",
                self.args.max_controllers, which
            );
            return;
        }

        if !self.active_sessions.contains_key(&which) {
            match self.gamepad_subsystem.open(jid) {
                Ok(gp) => {
                    println!("Opened physical gamepad: {:?}", gp.name());
                    match self.viiper_manager.create_virtual_xbox_controller("Virtual Steam Controller").await {
                        Ok((dev_stream, rumble_rx)) => {
                            self.active_sessions.insert(which, ActiveSession {
                                gamepad: gp,
                                dev_stream,
                                rumble_rx,
                            });
                        }
                        Err(e) => println!("Failed to create virtual device: {}", e),
                    }
                }
                Err(e) => println!("Failed to open gamepad: {}", e),
            }
        }
    }

    fn handle_device_removed(&mut self, which: u32) {
        if self.active_sessions.remove(&which).is_some() {
            println!("Gamepad Removed: ID {} (Virtual controller destroyed)", which);
        }
    }

    async fn tick_sessions(&mut self) {
        for session in self.active_sessions.values_mut() {
            session.apply_rumble().await;
            session.update_and_send(&self.config).await;
        }
    }
}
