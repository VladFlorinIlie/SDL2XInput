use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Mouse(u8),
    Keyboard(u8),
    None,
}

impl Action {
    pub fn parse(name: &str) -> Self {
        let n = name.trim().to_lowercase();
        if n.is_empty() || n == "none" {
            return Action::None;
        }
        if let Some(btn) = get_mouse_button(&n) {
            Action::Mouse(btn)
        } else if let Some(key) = get_key_code(&n) {
            Action::Keyboard(key)
        } else {
            tracing::warn!("Unknown key/mouse button mapping: '{}'", name);
            Action::None
        }
    }
}

pub fn get_key_code(name: &str) -> Option<u8> {
    static KEY_MAP: OnceLock<HashMap<&'static str, u8>> = OnceLock::new();
    let map = KEY_MAP.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("a", 0x04);
        m.insert("b", 0x05);
        m.insert("c", 0x06);
        m.insert("d", 0x07);
        m.insert("e", 0x08);
        m.insert("f", 0x09);
        m.insert("g", 0x0A);
        m.insert("h", 0x0B);
        m.insert("i", 0x0C);
        m.insert("j", 0x0D);
        m.insert("k", 0x0E);
        m.insert("l", 0x0F);
        m.insert("m", 0x10);
        m.insert("n", 0x11);
        m.insert("o", 0x12);
        m.insert("p", 0x13);
        m.insert("q", 0x14);
        m.insert("r", 0x15);
        m.insert("s", 0x16);
        m.insert("t", 0x17);
        m.insert("u", 0x18);
        m.insert("v", 0x19);
        m.insert("w", 0x1A);
        m.insert("x", 0x1B);
        m.insert("y", 0x1C);
        m.insert("z", 0x1D);
        m.insert("1", 0x1E);
        m.insert("2", 0x1F);
        m.insert("3", 0x20);
        m.insert("4", 0x21);
        m.insert("5", 0x22);
        m.insert("6", 0x23);
        m.insert("7", 0x24);
        m.insert("8", 0x25);
        m.insert("9", 0x26);
        m.insert("0", 0x27);
        m.insert("enter", 0x28);
        m.insert("escape", 0x29);
        m.insert("backspace", 0x2A);
        m.insert("tab", 0x2B);
        m.insert("space", 0x2C);
        m.insert("minus", 0x2D);
        m.insert("equal", 0x2E);
        m.insert("leftbrace", 0x2F);
        m.insert("rightbrace", 0x30);
        m.insert("backslash", 0x31);
        m.insert("semicolon", 0x33);
        m.insert("apostrophe", 0x34);
        m.insert("grave", 0x35);
        m.insert("comma", 0x36);
        m.insert("period", 0x37);
        m.insert("slash", 0x38);
        m.insert("capslock", 0x39);
        m.insert("f1", 0x3A);
        m.insert("f2", 0x3B);
        m.insert("f3", 0x3C);
        m.insert("f4", 0x3D);
        m.insert("f5", 0x3E);
        m.insert("f6", 0x3F);
        m.insert("f7", 0x40);
        m.insert("f8", 0x41);
        m.insert("f9", 0x42);
        m.insert("f10", 0x43);
        m.insert("f11", 0x44);
        m.insert("f12", 0x45);
        m.insert("printscreen", 0x46);
        m.insert("scrolllock", 0x47);
        m.insert("pause", 0x48);
        m.insert("insert", 0x49);
        m.insert("home", 0x4A);
        m.insert("pageup", 0x4B);
        m.insert("delete", 0x4C);
        m.insert("end", 0x4D);
        m.insert("pagedown", 0x4E);
        m.insert("right", 0x4F);
        m.insert("left", 0x50);
        m.insert("down", 0x51);
        m.insert("up", 0x52);
        m.insert("numlock", 0x53);
        m.insert("mute", 0x7F);
        m.insert("volumeup", 0x80);
        m.insert("volumedown", 0x81);
        m
    });

    map.get(name.replace(" ", "").replace("_", "").as_str()).copied()
}

pub fn get_mouse_button(name: &str) -> Option<u8> {
    match name.replace(" ", "").replace("_", "").as_str() {
        "mouseleft" | "leftclick" => Some(0x01),
        "mouseright" | "rightclick" => Some(0x02),
        "mousemiddle" | "middleclick" => Some(0x04),
        "mouseback" => Some(0x08),
        "mouseforward" => Some(0x10),
        _ => None,
    }
}
