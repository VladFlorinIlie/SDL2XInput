<h1>
   SDL2XInput
  <img src="assets/icon.png" width="48" align="top" alt="Logo" />
</h1>

A lightweight, high-performance standalone Rust application that bridges SDL3-compatible controllers (such as the Steam Controller) to virtual Xbox 360 controllers system-wide using the [VIIPER library](https://github.com/Alia5/VIIPER) to communicate with the USBIP server.

## Origins & Credits

This project is a standalone mix of two open-source projects:
* **[InputFusion](https://github.com/xan105/InputFusion)**: The project uses the mapping logic from `xan105` to translate inputs into standard XInput behavior.
* **[SISR](https://github.com/Alia5/SISR)** (Steam Input System-Wide Redirector): The project uses the approach from `Alia5` of using the [VIIPER](https://github.com/Alia5/VIIPER) library to spawn a virtual gamepad at the USB driver level.

By combining these concepts into a native SDL3 application that embeds `libviiper`, the project removes the need to run an external server process or have Steam running in the background.

## How it Works

Many modern controllers (especially the Steam Controller or generic HID controllers) do not natively support XInput, making them incompatible with many games outside of Steam's ecosystem. 

**SDL2XInput** solves this natively:
1. It reads the physical controller using **SDL3**.
2. It translates the inputs (buttons, analog triggers, axes) into XInput format.
3. It uses the **VIIPER library** (statically embedded) to communicate with the Windows USBIP bus and spawn a system-wide Virtual Xbox 360 controller.

## Features

* **Integrated VIIPER Library**: Uses `libviiper` to handle USBIP communications directly. No external server process required.
* **No Custom Drivers**: Uses the native Windows `xusb22.sys` driver via USBIP. No `ViGEmBus` required.
* **Rumble Support**: Supports bidirectional rumble pass-through from the virtual controller back to the physical hardware.
* **Button Remapping**: Supports custom button layouts and axis inversions via a TOML configuration file.
* **System Tray Integration**: Can run in the background. If launched via double-click, it detaches from the console and stays in the Windows system tray.
* **Logging**: Writes logs to `sdl2xinput.log` when running in the background, or to the console if launched from a terminal.
* **Adjustable Polling Rate**: Configurable polling frequency (1-1000 Hz) to balance latency and CPU usage.
* **Device Filtering**: Supports custom VID:PID blocklists to ignore specific hardware. By default, **all Xbox 360 controllers are ignored**. This is because the virtual controllers spawned by the application are recognized as Xbox 360 controllers, and the application must ignore them to prevent reading its own output and creating an infinite input loop.
* **Multi-Controller Support**: Creates a 1-to-1 virtual controller for every physical controller connected.

## Prerequisites

### For Building from Source
* **Rust & Cargo**: To compile the source code.
* **Go**: Required to compile the embedded VIIPER components.
* **GCC Toolchain (MinGW-w64)**: Required for the Rust compiler to link the C and Go components on Windows.

## Installation & Build

Clone the repository and build the optimized standalone executable:

```bash
git clone --recurse-submodules https://github.com/VladFlorinIlie/sdl2xinput.git
cd sdl2xinput
cargo build --release
```

The standalone executable will be located at `target/release/sdl2xinput.exe`. It is fully self-contained and requires no external DLLs or server processes.

## Usage

Simply launch the redirector. It will automatically initialize the **VIIPER library** and begin forwarding inputs.

**Running in the Background:**
Double-click the `sdl2xinput.exe` file from Windows Explorer. The console will hide, and an icon will appear in the system tray. Right-click the tray icon to exit the application. Logs will be written to `sdl2xinput.log` in the same directory.

**Running from Terminal:**
```powershell
.\sdl2xinput.exe
```
When launched from a terminal, the application will print logs directly to the console and can be terminated with `Ctrl+C`.

### Configuration (Button Remapping)

The project supports comprehensive button remapping through a `config.toml` file. If the file is not provided, a default identity mapping is used.

Example `config.toml` (Nintendo-style layout):
```toml
[buttons]
south = "b"
east  = "a"
west  = "y"
north = "x"

[axes]
swap_triggers = false
invert_left_y = false
```

### Command Line Arguments

The redirector can be configured using CLI arguments. Use `--help` to see all options:

* `-c, --config <FILE>`: Path to a TOML config file for button remapping and axis tweaks.
* `-p, --polling-rate <HZ>`: Input polling rate (1-1000 Hz). Higher values lower latency but use more CPU (Default: `250`).
* `--usb-server-addr <ADDRESS>`: The IP address and Port for the USBIP server (e.g. `127.0.0.1:3241`). Defaults to the system default if not provided.
* `-m, --max-controllers <NUMBER>`: Limit the maximum number of active virtual controllers (Default: `1`).
* `-d, --deadzone <INT>`: Hardware deadzone applied to analog sticks to eliminate micro-jitter (Default: `1000`). Set to `0` to disable completely.
* `--filter-device <VID:PID>`: Block a specific device by VID:PID (hex, e.g. `045E:028E`). Can be repeated.
* `--empty-device-filter`: Disables the default Xbox 360 controller blocklist. 
* `--no-tray`: Force the application to skip creating a system tray icon.

> [!WARNING]
> Use `--empty-device-filter` *only* if you are using a physical Xbox 360 controller and have another method (like HidHide) to hide the virtual controller from the application to prevent infinite loops.

## The "Double Input" Problem

> [!NOTE]
> Because the application acts as a standalone bridge, Windows will natively see **two** controllers: the physical controller and the Virtual Xbox 360 Controller.

If a game reads all connected devices, it will register "Double Input". To fix this without `.dll` hooking, there are two solutions:

1. **System-wide Solution (Recommended)**: Install [HidHide](https://github.com/nefarius/HidHide) and configure it to hide the physical controller from all applications *except* `sdl2xinput.exe`.
2. **For SDL Games**: Add the `SDL_GAMECONTROLLER_IGNORE_DEVICES` environment variable to the game's launch options with the physical controller's Vendor ID and Product ID.

## License
GPLv3 License
