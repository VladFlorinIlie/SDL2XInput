use serde::Deserialize;
use std::path::Path;
use anyhow::Result;

/// Top-level config loaded from a TOML file (e.g. config.toml).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub buttons: ButtonRemap,
    pub axes: AxisConfig,
    pub mouse: MouseConfig,
    pub keyboard: KeyboardConfig,
}

/// Remaps each physical SDL3 button to a virtual Xbox 360 button.
/// The string values must be one of the Xbox 360 button names listed below.
///
/// Xbox 360 button names: a, b, x, y, start, back,
///   left_stick, right_stick, left_shoulder, right_shoulder, guide,
///   dpad_up, dpad_down, dpad_left, dpad_right
///
/// Example — swap A/B for Nintendo-style layout:
/// ```toml
/// [buttons]
/// south = "b"
/// east  = "a"
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ButtonRemap {
    // Face buttons (SDL names map 1-to-1 to Xbox letters by default)
    pub south:          XboxButton,
    pub east:           XboxButton,
    pub west:           XboxButton,
    pub north:          XboxButton,
    // Menu / system
    pub start:          XboxButton,
    pub back:           XboxButton,
    pub guide:          XboxButton,
    // Sticks
    pub left_stick:     XboxButton,
    pub right_stick:    XboxButton,
    // Shoulders
    pub left_shoulder:  XboxButton,
    pub right_shoulder: XboxButton,
    // D-Pad
    pub dpad_up:        XboxButton,
    pub dpad_down:      XboxButton,
    pub dpad_left:      XboxButton,
    pub dpad_right:     XboxButton,
}

impl Default for ButtonRemap {
    /// The identity mapping — physical button goes to its natural Xbox 360 equivalent.
    fn default() -> Self {
        Self {
            south:          XboxButton::A,
            east:           XboxButton::B,
            west:           XboxButton::X,
            north:          XboxButton::Y,
            start:          XboxButton::Start,
            back:           XboxButton::Back,
            guide:          XboxButton::Guide,
            left_stick:     XboxButton::LeftStick,
            right_stick:    XboxButton::RightStick,
            left_shoulder:  XboxButton::LeftShoulder,
            right_shoulder: XboxButton::RightShoulder,
            dpad_up:        XboxButton::DPadUp,
            dpad_down:      XboxButton::DPadDown,
            dpad_left:      XboxButton::DPadLeft,
            dpad_right:     XboxButton::DPadRight,
        }
    }
}

/// Axis / trigger tweaks.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AxisConfig {
    /// Invert the left stick Y axis (useful for flight/racing games).
    pub invert_left_y:  bool,
    /// Invert the right stick Y axis.
    pub invert_right_y: bool,
    /// Invert the left stick X axis.
    pub invert_left_x:  bool,
    /// Invert the right stick X axis.
    pub invert_right_x: bool,
    /// Swap the left and right triggers.
    pub swap_triggers:  bool,
}

/// Mouse emulation using touchpads or gyro.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MouseConfig {
    pub enabled: bool,
    pub sensitivity: f32,
    pub touchpad_soft_action: String,
    pub touchpad_hard_action: String,
    pub tap_distance_threshold: f32,
    pub tap_time_ms: u128,
    pub drag_tap_time_ms: u128,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitivity: 1.5,
            touchpad_soft_action: "MouseLeft".to_string(),
            touchpad_hard_action: "MouseRight".to_string(),
            tap_distance_threshold: 0.005,
            tap_time_ms: 350,
            drag_tap_time_ms: 400,
        }
    }
}

/// Keyboard emulation mapping physical buttons to keys.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct KeyboardConfig {
    pub enabled: bool,
    pub mapping: std::collections::HashMap<String, String>,
}

/// All valid Xbox 360 button targets that a physical button can be remapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XboxButton {
    A, B, X, Y,
    Start, Back, Guide,
    LeftStick, RightStick,
    LeftShoulder, RightShoulder,
    DPadUp, DPadDown, DPadLeft, DPadRight,
}

impl Config {
    /// Load config from a TOML file. Returns `Config::default()` if the path is None.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        match path {
            None => Ok(Self::default()),
            Some(p) => {
                let raw = std::fs::read_to_string(p)?;
                let cfg: Config = toml::from_str(&raw)?;
                tracing::info!("Loaded config from: {}", p.display());
                Ok(cfg)
            }
        }
    }
}
