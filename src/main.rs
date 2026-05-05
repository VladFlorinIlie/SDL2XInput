mod app;
mod mapping;
mod viiper_bridge;
mod session;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The Viiper Server Address
    #[arg(short, long, default_value = "127.0.0.1:3242")]
    pub viiper_address: String,

    /// Maximum number of active controllers allowed
    #[arg(short, long, default_value_t = 1)]
    pub max_controllers: usize,

    /// Block a specific device by VID:PID (hex, e.g. 045E:028E). Can be repeated.
    /// Use this to filter out known virtual controller identities instead of relying on name matching.
    #[arg(long = "filter-device", value_name = "VID:PID")]
    pub filter_devices: Vec<String>,

    /// Start with an empty device filter (no default Xbox 360 VID:PID block).
    /// Use this if HidHide is already hiding your physical controller, or if you
    /// need to pass Xbox 360 hardware through without any automatic filtering.
    #[arg(long, default_value_t = false)]
    pub empty_device_filter: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    let mut app = app::App::new(args).await?;
    app.run().await?;
    
    Ok(())
}
