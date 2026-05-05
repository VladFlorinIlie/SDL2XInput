use sdl3::gamepad::{Gamepad, Axis, Button};
use viiper_client::devices::xbox360::{self, Xbox360Input};

pub fn update_from_sdl_gamepad(istate: &mut Xbox360Input, gp: &Gamepad) {
    let mut b: u32 = 0;

    if gp.button(Button::South) { b |= xbox360::BUTTON_A as u32; }
    if gp.button(Button::East) { b |= xbox360::BUTTON_B as u32; }
    if gp.button(Button::West) { b |= xbox360::BUTTON_X as u32; }
    if gp.button(Button::North) { b |= xbox360::BUTTON_Y as u32; }
    if gp.button(Button::Start) { b |= xbox360::BUTTON_START as u32; }
    if gp.button(Button::Back) { b |= xbox360::BUTTON_BACK as u32; }
    if gp.button(Button::LeftStick) { b |= xbox360::BUTTON_L_THUMB as u32; }
    if gp.button(Button::RightStick) { b |= xbox360::BUTTON_R_THUMB as u32; }
    if gp.button(Button::LeftShoulder) { b |= xbox360::BUTTON_L_SHOULDER as u32; }
    if gp.button(Button::RightShoulder) { b |= xbox360::BUTTON_R_SHOULDER as u32; }
    if gp.button(Button::Guide) { b |= xbox360::BUTTON_GUIDE as u32; }
    if gp.button(Button::DPadUp) { b |= xbox360::BUTTON_D_PAD_UP as u32; }
    if gp.button(Button::DPadDown) { b |= xbox360::BUTTON_D_PAD_DOWN as u32; }
    if gp.button(Button::DPadLeft) { b |= xbox360::BUTTON_D_PAD_LEFT as u32; }
    if gp.button(Button::DPadRight) { b |= xbox360::BUTTON_D_PAD_RIGHT as u32; }

    let lt = gp.axis(Axis::TriggerLeft);
    let rt = gp.axis(Axis::TriggerRight);

    istate.buttons = b;
    istate.lt = ((lt.max(0) as i32 * 255) / 32767).clamp(0, 255) as u8;
    istate.rt = ((rt.max(0) as i32 * 255) / 32767).clamp(0, 255) as u8;

    istate.lx = gp.axis(Axis::LeftX);
    istate.ly = gp.axis(Axis::LeftY).saturating_neg();
    istate.rx = gp.axis(Axis::RightX);
    istate.ry = gp.axis(Axis::RightY).saturating_neg();
}
