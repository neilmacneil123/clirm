mod config;
mod export;
mod filter;
mod junk;
mod models;
mod path_safety;
mod protection;
mod registry_util;
mod scanner;
mod tui;
mod uninstall;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "clirm", about = "BCUninstaller-style CLI TUI for Windows")]
struct Cli {
    /// Run without launching the TUI (reserved for future CLI subcommands)
    #[arg(long, hide = true)]
    headless: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    if cli.headless {
        anyhow::bail!("headless mode is not yet implemented");
    }

    let config = config::Config::load()?;
    let protection = protection::ProtectionList::load()?;
    let elevated = is_elevated::is_elevated();

    if !elevated {
        tracing::warn!("not running with administrator privileges; uninstall may fail");
    }

    let mut app = tui::TuiApp::new(config, protection, elevated)?;
    app.run()?;

    Ok(())
}
