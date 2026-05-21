use crate::discovery::{discover_catalog, find_by_id, find_repo_root};
use crate::doctor::{run_all, ProbeStatus};
use crate::runner::{run as launch, RunOpts};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "classic-launcher", about = "Launch classic games from manifests")]
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
            eprintln!(
                "error: cannot find repo root (no parent directory contains both dos/ and amiga/)"
            );
            eprintln!("Run classic-launcher from within the game guides repository.");
            return ExitCode::FAILURE;
        }
    };

    match cli.command {
        Commands::List => cmd_list(&repo_root),
        Commands::Run {
            id,
            dry_run,
            windowed,
        } => cmd_run(&repo_root, &id, dry_run, windowed),
        Commands::Doctor => cmd_doctor(&repo_root),
    }
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(override_root) = std::env::var("CLASSIC_LAUNCHER_REPO_ROOT") {
        return Some(PathBuf::from(override_root));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}

fn cmd_list(repo_root: &Path) -> ExitCode {
    let mut manifests = discover_catalog(repo_root);
    manifests.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
    for (_, m) in &manifests {
        let platform = format!("{:?}", m.platform).to_lowercase();
        println!(
            "{:<15}  {:<6}  {:<20}  {}",
            m.id, platform, m.collection, m.title
        );
    }
    ExitCode::SUCCESS
}

fn cmd_run(repo_root: &Path, id: &str, dry_run: bool, windowed: bool) -> ExitCode {
    match find_by_id(repo_root, id) {
        None => {
            eprintln!("error: no manifest found for id '{id}'");
            ExitCode::FAILURE
        }
        Some((path, manifest)) => {
            let games_base = crate::paths::expand_tilde("~/games");
            let opts = RunOpts { dry_run, windowed };
            match launch(&path, &manifest, repo_root, &games_base, &opts) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("error: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn cmd_doctor(repo_root: &Path) -> ExitCode {
    let games_base = crate::paths::expand_tilde("~/games");
    let results = run_all(repo_root, &games_base);
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
