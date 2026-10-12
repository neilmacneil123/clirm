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

pub const EXIT_CANCELLED: i32 = 1223;
pub const EXIT_FAILURE: i32 = 1;

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
    /// Export installed applications to JSON, CSV, or XML
    Export {
        /// Output file path (.json, .csv, or .xml)
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
    /// List startup entries (not in BCU-console; use instead of ambiguous `scan`)
    Startup {
        #[arg(long = "V", short = 'v', help = "Verbose output")]
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

/// Normalize BCU-console slash flags (`/Q`, `/V`, `/J=Level`) for clap parsing.
pub fn normalize_bcu_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|arg| normalize_bcu_token(arg.as_ref()))
        .collect()
}

fn normalize_bcu_token(arg: &str) -> String {
    if arg.eq_ignore_ascii_case("/Q") {
        return "--Q".into();
    }
    if arg.eq_ignore_ascii_case("/U") {
        return "--U".into();
    }
    if arg.eq_ignore_ascii_case("/V") {
        return "--V".into();
    }
    if arg.eq_ignore_ascii_case("/J") {
        return "--J".into();
    }
    if let Some(level) = arg.strip_prefix("/J=").or_else(|| arg.strip_prefix("/j=")) {
        return format!("--J={level}");
    }
    arg.to_string()
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(CliCommand::List(flags)) => cmd_list(flags),
        Some(CliCommand::Export { file, flags }) => cmd_export(&file, flags),
        Some(CliCommand::Uninstall { list, flags, junk }) => cmd_uninstall(&list, flags, junk),
        Some(CliCommand::Startup { verbose }) => cmd_startup(verbose),
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
        let name = truncate_bcu(&app.display_name, 40);
        let version = truncate_bcu(app.version.as_deref().unwrap_or(""), 20);
        let source = truncate_bcu(app.source.label(), 40);
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
        "json" => export::export_json(&apps, path)?,
        "xml" => export::export_xml(&apps, path)?,
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

    if !is_elevated::is_elevated() {
        bail!("administrator privileges required for uninstall");
    }

    if !flags.unattended {
        print!("Continue? [Y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled by the user.");
            std::process::exit(EXIT_CANCELLED);
        }
    } else {
        eprintln!("WARNING: Running in unattended mode.");
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
            eprintln!(
                "Uninstalling: {} ({})",
                app.display_name,
                if flags.quiet { "quiet" } else { "loud" }
            );
        }

        let result = uninstall::uninstall_application_with_options(app, flags.quiet);

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

    if failed > 0 {
        std::process::exit(EXIT_FAILURE);
    }

    if let Some(level) = junk_level {
        let junk_failed = cleanup_junk(&matched, &level, flags.unattended, flags.verbose)?;
        if junk_failed > 0 {
            std::process::exit(EXIT_FAILURE);
        }
    }

    Ok(())
}

fn cleanup_junk(
    apps: &[ApplicationEntry],
    level: &str,
    unattended: bool,
    verbose: bool,
) -> Result<usize> {
    let min_confidence = confidence_threshold(level)?;

    if verbose {
        eprintln!("Scanning for leftovers (min confidence: {level})...");
    }

    let mut all_junk = Vec::new();
    for app in apps {
        if let Ok(items) = scan_for_app(app) {
            all_junk.extend(items);
        }
    }

    all_junk.retain(|item| item.confidence >= min_confidence);

    if all_junk.is_empty() {
        println!("No remaining junk found.");
        return Ok(0);
    }

    println!("{} junk item(s) to delete:", all_junk.len());
    for item in &all_junk {
        println!(
            "  [{}] {} (confidence {})",
            item.kind.label(),
            item.path,
            item.confidence
        );
    }

    if !unattended {
        print!("Delete junk? [Y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Junk cleanup cancelled by the user.");
            std::process::exit(EXIT_CANCELLED);
        }
    }

    let mut failed = 0usize;
    for item in &all_junk {
        if let Err(e) = delete_junk_item(item) {
            failed += 1;
            eprintln!("Failed to delete {}: {e}", item.path);
        }
    }

    Ok(failed)
}

fn confidence_threshold(level: &str) -> Result<u8> {
    match level {
        "VeryGood" => Ok(80),
        "Good" => Ok(60),
        "Questionable" => Ok(40),
        "Bad" => Ok(20),
        "Unknown" => Ok(0),
        other => bail!("invalid junk confidence level: {other}"),
    }
}

fn cmd_startup(verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("Scanning startup entries...");
    }
    let entries = startup::scan_startup_entries()?;
    println!("{:<30}  {:<20}  Command", "Name", "Location");
    println!("{}", "-".repeat(100));
    for entry in &entries {
        let name = truncate_bcu(&entry.name, 30);
        let location = truncate_bcu(entry.location.label(), 20);
        let command = truncate_bcu(&entry.command, 48);
        println!("{name:<30}  {location:<20}  {command}");
    }
    println!("\n{} startup entries found.", entries.len());
    Ok(())
}

/// BCU-console truncates by .NET string length (UTF-16 code units); ASCII aligns with byte length.
fn truncate_bcu(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        let end = value.floor_char_boundary(max.saturating_sub(1));
        format!("{}…", &value[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_slash_flags() {
        let args = normalize_bcu_args(["clirm", "list", "/V", "/Q"]);
        assert_eq!(args, vec!["clirm", "list", "--V", "--Q"]);
    }

    #[test]
    fn normalize_junk_flag_with_level() {
        let args = normalize_bcu_args(["clirm", "uninstall", "list.bcul", "/J=VeryGood", "/U"]);
        assert_eq!(
            args,
            vec!["clirm", "uninstall", "list.bcul", "--J=VeryGood", "--U"]
        );
    }

    #[test]
    fn truncate_bcu_uses_byte_length() {
        assert_eq!(truncate_bcu("hello", 10), "hello");
        assert_eq!(truncate_bcu("hello world!!!", 8), "hello w…");
    }

    #[test]
    fn truncate_bcu_handles_multibyte_utf8() {
        assert_eq!(truncate_bcu("€abc", 3), "…");
        assert_eq!(truncate_bcu("€abc", 4), "€…");
        assert_eq!(truncate_bcu("日本語アプリ", 7), "日本…");
    }

    #[test]
    fn confidence_thresholds_match_bcu_levels() {
        assert_eq!(confidence_threshold("VeryGood").unwrap(), 80);
        assert_eq!(confidence_threshold("Good").unwrap(), 60);
        assert_eq!(confidence_threshold("Unknown").unwrap(), 0);
        assert!(confidence_threshold("NotReal").is_err());
    }

    #[test]
    fn slash_flags_parse_with_clap() {
        let args = normalize_bcu_args(["clirm", "list", "/V"]);
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(CliCommand::List(flags)) => assert!(flags.verbose),
            _ => panic!("expected list command"),
        }
    }
}
