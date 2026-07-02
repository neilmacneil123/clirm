mod cli;
mod config;
mod export;
mod filter;
mod junk;
mod models;
mod path_safety;
mod protection;
mod ratings;
mod registry_util;
mod scanner;
mod startup;
mod tui;
mod uninstall;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = cli::normalize_bcu_args(std::env::args());
    let cli = cli::Cli::parse_from(args);

    if cli.command.is_some() {
        return cli::run(cli);
    }

    let config = config::Config::load()?;
    let protection = protection::ProtectionList::load()?;
    let elevated = is_elevated::is_elevated();

    if !elevated {
        tracing::warn!("not running with administrator privileges; uninstall may fail");
    }

    let bcul_filter = cli
        .bcul
        .as_ref()
        .map(|path| filter::bcul::BculList::from_file(path))
        .transpose()?;

    let mut app = tui::TuiApp::new(config, protection, elevated, bcul_filter)?;
    app.run()?;

    Ok(())
}
