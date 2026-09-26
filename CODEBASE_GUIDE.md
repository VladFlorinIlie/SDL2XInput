# SDL2XInput: Codebase Guide

This guide is written specifically for developers familiar with Java, TypeScript, and C, but who might not have extensive experience with Rust. It provides a comprehensive breakdown of how this application works under the hood, how the components interact, and how key Rust concepts map to languages you already know.

## High-Level Architecture

`sdl2xinput` is a high-performance input translation layer. Its primary goal is to take a physical gamepad (like a PlayStation DualSense), read its inputs via **SDL3**, and mathematically translate those inputs into virtual USB devices (Xbox 360 Controller, Mouse, Keyboard) using a library called **VIIPER** (which implements the USB/IP protocol).

Because input polling needs to happen at extremely high speeds (250Hz) with minimal latency, the hot-loops are heavily optimized. There is no heap allocation (like `new Object()` in Java) inside the polling loop, and we cache states to ensure we only make expensive kernel-level syscalls when absolutely necessary.

## Rust Concepts for Java/TS/C Developers

Before diving into the files, here is a quick translation guide for some Rust paradigms you will see:

1. **`struct` and `impl` (Classes)**: Rust does not have classes. Data is stored in `struct`s (like C), and methods are attached to them using `impl` blocks.
2. **`Trait`s (Interfaces)**: Similar to Java interfaces. You implement a trait for a struct to give it shared behavior.
3. **`Option<T>` (No Nulls)**: Rust does not have `null`. If a value might be missing, it is wrapped in an `Option`. To get the value, you use `match` or `if let Some(val) = ...`. This forces the developer to handle "null pointer" scenarios at compile time.
4. **`Result<T, E>` (No Exceptions)**: Rust does not throw exceptions. Functions that can fail return a `Result`. You will often see the `?` operator at the end of function calls (e.g., `do_something()?`). This means "if this fails, immediately return the error up the call stack," acting very much like a checked exception.
5. **Ownership and Borrowing (`&` and `&mut`)**: Rust has no Garbage Collector (unlike Java/TS) and you don't manually `free()` memory (like C). Instead, variables "own" their data. When they go out of scope, memory is freed. If you want to pass data without giving up ownership, you pass a reference (`&`). If you want to modify it, you pass a mutable reference (`&mut`). 
6. **`unsafe { ... }`**: Rust guarantees memory safety, *except* when you talk to C code. Calling a C function requires an `unsafe` block, acknowledging that the compiler can no longer guarantee the C code isn't doing something dangerous.

---

## File-by-File Breakdown

### 1. `src/main.rs` (The Entry Point)
Like `public static void main` in Java, this is where the application starts.
- It uses the `clap` crate (similar to `commander` in Node.js) to parse command-line arguments.
- It initializes tracing (logging).
- It creates an instance of `App` and calls `.run()`.

### 2. `src/app.rs` (The Core Loop)
This holds the `App` struct, which is the central state manager.
- **Initialization**: Sets up the SDL3 context and connects to the VIIPER USB server.
- **`active_sessions`**: A HashMap (Dictionary) tracking currently connected physical gamepads. When you plug in a DualSense, it gets assigned an ID, and an `ActiveSession` is spawned for it.
- **`run()`**: The main event loop. It loops infinitely, doing two things:
  1. Checks for SDL events (like a controller being plugged in or a touchpad swipe).
  2. Calls `self.tick_sessions()` every 4ms (at 250Hz).
- **Graceful Shutdown**: It catches Ctrl+C signals and system tray exits to cleanly shut down the virtual USB devices before the process exits.

### 3. `src/session.rs` (Virtual Device Lifecycle)
An `ActiveSession` represents one physical gamepad and its corresponding virtual devices.
- **`new()`**: When a controller connects, this function asks VIIPER to spawn a Virtual Xbox 360 Controller on a new USB bus. If configured, it also spawns a Virtual Mouse and Virtual Keyboard on the *exact same bus* (creating a Composite USB Device in Windows).
- **Touchpad Handling**: Tracks finger taps, drags, and movement mathematically to calculate mouse cursor deltas (`dx`, `dy`).
- **`update_and_send()`**: The "Hot Loop". This runs 250 times a second. It reads the physical gamepad, calls `mapping::update_from_sdl_gamepad` to build the new state, and sends it to VIIPER.
  - *Performance Optimization*: It maintains a cache (`last_xbox_state`, etc.). If you leave the controller idle, the state doesn't change, and it skips sending data to VIIPER. This eliminates thousands of redundant kernel IOCTLs.

### 4. `src/mapping.rs` (Business Logic)
This is a pure, stateless data transformation module.
- **`update_from_sdl_gamepad()`**: It takes the raw physical SDL3 Gamepad state and populates a C-compatible `Xbox360DeviceState` struct.
- **Deadzones**: Applies the hardware deadzone to the analog sticks and triggers to eliminate micro-jitter (stick drift).
- **Exclusive Routing**: It checks the parsed `config.toml` mappings. If you pressed the `X` button (physical `south`) and mapped it to the `Space` bar, it flips the bit for `Space` in the `KeyboardDeviceState` and *explicitly skips* mapping it to the Xbox `A` button.

### 5. `src/viiper_bridge.rs` (The C/Go Interop Layer)
This is where Rust talks to the underlying VIIPER library.
- **FFI Definitions**: You'll see `#[repr(C)]` on structs like `Xbox360DeviceState`. This tells the Rust compiler to arrange the memory exactly like a C struct, preventing Rust from optimizing or reordering the fields.
- **`unsafe extern "C"`**: Defines the C function signatures imported from `libviiper.h`.
- **`ViiperManager`**: A safe Rust wrapper around the unsafe C calls. It translates Rust `Result`s into the appropriate C calls, so the rest of the application doesn't have to write `unsafe` blocks.

### 6. `src/config.rs`
Uses the `serde` crate (similar to Jackson in Java or `JSON.parse` in TS) to automatically deserialize the `config.toml` file into strongly-typed Rust structs.

### 7. `src/keys.rs`
A massive lookup table. USB HID (Human Interface Device) keyboards do not send ASCII characters; they send hexadecimal "Usage Codes" (e.g., `0x04` means the 'A' key). This file provides a giant `match` statement (switch statement) to translate human-readable strings from the config like `"Enter"` or `"F1"` into the correct integer bit-index needed by the VIIPER virtual keyboard.

### 8. `build.rs` (The Build Script)
Rust allows you to run a script *before* the compiler runs.
- It tells `cargo` (the Rust build tool) where to find `libviiper.a` (the statically compiled Go library).
- It links against core Windows libraries (`ws2_32`, `userenv`, `bcrypt`) which the Go runtime requires to function inside the compiled Windows `.exe`.

---

## The Request Lifecycle (Example)

Here is what happens in milliseconds when you press the physical 'Square' button on a DualSense:

1. **Hardware**: DualSense sends a Bluetooth/USB interrupt. Windows passes it to SDL3.
2. **`app.rs`**: The 250Hz loop wakes up.
3. **`session.rs`**: `ActiveSession::update_and_send()` calls into `mapping.rs`.
4. **`mapping.rs`**: Reads `gp.button(West)` as `true`. It sets the `X` button bit inside `Xbox360DeviceState.buttons` using bitwise OR (`|=`).
5. **`session.rs`**: Compares `new_state != last_xbox_state`. They are different! It caches the new state and calls `viiper.set_xbox360_state()`.
6. **`viiper_bridge.rs`**: The safe Rust wrapper makes an `unsafe` FFI call to the C function `SetXbox360DeviceState()`.
7. **VIIPER (Go)**: The Go library queues this state change. Because we set `write_batch_flush_interval_ms` to 4ms, the Go routine flushes it.
8. **Windows Kernel**: The Go library sends an IOCTL down to the `usbipd` Windows driver, which tells the OS that the Virtual Xbox 360 controller's 'X' button is now pressed.
9. **Game**: The game reads XInput and sees the button press.
