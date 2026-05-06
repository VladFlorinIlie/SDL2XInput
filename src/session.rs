use sdl3::gamepad::Gamepad;
use crate::viiper_bridge::{Xbox360DeviceState, Xbox360DeviceHandle, ViiperManager, MouseDeviceHandle, MouseDeviceState, KeyboardDeviceHandle, KeyboardDeviceState};
use std::sync::mpsc;
use crate::config::Config;
use crate::keys::Action;
use std::collections::HashMap;

struct TouchState {
    start_x: f32,
    start_y: f32,
    last_x: f32,
    last_y: f32,
    start_time: std::time::Instant,
    is_tap: bool,
    is_drag_tap: bool,
}

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
    
    // Cached states to avoid spamming IOCTLs
    last_xbox_state: Xbox360DeviceState,
    last_mouse_buttons: u8,
    last_keyboard_state: KeyboardDeviceState,
    
    // Touchpad Absolute Tracking
    // Maps (touchpad_id, finger_id) -> TouchState
    finger_tracking: HashMap<(i32, i32), TouchState>,
    
    // Actions that need to be released after a few frames
    pending_action_releases: Vec<(Action, u8)>,
    
    // For tap-and-drag gesture
    last_tap_time: Option<std::time::Instant>,
    
    // Cached mapping for O(1) hot-loop performance
    pre_parsed_kb_mapping: HashMap<String, u8>,
    touchpad_soft_action: Action,
    touchpad_hard_action: Action,
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

        let needs_keyboard = cfg.keyboard.enabled || !cfg.keyboard.mapping.is_empty() || {
            matches!(Action::parse(&cfg.mouse.touchpad_soft_action), Action::Keyboard(_)) ||
            matches!(Action::parse(&cfg.mouse.touchpad_hard_action), Action::Keyboard(_))
        };

        let keyboard_handle = if needs_keyboard {
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

        let mut pre_parsed_kb_mapping = HashMap::new();
        if needs_keyboard {
            for (phys_name, mapped_key_name) in &cfg.keyboard.mapping {
                if let Action::Keyboard(keycode) = Action::parse(mapped_key_name) {
                    pre_parsed_kb_mapping.insert(phys_name.clone(), keycode);
                }
            }
        }

        Self { 
            gamepad, dev_handle, bus_id, rumble_rx, 
            rumble_state: (0, 0), last_rumble_update: None,
            mouse_handle, keyboard_handle,
            mouse_state: MouseDeviceState::default(),
            keyboard_state: KeyboardDeviceState::default(),
            last_xbox_state: Xbox360DeviceState::default(),
            last_mouse_buttons: 0,
            last_keyboard_state: KeyboardDeviceState::default(),
            finger_tracking: HashMap::new(),
            pending_action_releases: Vec::new(),
            last_tap_time: None,
            pre_parsed_kb_mapping,
            touchpad_soft_action: Action::parse(&cfg.mouse.touchpad_soft_action),
            touchpad_hard_action: Action::parse(&cfg.mouse.touchpad_hard_action),
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
        if let Some(touch) = self.finger_tracking.get_mut(&key) {
            let dx = x - touch.last_x;
            let dy = y - touch.last_y;
            
            // Relaxed distance threshold for a tap (approx 7% of touchpad) to allow for finger roll
            let dist_sq = (x - touch.start_x).powi(2) + (y - touch.start_y).powi(2);
            if dist_sq > cfg.mouse.tap_distance_threshold {
                touch.is_tap = false;
            }

            // Multiply by base resolution scalar and sensitivity
            let scalar = 800.0 * cfg.mouse.sensitivity;
            self.mouse_state.dx += (dx * scalar) as i16;
            self.mouse_state.dy += (dy * scalar) as i16;
            
            touch.last_x = x;
            touch.last_y = y;
        }
    }

    pub fn handle_touchpad_down(&mut self, touchpad: i32, finger: i32, x: f32, y: f32, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        
        let now = std::time::Instant::now();
        let is_drag_tap = if let Some(last) = self.last_tap_time {
            now.duration_since(last).as_millis() < cfg.mouse.drag_tap_time_ms
        } else {
            false
        };

        if is_drag_tap {
            self.apply_action(self.touchpad_soft_action, true);
        }

        self.finger_tracking.insert((touchpad, finger), TouchState {
            start_x: x, start_y: y, last_x: x, last_y: y,
            start_time: now,
            is_tap: true,
            is_drag_tap,
        });
    }

    pub fn handle_touchpad_up(&mut self, touchpad: i32, finger: i32, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        if let Some(touch) = self.finger_tracking.remove(&(touchpad, finger)) {
            if touch.is_drag_tap {
                // End the drag
                self.apply_action(self.touchpad_soft_action, false);
            } else if touch.is_tap && touch.start_time.elapsed().as_millis() < cfg.mouse.tap_time_ms {
                // It's a quick tap
                self.apply_action(self.touchpad_soft_action, true);
                self.pending_action_releases.push((self.touchpad_soft_action, 5));
                self.last_tap_time = Some(std::time::Instant::now());
            }
        }
    }

    pub fn handle_touchpad_button(&mut self, down: bool, cfg: &Config) {
        if self.mouse_handle.is_none() || !cfg.mouse.enabled { return; }
        self.apply_action(self.touchpad_hard_action, down);
    }

    fn apply_action(&mut self, action: Action, down: bool) {
        match action {
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
        
        // Process pending action releases
        let mut i = 0;
        while i < self.pending_action_releases.len() {
            if self.pending_action_releases[i].1 <= 1 {
                let action = self.pending_action_releases.remove(i).0;
                self.apply_action(action, false);
            } else {
                self.pending_action_releases[i].1 -= 1;
                i += 1;
            }
        }

        // Preserve any keys held down by touchpad actions
        kb_state.key_bitmap = self.keyboard_state.key_bitmap;

        crate::mapping::update_from_sdl_gamepad(&mut state, Some(&mut kb_state), &self.gamepad, cfg, deadzone, &self.pre_parsed_kb_mapping);
        
        if state != self.last_xbox_state {
            if let Err(e) = viiper.set_xbox360_state(self.dev_handle, state) {
                tracing::error!("Error sending state to viiper: {}", e);
            }
            self.last_xbox_state = state;
        }

        if let Some(mh) = self.mouse_handle {
            let has_movement = self.mouse_state.dx != 0 || self.mouse_state.dy != 0 || self.mouse_state.wheel != 0 || self.mouse_state.pan != 0;
            let buttons_changed = self.mouse_state.buttons != self.last_mouse_buttons;
            
            if has_movement || buttons_changed {
                if let Err(e) = viiper.set_mouse_state(mh, self.mouse_state) {
                    tracing::error!("Error sending mouse state: {}", e);
                }
                self.last_mouse_buttons = self.mouse_state.buttons;
            }

            // Mouse deltas are consumed each poll cycle, so we reset them
            self.mouse_state.dx = 0;
            self.mouse_state.dy = 0;
            self.mouse_state.wheel = 0;
            self.mouse_state.pan = 0;
        }

        if let Some(kh) = self.keyboard_handle {
            if kb_state != self.last_keyboard_state {
                if let Err(e) = viiper.set_keyboard_state(kh, kb_state) {
                    tracing::error!("Error sending keyboard state: {}", e);
                }
                self.last_keyboard_state = kb_state;
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
