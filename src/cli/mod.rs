use crate::config::Config;
use crate::export;
use crate::filter::bcul::BculList;
use crate::junk::{delete_junk_item, scan_for_app};
use crate::models::ApplicationEntry;
use crate::protection::ProtectionList;
use crate::scanner;
use crate::startup;
use crate::uninstall;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "clirm", about = "BCUninstaller-style CLI TUI for Windows")]
pub struct Cli {
    /// Import a BCUninstaller .bcul filter list (TUI mode)
    #[arg(long, value_name = "FILE")]
    pub bcul: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// List installed applications (BCU-compatible)
    List(BcuFlags),
    /// Export installed applications to JSON
    Export {
        /// Output file path (.json or .csv)
        file: PathBuf,
        #[command(flatten)]
        flags: BcuFlags,
    },
    /// Uninstall applications matching a .bcul list
    Uninstall {
        /// Path to .bcul uninstall list
        list: PathBuf,
        #[command(flatten)]
        flags: BcuFlags,
        /// Clean leftovers after uninstall (VeryGood, Good, Questionable, Bad, Unknown)
        #[arg(long = "J", value_name = "LEVEL", num_args = 0..=1, default_missing_value = "VeryGood")]
        junk: Option<String>,
    },
    /// Scan and list startup entries
    Scan {
        #[arg(long, short = 'V', help = "Verbose output")]
        verbose: bool,
    },
}

#[derive(Debug, Parser, Clone, Default)]
pub struct BcuFlags {
    /// Use quiet uninstallers wherever possible
    #[arg(long = "Q", short = 'q')]
    pub quiet: bool,
    /// Unattended mode (no confirmation prompts)
    #[arg(long = "U", short = 'u')]
    pub unattended: bool,
    /// Verbose logging
    #[arg(long = "V", short = 'v')]
    pub verbose: bool,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(CliCommand::List(flags)) => cmd_list(flags),
        Some(CliCommand::Export { file, flags }) => cmd_export(&file, flags),
        Some(CliCommand::Uninstall { list, flags, junk }) => cmd_uninstall(&list, flags, junk),
        Some(CliCommand::Scan { verbose }) => cmd_scan(verbose),
        None => Ok(()),
    }
}

fn load_apps(verbose: bool) -> Result<Vec<ApplicationEntry>> {
    let config = Config::load()?;
    let protection = ProtectionList::load()?;
    if verbose {
        eprintln!("Scanning applications...");
    }
    let apps = scanner::scan_applications(&config, &protection)?;
    if verbose {
        eprintln!("Found {} applications.", apps.len());
    }
    Ok(apps)
}

fn cmd_list(flags: BcuFlags) -> Result<()> {
    let apps = load_apps(flags.verbose)?;

    println!(
        "{:<40}  {:<20}  {:<40}",
        "Display Name", "Version", "Source"
    );
    println!("{}", "-".repeat(102));

    for app in &apps {
        let name = truncate(&app.display_name, 40);
        let version = truncate(app.version.as_deref().unwrap_or(""), 20);
        let source = truncate(app.source.label(), 40);
        println!("{name:<40}  {version:<20}  {source:<40}");
    }

    Ok(())
}

fn cmd_export(path: &Path, flags: BcuFlags) -> Result<()> {
    let apps = load_apps(flags.verbose)?;
    if flags.verbose {
        eprintln!("Exporting to {}...", path.display());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("json")
        .to_ascii_lowercase();

    match ext.as_str() {
        "csv" => export::export_csv(&apps, path)?,
        "json" | "xml" => export::export_json(&apps, path)?,
        other => bail!("unsupported export format: {other}"),
    }

    if flags.verbose {
        eprintln!("Export complete.");
    }
    Ok(())
}

fn cmd_uninstall(list_path: &Path, flags: BcuFlags, junk_level: Option<String>) -> Result<()> {
    let bcul = BculList::from_file(list_path)?;
    let apps = load_apps(flags.verbose)?;
    let matched: Vec<ApplicationEntry> = bcul
        .matching_indices(&apps)
        .into_iter()
        .map(|i| apps[i].clone())
        .collect();

    if matched.is_empty() {
        println!("No applications matched the supplied uninstall list.");
        return Ok(());
    }

    println!("{} application(s) matched:", matched.len());
    for app in &matched {
        println!("  - {}", app.display_name);
    }

    if !flags.unattended {
        print!("Continue? [Y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled by the user.");
            std::process::exit(1223);
        }
    } else {
        eprintln!("WARNING: Running in unattended mode.");
    }

    if !is_elevated::is_elevated() {
        bail!("administrator privileges required for uninstall");
    }

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for app in &matched {
        if !app.can_uninstall() {
            eprintln!("Skipping (not uninstallable): {}", app.display_name);
            failed += 1;
            continue;
        }

        if flags.verbose {
            eprintln!("Uninstalling: {}", app.display_name);
        }

        let _ = flags.quiet;
        let result = uninstall::uninstall_application(app);

        match result {
            Ok(()) => {
                succeeded += 1;
                if flags.verbose {
                    eprintln!("  OK: {}", app.display_name);
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("  FAILED: {} — {e}", app.display_name);
            }
        }
    }

    println!("Uninstall complete: {succeeded} ok, {failed} failed");

    if let Some(level) = junk_level {
        cleanup_junk(&matched, &level, flags.unattended, flags.verbose)?;
    }

    Ok(())
}

fn cleanup_junk(
    apps: &[ApplicationEntry],
    level: &str,
    unattended: bool,
    verbose: bool,
) -> Result<()> {
    let min_confidence = match level {
        "VeryGood" => 80u8,
        "Good" => 60,
        "Questionable" => 40,
        "Bad" => 20,
        "Unknown" => 0,
        other => bail!("invalid junk confidence level: {other}"),
    };

    if verbose {
        eprintln!("Scanning for leftovers (min confidence: {level})...");
    }

    let mut all_junk = Vec::new();
    for app in apps {
        if let Ok(items) = scan_for_app(app) {
            all_junk.extend(items);
        }
    }

    all_junk.retain(|item| junk_confidence(item) >= min_confidence);

    if all_junk.is_empty() {
        println!("No remaining junk found.");
        return Ok(());
    }

    println!("{} junk item(s) to delete:", all_junk.len());
    for item in &all_junk {
        println!("  [{}] {}", item.kind.label(), item.path);
    }

    if !unattended {
        print!("Delete junk? [Y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Junk cleanup cancelled.");
            return Ok(());
        }
    }

    for item in &all_junk {
        if let Err(e) = delete_junk_item(item) {
            eprintln!("Failed to delete {}: {e}", item.path);
        }
    }

    Ok(())
}

fn junk_confidence(item: &crate::models::JunkItem) -> u8 {
    match item.kind {
        crate::models::JunkKind::RegistryKey => 90,
        crate::models::JunkKind::Directory => 70,
        crate::models::JunkKind::File => 60,
    }
}

fn cmd_scan(verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("Scanning startup entries...");
    }
    let entries = startup::scan_startup_entries()?;
    println!("{:<30}  {:<20}  Command", "Name", "Location");
    println!("{}", "-".repeat(100));
    for entry in &entries {
        let name = truncate(&entry.name, 30);
        let location = truncate(entry.location.label(), 20);
        let command = truncate(&entry.command, 48);
        println!("{name:<30}  {location:<20}  {command}");
    }
    println!("\n{} startup entries found.", entries.len());
    Ok(())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("hello", 10), "hello");
        assert!(truncate("hello world example text", 10).chars().count() <= 10);
    }
}
