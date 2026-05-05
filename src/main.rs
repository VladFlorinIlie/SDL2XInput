mod app;
mod config;
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

    /// Path to a TOML config file for button remapping and axis tweaks.
    /// If not provided, the default identity mapping is used.
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    let mut app = app::App::new(args).await?;
    app.run().await?;
    
    Ok(())
}
