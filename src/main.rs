mod app;
mod config;
mod mapping;
mod viiper_bridge;
mod session;
pub mod keys;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Maximum number of active controllers allowed
    #[arg(short, long, default_value_t = 1)]
    pub max_controllers: usize,

    /// Path to a TOML config file for button remapping and axis tweaks.
    /// If not provided, the default identity mapping is used.
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<std::path::PathBuf>,

    /// Input polling rate in Hz (1-1000). Higher values give lower latency but use more CPU.
    #[arg(short, long, default_value_t = 250, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub polling_rate: u32,

    /// Block a specific device by VID:PID (hex, e.g. 045E:028E). Can be repeated.
    /// Use this to filter out known virtual controller identities instead of relying on name matching.
    #[arg(long = "filter-device", value_name = "VID:PID")]
    pub filter_devices: Vec<String>,

    /// Start with an empty device filter (no default Xbox 360 VID:PID block).
    /// Use this if HidHide is already hiding your physical controller, or if you
    /// need to pass Xbox 360 hardware through without any automatic filtering.
    #[arg(long, default_value_t = false)]
    pub empty_device_filter: bool,

    /// Do not hide the console window or create a system tray icon.
    /// By default, the application runs in the background with a system tray icon.
    #[arg(long, default_value_t = false)]
    pub no_tray: bool,

    /// Hardware deadzone applied to the analog sticks to filter out micro-jitter from highly sensitive controllers.
    /// Set to 0 to disable the deadzone completely.
    #[arg(short, long, default_value_t = 1000)]
    pub deadzone: i16,

    /// IP address and port for the USBIP server (e.g. 127.0.0.1:3241).
    /// Defaults to NULL (usually 127.0.0.1:3241) if not specified.
    #[arg(long, value_name = "ADDR")]
    pub usb_server_addr: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut is_double_click = false;
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::GetConsoleProcessList;
        let mut process_list = [0u32; 2];
        if GetConsoleProcessList(process_list.as_mut_ptr(), 2) <= 1 {
            is_double_click = true;
        }
    }

    if is_double_click && !args.no_tray {
        #[cfg(target_os = "windows")]
        unsafe {
            windows_sys::Win32::System::Console::FreeConsole();
        }

        let exe_path = std::env::current_exe()?;
        let log_path = exe_path.with_file_name("sdl2xinput.log");
        let file = std::fs::OpenOptions::new().create(true).append(true).open(log_path)?;
        tracing_subscriber::fmt()
            .with_writer(std::sync::Arc::new(file))
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt().init();
    }
    
    let mut app = app::App::new(args)?;
    app.run()?;
    
    Ok(())
}
