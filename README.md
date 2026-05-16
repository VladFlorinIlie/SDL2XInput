<h1>
   SDL2XInput
  <img src="assets/icon.png" width="48" align="top" alt="Logo" />
</h1>

A small, standalone Windows tool that makes any SDL3-compatible controller (Steam Controller, DualSense, Switch Pro, etc.) act like a standard Xbox 360 controller system-wide.

## What it does

A lot of controllers don't work well outside of Steam because they lack native XInput support. SDL2XInput solves this. It reads your physical controller and creates a virtual Xbox 360 controller directly at the Windows driver level.

It can also act as a virtual mouse and keyboard, letting you map buttons to keys or use your controller's touchpad and gyro to move the cursor.

## Features

- **No bloat**: Doesn't require Steam or background servers.
- **No ViGEmBus**: Uses native Windows drivers via USBIP.
- **Mouse & Keyboard**: Map any controller button to a keyboard key or mouse click.
- **Touchpad & Gyro**: Full support for using them as a mouse.
- **Rumble**: Haptic feedback passes through perfectly (for supported controllers).
- **Customizable**: Swap buttons, tweak deadzones, and invert axes via a simple config file.

## Requirements

**The only thing you need to install** is the [usbip-win2](https://github.com/vadimgrn/usbip-win2) driver. This is what allows the app to spawn the virtual controllers.

## How to use it

1. Install [usbip-win2](https://github.com/vadimgrn/usbip-win2).
2. Download `sdl2xinput.exe` (or build it yourself).
3. Double-click the `.exe`.

That's it. It will sit in your system tray and do its thing. To close it, just right-click the tray icon. Logs are saved to `sdl2xinput.log` if you need to troubleshoot.

## Configuration (Optional)

If you want to change button mappings, tweak mouse sensitivity, or map buttons to keyboard keys, create a `config.toml` next to the `.exe`. 

Here is a quick example:

```toml
[buttons]
# Swap A and B (Nintendo style)
south = "b"
east  = "a"

[mouse]
enabled = true
sensitivity = 1.5

[mapping]
# Make the Guide button press 'Escape' on your keyboard
guide = "Escape"
# Hold a back paddle to turn on gyro aiming
left_paddle1 = "gyro"
```

*For more advanced options (polling rates, device filtering, deadzones), run `sdl2xinput.exe --help` from your terminal.*

## The "Double Input" Problem

Windows will now see *two* controllers: your real one, and the virtual Xbox 360 one. Some games will read both at the same time, causing double inputs. 

**The fix:** Install [HidHide](https://github.com/nefarius/HidHide) and set it up to hide your real controller from everything *except* `sdl2xinput.exe`. 

## Building from source

If you want to compile the project yourself, you'll need:
* Rust & Cargo
* Go (for the embedded VIIPER components)
* GCC Toolchain (MinGW-w64 on Windows)

```bash
git clone --recurse-submodules https://github.com/VladFlorinIlie/sdl2xinput.git
cd sdl2xinput
cargo build --release
```

## Credits

This project wouldn't exist without [InputFusion](https://github.com/xan105/InputFusion) (for the mapping logic) and [SISR](https://github.com/Alia5/SISR) / [VIIPER](https://github.com/Alia5/VIIPER) (for the virtual USB driver approach). 
