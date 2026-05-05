use sdl3::gamepad::{Gamepad, Axis, Button};
use crate::viiper_bridge::Xbox360DeviceState;
use crate::config::{Config, XboxButton};

pub fn update_from_sdl_gamepad(istate: &mut Xbox360DeviceState, gp: &Gamepad, cfg: &Config, deadzone: i16) {
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
    // We also apply a configurable hardware deadzone 
    // to eliminate micro-jitter from highly sensitive sticks.
    let lx = apply_deadzone(gp.axis(Axis::LeftX), deadzone);
    let ly = apply_deadzone(gp.axis(Axis::LeftY).saturating_neg(), deadzone);
    let rx = apply_deadzone(gp.axis(Axis::RightX), deadzone);
    let ry = apply_deadzone(gp.axis(Axis::RightY).saturating_neg(), deadzone);

    istate.lx = if cfg.axes.invert_left_x  { lx.saturating_neg() } else { lx };
    istate.ly = if cfg.axes.invert_left_y  { ly.saturating_neg() } else { ly };
    istate.rx = if cfg.axes.invert_right_x { rx.saturating_neg() } else { rx };
    istate.ry = if cfg.axes.invert_right_y { ry.saturating_neg() } else { ry };
}

/// Applies a center deadzone to prevent micro-jitter.
fn apply_deadzone(val: i16, deadzone: i16) -> i16 {
    if (val as i32).abs() < (deadzone as i32) {
        0
    } else {
        val
    }
}

// XInput Button Bitmasks (matches viiper_bridge expectations)
const BUTTON_DPAD_UP: u32 = 0x0001;
const BUTTON_DPAD_DOWN: u32 = 0x0002;
const BUTTON_DPAD_LEFT: u32 = 0x0004;
const BUTTON_DPAD_RIGHT: u32 = 0x0008;
const BUTTON_START: u32 = 0x0010;
const BUTTON_BACK: u32 = 0x0020;
const BUTTON_L_THUMB: u32 = 0x0040;
const BUTTON_R_THUMB: u32 = 0x0080;
const BUTTON_L_SHOULDER: u32 = 0x0100;
const BUTTON_R_SHOULDER: u32 = 0x0200;
const BUTTON_GUIDE: u32 = 0x0400;
const BUTTON_A: u32 = 0x1000;
const BUTTON_B: u32 = 0x2000;
const BUTTON_X: u32 = 0x4000;
const BUTTON_Y: u32 = 0x8000;

/// Convert an XboxButton variant to its XInput bitmask bit.
fn xbox_button_bit(btn: XboxButton) -> u32 {
    match btn {
        XboxButton::A             => BUTTON_A,
        XboxButton::B             => BUTTON_B,
        XboxButton::X             => BUTTON_X,
        XboxButton::Y             => BUTTON_Y,
        XboxButton::Start         => BUTTON_START,
        XboxButton::Back          => BUTTON_BACK,
        XboxButton::Guide         => BUTTON_GUIDE,
        XboxButton::LeftStick     => BUTTON_L_THUMB,
        XboxButton::RightStick    => BUTTON_R_THUMB,
        XboxButton::LeftShoulder  => BUTTON_L_SHOULDER,
        XboxButton::RightShoulder => BUTTON_R_SHOULDER,
        XboxButton::DPadUp        => BUTTON_DPAD_UP,
        XboxButton::DPadDown      => BUTTON_DPAD_DOWN,
        XboxButton::DPadLeft      => BUTTON_DPAD_LEFT,
        XboxButton::DPadRight     => BUTTON_DPAD_RIGHT,
    }
}

