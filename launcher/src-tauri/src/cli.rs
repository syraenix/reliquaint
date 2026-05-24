use crate::catalog::Platform;
use crate::catalog_view::{CatalogView, CatalogViewEntry};
use crate::discovery::find_repo_root;
use crate::doctor::{check_install, ProbeStatus};
use crate::install_record::{self, Install as InstallRec, InstallRecord};
use crate::launch;
use crate::sidecar;
use crate::user_config;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "reliquaint", about = "Launch classic games from manifests")]
struct Cli {
    /// Increase log verbosity. `-v` enables DEBUG, `-vv` enables TRACE.
    /// `RUST_LOG`, if set, overrides this.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List catalog entries with their install status.
    List(ListOpts),
    /// Launch a game by its catalog id.
    Run {
        id: String,
        /// Print the resolved command without spawning the emulator.
        #[arg(long)]
        dry_run: bool,
    },
    /// Register a game's install location with the launcher.
    Install {
        /// Catalog id (run `reliquaint list` to see options).
        id: String,
        /// Directory containing the installed game files.
        path: PathBuf,
        /// Skip the prompt when `[install].expects_files` are missing.
        #[arg(long)]
        force: bool,
    },
    /// Run host-dependency and install-record diagnostics.
    Doctor,
}

#[derive(Args)]
struct ListOpts {
    /// Filter to a single platform.
    #[arg(long, value_enum)]
    platform: Option<PlatformArg>,
    /// Show only entries with an install record.
    #[arg(long)]
    installed: bool,
    /// Show only entries without an install record.
    #[arg(long, conflicts_with = "installed")]
    not_installed: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Tabular)]
    format: Format,
}

#[derive(Clone, Copy, ValueEnum)]
enum PlatformArg {
    Dos,
    Amiga,
}

impl From<PlatformArg> for Platform {
    fn from(p: PlatformArg) -> Self {
        match p {
            PlatformArg::Dos => Self::Dos,
            PlatformArg::Amiga => Self::Amiga,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Tabular,
    Json,
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    crate::logging::init_cli(cli.verbose);
    crate::error::install_panic_hook();

    let repo_root = match resolve_repo_root() {
        Some(r) => r,
        None => {
            eprintln!("error: cannot find repo root");
            return ExitCode::FAILURE;
        }
    };
    match cli.command {
        Commands::List(opts) => match load_view(&repo_root) {
            Ok(view) => cmd_list(&view, &opts),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Run { id, dry_run } => match load_view(&repo_root) {
            Ok(view) => cmd_run(&view, &id, dry_run),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Install { id, path, force } => match load_view(&repo_root) {
            Ok(view) => cmd_install(&view, &id, &path, force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Doctor => match load_view(&repo_root) {
            Ok(view) => cmd_doctor(&view),
            Err(()) => ExitCode::FAILURE,
        },
    }
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(r) = std::env::var("RELIQUAINT_REPO_ROOT") {
        return Some(PathBuf::from(r));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}

/// Assemble a `CatalogView` from the bundled tap + install records.
/// A missing bundled tap is non-fatal (warn and continue with no
/// entries); a structural tap-loading error is fatal.
fn load_view(repo_root: &Path) -> Result<CatalogView, ()> {
    let tap_root = crate::paths::tap_root(repo_root);
    let taps = match crate::tap::load_tap(&tap_root) {
        Ok(t) => vec![t],
        Err(crate::tap::TapError::MissingRoot { .. }) => {
            tracing::warn!(
                root = %tap_root.display(),
                "bundled tap not found; treating catalog as empty"
            );
            Vec::new()
        }
        Err(e) => {
            eprintln!("error: {e}");
            return Err(());
        }
    };
    let installs = crate::install_record::load_all(&crate::paths::installs_dir());
    Ok(CatalogView::assemble(taps, installs))
}

fn cmd_list(view: &CatalogView, opts: &ListOpts) -> ExitCode {
    let filtered: Vec<&CatalogViewEntry> = view
        .all()
        .iter()
        .filter(|e| match opts.platform {
            Some(p) => e.catalog.game.platform == p.into(),
            None => true,
        })
        .filter(|e| {
            if opts.installed {
                e.install.is_some()
            } else if opts.not_installed {
                e.install.is_none()
            } else {
                true
            }
        })
        .collect();

    match opts.format {
        Format::Tabular => print_tabular(&filtered),
        Format::Json => match print_json(&filtered) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: failed to serialize JSON: {e}");
                return ExitCode::FAILURE;
            }
        },
    }
    ExitCode::SUCCESS
}

fn print_tabular(entries: &[&CatalogViewEntry]) {
    use std::collections::BTreeMap;
    if entries.is_empty() {
        return;
    }
    let mut by_collection: BTreeMap<String, Vec<&CatalogViewEntry>> = BTreeMap::new();
    for e in entries {
        let key = e
            .catalog
            .game
            .collection
            .clone()
            .unwrap_or_else(|| "(no collection)".into());
        by_collection.entry(key).or_default().push(e);
    }
    let mut first = true;
    for (collection, items) in by_collection {
        if !first {
            println!();
        }
        first = false;
        println!("{collection}");
        for e in items {
            let status = if e.install.is_some() {
                "installed"
            } else {
                "not installed"
            };
            let year = e
                .catalog
                .meta
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "----".into());
            println!(
                "  {:<14}  {:<35}  {:<5}  {status}",
                e.catalog.game.id, e.catalog.game.title, year
            );
        }
    }
}

fn print_json(entries: &[&CatalogViewEntry]) -> Result<(), serde_json::Error> {
    #[derive(serde::Serialize)]
    struct JsonRow<'a> {
        id: &'a str,
        title: &'a str,
        platform: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        year: Option<u32>,
        tap_id: &'a str,
        installed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        install_path: Option<String>,
    }

    let rows: Vec<JsonRow> = entries
        .iter()
        .map(|e| JsonRow {
            id: &e.catalog.game.id,
            title: &e.catalog.game.title,
            platform: match e.catalog.game.platform {
                Platform::Dos => "dos",
                Platform::Amiga => "amiga",
            },
            collection: e.catalog.game.collection.as_deref(),
            year: e.catalog.meta.year,
            tap_id: &e.tap_id,
            installed: e.install.is_some(),
            install_path: e
                .install
                .as_ref()
                .map(|i| i.install.install_path.to_string_lossy().into_owned()),
        })
        .collect();

    let output = serde_json::to_string_pretty(&rows)?;
    println!("{output}");
    Ok(())
}

fn cmd_run(view: &CatalogView, id: &str, dry_run: bool) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry for '{id}'");
            eprintln!("hint: run 'reliquaint list' to see available ids.");
            return ExitCode::FAILURE;
        }
    };

    let install = match entry.install.as_ref() {
        Some(i) => i,
        None => {
            eprintln!("error: '{id}' has no installation record");
            eprintln!("hint: run 'reliquaint install {id} <path-to-game-files>' first.");
            return ExitCode::FAILURE;
        }
    };

    let user_config = user_config::load_or_default(&crate::paths::user_config_path());

    let plan = match entry.catalog.game.platform {
        Platform::Dos => {
            launch::compose_dosbox(&entry.catalog, &entry.source_path, install, &user_config)
        }
        Platform::Amiga => {
            launch::compose_fs_uae(&entry.catalog, &entry.source_path, install, &user_config)
        }
    };
    let plan = match plan {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if dry_run {
        for s in &plan.sidecars {
            println!("[sidecar:{}] {}", s.name, s.command.display_line());
        }
        println!("[primary] {}", plan.primary.display_line());
        return ExitCode::SUCCESS;
    }

    match sidecar::run_plan(plan) {
        Ok(status) => match status.code() {
            Some(code) if code >= 0 => ExitCode::from(code.min(255) as u8),
            _ => ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_install(view: &CatalogView, id: &str, path: &Path, force: bool) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry for '{id}'");
            eprintln!("hint: run 'reliquaint list' to see available ids.");
            return ExitCode::FAILURE;
        }
    };

    // canonicalize handles both existence and absolutization.
    let abs_path = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if !abs_path.is_dir() {
        eprintln!("error: {} is not a directory", abs_path.display());
        return ExitCode::FAILURE;
    }

    let missing = missing_expects_files(&abs_path, &entry.catalog.install.expects_files);
    if !missing.is_empty() {
        eprintln!(
            "warning: expected files not found in {}:",
            abs_path.display()
        );
        for f in &missing {
            eprintln!("  - {f}");
        }
        if !force && !prompt_yes_no("Write install record anyway? [y/N]") {
            eprintln!("aborted");
            return ExitCode::FAILURE;
        }
    }

    let installed_at = match toml::value::Datetime::from_str(&now_iso8601()) {
        Ok(dt) => dt,
        Err(e) => {
            eprintln!("error: failed to construct timestamp: {e}");
            return ExitCode::FAILURE;
        }
    };

    let record = InstallRecord {
        schema_version: 1,
        install: InstallRec {
            catalog_id: entry.catalog.game.id.clone(),
            tap: entry.tap_id.clone(),
            install_path: abs_path.clone(),
            installed_at,
        },
    };

    let installs_dir = crate::paths::installs_dir();
    if let Err(e) = std::fs::create_dir_all(&installs_dir) {
        eprintln!(
            "error: cannot create installs directory {}: {e}",
            installs_dir.display()
        );
        return ExitCode::FAILURE;
    }
    let record_path = installs_dir.join(format!("{}.toml", entry.catalog.game.id));
    if let Err(e) = install_record::write(&record, &record_path) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    println!("installed {} at {}", entry.catalog.game.id, abs_path.display());
    ExitCode::SUCCESS
}

/// Case-insensitive check for each expected filename at the top level of
/// `install_dir`. Returns the names that weren't found.
fn missing_expects_files(install_dir: &Path, expected: &[String]) -> Vec<String> {
    let entries: Vec<String> = match std::fs::read_dir(install_dir) {
        Ok(read) => read
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect(),
        Err(_) => Vec::new(),
    };
    expected
        .iter()
        .filter(|exp| !entries.iter().any(|e| e.eq_ignore_ascii_case(exp)))
        .cloned()
        .collect()
}

fn prompt_yes_no(question: &str) -> bool {
    use std::io::{BufRead, Write};
    eprint!("{question} ");
    let _ = std::io::stderr().flush();
    let stdin = std::io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }
    line.trim().eq_ignore_ascii_case("y")
}

/// Current UTC time formatted as ISO 8601 (YYYY-MM-DDTHH:MM:SSZ), built
/// from libc::gmtime_r to avoid pulling in chrono / time crates.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: gmtime_r is reentrant and takes valid pointers to a
    // time_t and a tm we own.
    unsafe {
        libc::gmtime_r(&secs, &mut tm);
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

fn cmd_doctor(view: &CatalogView) -> ExitCode {
    let user_config = user_config::load_or_default(&crate::paths::user_config_path());
    let results = check_install(view, &user_config);
    let mut any_missing = false;
    for r in &results {
        let label = match r.status {
            ProbeStatus::Ok => "ok     ",
            ProbeStatus::Missing => "missing",
            ProbeStatus::Unknown => "unknown",
        };
        let detail = r.detail.as_deref().unwrap_or("");
        println!("[ {label} ] {}  {}", r.name, detail);
        if r.status == ProbeStatus::Missing {
            any_missing = true;
        }
    }
    if any_missing {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}
