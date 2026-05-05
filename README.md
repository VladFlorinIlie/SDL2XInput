# SDL2XInput

A lightweight, high-performance standalone Rust application that bridges SDL3-compatible controllers (like the Steam Controller, DualSense, Flydigi, etc.) to virtual Xbox 360 controllers system-wide using the [VIIPER](https://github.com/Alia5/VIIPER) and the USBIP system.

## Origins & Credits

This project is a standalone evolution of two amazing open-source projects:
* **[InputFusion](https://github.com/xan105/InputFusion)**: We utilize the mathematical mapping logic and controller translation concepts designed by `xan105` to perfectly translate generic inputs into flawless XInput behavior.
* **[SISR](https://github.com/Alia5/SISR)** (Steam Input System-Wide Redirector): We borrow the brilliant concept by `Alia5` of using [VIIPER](https://github.com/Alia5/VIIPER) to spawn a virtual gamepad at the USB driver level.

By combining these two concepts into a native SDL3 application, this project **completely removes the need to have Steam running in the background**, and completely avoids the tedious and anti-cheat-triggering `.dll` hooking required by the original InputFusion!

## How it Works

Many modern controllers (especially the Steam Controller or generic HID controllers) do not natively support XInput, making them incompatible with many games outside of Steam's ecosystem. 

**SDL2XInput** solves this natively:
1. It reads your physical controller using **SDL3**.
2. It translates the inputs (buttons, analog triggers, axes) into Microsoft XInput math.
3. It sends that data to a local **VIIPER** server, which spawns a system-wide, undetectable Virtual Xbox 360 controller at the USB driver level.

## Features
* **Zero Custom Drivers**: Relies entirely on the native Windows `xusb22.sys` driver via USBIP. No `ViGEmBus` required.
* **Flawless Rumble**: Supports bidirectional rumble pass-through from the virtual controller back to your physical hardware.
* **Feedback Loop Protection**: Intelligently ignores its own spawned virtual controllers to prevent infinite loop crashes.
* **Multi-Controller Support**: Dynamically creates a 1-to-1 virtual controller for every physical controller connected.

## Prerequisites

1. **Rust & Cargo**: To compile the source code.
2. **VIIPER Server**: You must have a VIIPER server running on your machine.
   * Download the latest `viiper-windows-amd64.exe` from the Viiper repository.
   * Run `viiper-windows-amd64.exe server` in a terminal.

## Installation & Build

Clone the repository and build the optimized standalone executable:

```bash
git clone https://github.com/yourusername/sdl2xinput.git
cd sdl2xinput
cargo build --release
```

Your fully standalone executable will be located at `target/release/sdl2xinput.exe`.

## Usage

Ensure your VIIPER server is running, then launch the redirector:

```bash
.\sdl2xinput.exe
```

### Command Line Arguments

You can configure the redirector using CLI arguments. Use `--help` to see all options:

```bash
.\sdl2xinput.exe --help
```

* `-v, --viiper-address <ADDRESS>`: The IP and Port of your VIIPER server (Default: `127.0.0.1:3242`).
* `-m, --max-controllers <NUMBER>`: Limit the maximum number of active virtual controllers. Useful for debugging or testing physical controllers that natively present as Xbox 360 controllers. (Default: `1`).

**Example:**
```bash
.\sdl2xinput.exe -v 127.0.0.1:3242 -m 2
```

## The "Double Input" Problem

Because this application acts as a standalone bridge, Windows will natively see **two** controllers: your physical controller (e.g. Steam Controller) and the Virtual Xbox 360 Controller.

If a game reads all connected devices, you will experience "Double Input". To fix this without `.dll` hooking, you have two choices:
1. **For SDL Games**: Add the `SDL_GAMECONTROLLER_IGNORE_DEVICES` environment variable to the game's launch options with your physical controller's Vendor ID and Product ID.
2. **System-wide Solution (Recommended)**: Install [HidHide](https://github.com/nefarius/HidHide) and configure it to hide your physical controller from all applications *except* `sdl2xinput.exe`.

## License
GPLv3 License
