use sdl3::gamepad::Gamepad;
use crate::viiper_bridge::{Xbox360DeviceState, Xbox360DeviceHandle, ViiperManager};
use std::sync::mpsc;
use crate::config::Config;

pub struct ActiveSession {
    pub gamepad: Gamepad,
    pub dev_handle: Xbox360DeviceHandle,
    pub rumble_rx: mpsc::Receiver<(u8, u8)>,
    rumble_state: (u8, u8),
}

impl ActiveSession {
    pub fn new(gamepad: Gamepad, dev_handle: Xbox360DeviceHandle, rumble_rx: mpsc::Receiver<(u8, u8)>) -> Self {
        Self { gamepad, dev_handle, rumble_rx, rumble_state: (0, 0) }
    }

    pub fn apply_rumble(&mut self) {
        while let Ok(val) = self.rumble_rx.try_recv() {
            self.rumble_state = val;
        }
        let (left, right) = self.rumble_state;
        if left > 0 || right > 0 {
            // Re-apply every tick with a short window; SDL3 stops automatically
            // once the window expires and we stop re-applying on (0,0).
            let _ = self.gamepad.set_rumble(
                (left  as u16) << 8 | left  as u16,
                (right as u16) << 8 | right as u16,
                50,
            );
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
