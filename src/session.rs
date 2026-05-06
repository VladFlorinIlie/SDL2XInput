use sdl3::gamepad::Gamepad;
use crate::viiper_bridge::{Xbox360DeviceState, Xbox360DeviceHandle, ViiperManager, MouseDeviceHandle, MouseDeviceState, KeyboardDeviceHandle, KeyboardDeviceState};
use std::sync::mpsc;
use crate::config::Config;
use crate::keys::Action;
use std::collections::HashMap;

pub struct ActiveSession {
    pub gamepad: Gamepad,
    pub dev_handle: Xbox360DeviceHandle,
    pub bus_id: u32,
    pub rumble_rx: mpsc::Receiver<(u8, u8)>,
    rumble_state: (u8, u8),
    last_rumble_update: Option<std::time::Instant>,

    // Mouse and Keyboard
    pub mouse_handle: Option<MouseDeviceHandle>,
    pub keyboard_handle: Option<KeyboardDeviceHandle>,
    mouse_state: MouseDeviceState,
    keyboard_state: KeyboardDeviceState,
    
    // Touchpad Absolute Tracking
    // Maps (touchpad_id, finger_id) -> (x, y)
    finger_tracking: HashMap<(i32, i32), (f32, f32)>,
}

impl ActiveSession {
    pub fn new(gamepad: Gamepad, dev_handle: Xbox360DeviceHandle, bus_id: u32, rumble_rx: mpsc::Receiver<(u8, u8)>, viiper: &ViiperManager, cfg: &Config) -> Self {
        let touchpads = gamepad.touchpads_count();
        let mouse_handle = if touchpads > 0 && cfg.mouse.enabled {
            match viiper.create_virtual_mouse(bus_id) {
                Ok(h) => {
                    tracing::info!("Spawned Virtual Mouse for gamepad with {} touchpads", touchpads);
                    Some(h)
                }
                Err(e) => {
                    tracing::warn!("Failed to spawn Virtual Mouse: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let keyboard_handle = if cfg.keyboard.enabled || cfg.mouse.enabled {
            match viiper.create_virtual_keyboard(bus_id) {
                Ok(h) => {
                    tracing::info!("Spawned Virtual Keyboard");
                    Some(h)
                }
                Err(e) => {
                    tracing::warn!("Failed to spawn Virtual Keyboard: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self { 
            gamepad, dev_handle, bus_id, rumble_rx, 
            rumble_state: (0, 0), last_rumble_update: None,
            mouse_handle, keyboard_handle,
            mouse_state: MouseDeviceState::default(),
            keyboard_state: KeyboardDeviceState::default(),
            finger_tracking: HashMap::new(),
        }
    }

    pub fn apply_rumble(&mut self) {
        let mut changed = false;
        while let Ok(val) = self.rumble_rx.try_recv() {
            if self.rumble_state != val {
                self.rumble_state = val;
                changed = true;
            }
        }
        
        let (left, right) = self.rumble_state;

        let now = std::time::Instant::now();
        let needs_refresh = if let Some(last) = self.last_rumble_update {
            now.duration_since(last).as_millis() >= 200
        } else {
            true
        };

        if changed || (needs_refresh && (left > 0 || right > 0)) {
            if left > 0 || right > 0 {
                let left_u16 = (left as u16) << 8 | left as u16;
                let right_u16 = (right as u16) << 8 | right as u16;

                if let Err(e) = self.gamepad.set_rumble(left_u16, right_u16, 500) {
                    tracing::error!("SDL3 set_rumble error: {}", e);
                }
                self.last_rumble_update = Some(now);
            } else if changed {
                if let Err(e) = self.gamepad.set_rumble(0, 0, 0) {
                    tracing::error!("SDL3 set_rumble (stop) error: {}", e);
                }
                self.last_rumble_update = None;
            }
        }
    }

    pub fn handle_touchpad_motion(&mut self, touchpad: i32, finger: i32, x: f32, y: f32, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        
        let key = (touchpad, finger);
        if let Some((last_x, last_y)) = self.finger_tracking.get(&key) {
            let dx = x - last_x;
            let dy = y - last_y;
            
            // Multiply by base resolution scalar (e.g. 1920) and sensitivity
            let scalar = 1920.0 * cfg.mouse.sensitivity;
            self.mouse_state.dx += (dx * scalar) as i16;
            self.mouse_state.dy += (dy * scalar) as i16;
        }
        self.finger_tracking.insert(key, (x, y));
    }

    pub fn handle_touchpad_down(&mut self, _touchpad: i32, _finger: i32, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        self.apply_action(&cfg.mouse.touchpad_soft_action, true);
    }

    pub fn handle_touchpad_up(&mut self, touchpad: i32, finger: i32, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        self.finger_tracking.remove(&(touchpad, finger));
        self.apply_action(&cfg.mouse.touchpad_soft_action, false);
    }

    pub fn handle_touchpad_button(&mut self, down: bool, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        self.apply_action(&cfg.mouse.touchpad_hard_action, down);
    }

    fn apply_action(&mut self, action_str: &str, down: bool) {
        match Action::parse(action_str) {
            Action::Mouse(btn) => {
                if down {
                    self.mouse_state.buttons |= btn;
                } else {
                    self.mouse_state.buttons &= !btn;
                }
            }
            Action::Keyboard(key) => {
                let idx = key as usize;
                if idx < 256 {
                    if down {
                        self.keyboard_state.key_bitmap[idx / 8] |= 1 << (idx % 8);
                    } else {
                        self.keyboard_state.key_bitmap[idx / 8] &= !(1 << (idx % 8));
                    }
                }
            }
            Action::None => {}
        }
    }

    pub fn update_and_send(&mut self, cfg: &Config, deadzone: i16, viiper: &ViiperManager) {
        let mut state = Xbox360DeviceState::default();
        let mut kb_state = KeyboardDeviceState::default();
        
        // Preserve any keys held down by touchpad actions
        kb_state.key_bitmap = self.keyboard_state.key_bitmap;

        crate::mapping::update_from_sdl_gamepad(&mut state, Some(&mut kb_state), &self.gamepad, cfg, deadzone);
        
        if let Err(e) = viiper.set_xbox360_state(self.dev_handle, state) {
            tracing::error!("Error sending state to viiper: {}", e);
        }

        if let Some(mh) = self.mouse_handle {
            if let Err(e) = viiper.set_mouse_state(mh, self.mouse_state) {
                tracing::error!("Error sending mouse state: {}", e);
            }
            // Mouse deltas are consumed each poll cycle, so we reset them
            self.mouse_state.dx = 0;
            self.mouse_state.dy = 0;
            self.mouse_state.wheel = 0;
            self.mouse_state.pan = 0;
        }

        if let Some(kh) = self.keyboard_handle {
            if let Err(e) = viiper.set_keyboard_state(kh, kb_state) {
                tracing::error!("Error sending keyboard state: {}", e);
            }
        }
    }

    pub fn destroy(&mut self, viiper: &ViiperManager) {
        if let Some(mh) = self.mouse_handle {
            viiper.remove_virtual_mouse(mh);
        }
        if let Some(kh) = self.keyboard_handle {
            viiper.remove_virtual_keyboard(kh);
        }
        if let Err(e) = viiper.remove_virtual_xbox_controller(self.dev_handle, self.bus_id) {
            tracing::error!("Failed to remove virtual xbox device: {}", e);
        }
    }
}
