use crate::discovery::{discover_installed, find_by_id, find_repo_root};
use crate::doctor::{run_all, ProbeStatus};
use crate::paths::expand_tilde;
use crate::runner::{run as launch, RunOpts};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "reliquaint", about = "Launch classic games from manifests")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Run {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        windowed: bool,
    },
    Doctor,
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();

    let repo_root = match resolve_repo_root() {
        Some(r) => r,
        None => {
            eprintln!("error: cannot find repo root");
            return ExitCode::FAILURE;
        }
    };
    let games_base = resolve_games_base();
    match cli.command {
        Commands::List => cmd_list(&repo_root, &games_base),
        Commands::Run {
            id,
            dry_run,
            windowed,
        } => cmd_run(&repo_root, &games_base, &id, dry_run, windowed),
        Commands::Doctor => cmd_doctor(&repo_root, &games_base),
    }
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(r) = std::env::var("RELIQUAINT_REPO_ROOT") {
        return Some(PathBuf::from(r));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}

fn resolve_games_base() -> PathBuf {
    if let Ok(base) = std::env::var("RELIQUAINT_GAMES_DIR") {
        return PathBuf::from(base);
    }
    expand_tilde("~/games")
}

fn cmd_list(repo_root: &Path, games_base: &Path) -> ExitCode {
    let mut entries = discover_installed(repo_root, games_base);
    entries.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    for e in &entries {
        let platform = format!("{:?}", e.manifest.platform).to_lowercase();
        println!(
            "{:<15}  {:<6}  {:<20}  {}",
            e.manifest.id, platform, e.manifest.collection, e.manifest.title
        );
    }
    ExitCode::SUCCESS
}

fn cmd_run(repo_root: &Path, games_base: &Path, id: &str, dry_run: bool, windowed: bool) -> ExitCode {
    match find_by_id(repo_root, id) {
        None => {
            eprintln!("error: no manifest found for id '{id}'");
            ExitCode::FAILURE
        }
        Some((path, manifest)) => {
            let opts = RunOpts { dry_run, windowed };
            match launch(&path, &manifest, repo_root, games_base, &opts) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("error: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn cmd_doctor(repo_root: &Path, games_base: &Path) -> ExitCode {
    let results = run_all(repo_root, games_base);
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
