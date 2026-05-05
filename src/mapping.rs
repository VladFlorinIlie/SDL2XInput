use sdl3::gamepad::{Gamepad, Axis, Button};
use viiper_client::devices::xbox360::{self, Xbox360Input};
use crate::config::{Config, XboxButton};

pub fn update_from_sdl_gamepad(istate: &mut Xbox360Input, gp: &Gamepad, cfg: &Config) {
    let mut b: u32 = 0;

    // Helper: apply one physical button to its remapped virtual Xbox 360 bit.
    let mut set = |pressed: bool, target: XboxButton| {
        if pressed {
            b |= xbox_button_bit(target);
        }
    };

    set(gp.button(Button::South),         cfg.buttons.south);
    set(gp.button(Button::East),          cfg.buttons.east);
    set(gp.button(Button::West),          cfg.buttons.west);
    set(gp.button(Button::North),         cfg.buttons.north);
    set(gp.button(Button::Start),         cfg.buttons.start);
    set(gp.button(Button::Back),          cfg.buttons.back);
    set(gp.button(Button::Guide),         cfg.buttons.guide);
    set(gp.button(Button::LeftStick),     cfg.buttons.left_stick);
    set(gp.button(Button::RightStick),    cfg.buttons.right_stick);
    set(gp.button(Button::LeftShoulder),  cfg.buttons.left_shoulder);
    set(gp.button(Button::RightShoulder), cfg.buttons.right_shoulder);
    set(gp.button(Button::DPadUp),        cfg.buttons.dpad_up);
    set(gp.button(Button::DPadDown),      cfg.buttons.dpad_down);
    set(gp.button(Button::DPadLeft),      cfg.buttons.dpad_left);
    set(gp.button(Button::DPadRight),     cfg.buttons.dpad_right);

    istate.buttons = b;

    // Triggers
    let lt_raw = gp.axis(Axis::TriggerLeft).max(0);
    let rt_raw = gp.axis(Axis::TriggerRight).max(0);
    let (lt_raw, rt_raw) = if cfg.axes.swap_triggers { (rt_raw, lt_raw) } else { (lt_raw, rt_raw) };
    istate.lt = ((lt_raw as i32 * 255) / 32767).clamp(0, 255) as u8;
    istate.rt = ((rt_raw as i32 * 255) / 32767).clamp(0, 255) as u8;

    // Sticks — SDL3 Y axis is already inverted (up = negative) so we negate to match XInput
    let lx = gp.axis(Axis::LeftX);
    let ly = gp.axis(Axis::LeftY).saturating_neg();
    let rx = gp.axis(Axis::RightX);
    let ry = gp.axis(Axis::RightY).saturating_neg();

    istate.lx = if cfg.axes.invert_left_x  { lx.saturating_neg() } else { lx };
    istate.ly = if cfg.axes.invert_left_y  { ly.saturating_neg() } else { ly };
    istate.rx = if cfg.axes.invert_right_x { rx.saturating_neg() } else { rx };
    istate.ry = if cfg.axes.invert_right_y { ry.saturating_neg() } else { ry };
}

/// Convert an XboxButton variant to its XInput bitmask bit.
fn xbox_button_bit(btn: XboxButton) -> u32 {
    match btn {
        XboxButton::A             => xbox360::BUTTON_A as u32,
        XboxButton::B             => xbox360::BUTTON_B as u32,
        XboxButton::X             => xbox360::BUTTON_X as u32,
        XboxButton::Y             => xbox360::BUTTON_Y as u32,
        XboxButton::Start         => xbox360::BUTTON_START as u32,
        XboxButton::Back          => xbox360::BUTTON_BACK as u32,
        XboxButton::Guide         => xbox360::BUTTON_GUIDE as u32,
        XboxButton::LeftStick     => xbox360::BUTTON_L_THUMB as u32,
        XboxButton::RightStick    => xbox360::BUTTON_R_THUMB as u32,
        XboxButton::LeftShoulder  => xbox360::BUTTON_L_SHOULDER as u32,
        XboxButton::RightShoulder => xbox360::BUTTON_R_SHOULDER as u32,
        XboxButton::DPadUp        => xbox360::BUTTON_D_PAD_UP as u32,
        XboxButton::DPadDown      => xbox360::BUTTON_D_PAD_DOWN as u32,
        XboxButton::DPadLeft      => xbox360::BUTTON_D_PAD_LEFT as u32,
        XboxButton::DPadRight     => xbox360::BUTTON_D_PAD_RIGHT as u32,
    }
}
