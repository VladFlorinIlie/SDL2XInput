use sdl3::gamepad::Gamepad;
use viiper_client::{AsyncDeviceStream, devices::xbox360::Xbox360Input};
use tokio::sync::mpsc;
use crate::config::Config;

pub struct ActiveSession {
    pub gamepad: Gamepad,
    pub dev_id: String,
    pub dev_stream: AsyncDeviceStream,
    pub rumble_rx: mpsc::UnboundedReceiver<(u8, u8)>,
}

impl ActiveSession {
    pub async fn apply_rumble(&mut self) {
        while let Ok((left, right)) = self.rumble_rx.try_recv() {
            let left_u16 = (left as u16) << 8 | (left as u16);
            let right_u16 = (right as u16) << 8 | (right as u16);
            let _ = self.gamepad.set_rumble(left_u16, right_u16, 0);
        }
    }

    pub async fn update_and_send(&mut self, cfg: &Config) {
        let mut istate = Xbox360Input::default();
        crate::mapping::update_from_sdl_gamepad(&mut istate, &self.gamepad, cfg);
        if let Err(e) = self.dev_stream.send(&istate).await {
            tracing::error!("Error sending state to viiper: {}", e);
        }
    }
}
