use sdl3::gamepad::Gamepad;
use crate::viiper_bridge::{Xbox360DeviceState, Xbox360DeviceHandle, ViiperManager};
use std::sync::mpsc;
use crate::config::Config;

pub struct ActiveSession {
    pub gamepad: Gamepad,
    pub dev_handle: Xbox360DeviceHandle,
    pub bus_id: u32,
    pub rumble_rx: mpsc::Receiver<(u8, u8)>,
    rumble_state: (u8, u8),
    last_rumble_update: Option<std::time::Instant>,
}

impl ActiveSession {
    pub fn new(gamepad: Gamepad, dev_handle: Xbox360DeviceHandle, bus_id: u32, rumble_rx: mpsc::Receiver<(u8, u8)>) -> Self {
        Self { gamepad, dev_handle, bus_id, rumble_rx, rumble_state: (0, 0), last_rumble_update: None }
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

    pub fn update_and_send(&mut self, cfg: &Config, deadzone: i16, viiper: &ViiperManager) {
        let mut state = Xbox360DeviceState::default();
        crate::mapping::update_from_sdl_gamepad(&mut state, &self.gamepad, cfg, deadzone);
        if let Err(e) = viiper.set_xbox360_state(self.dev_handle, state) {
            tracing::error!("Error sending state to viiper: {}", e);
        }
    }
}
