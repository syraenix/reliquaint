use crate::catalog::Platform;
use crate::catalog_view::{CatalogView, CatalogViewEntry};
use crate::doctor::{check_install, ProbeStatus};
use crate::game_install;
use crate::install_record;
use crate::installer;
use crate::launch;
use crate::paths::find_repo_root;
use crate::sidecar;
use crate::user_config;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "reliquaint",
    version,
    about = "Launch classic games from manifests"
)]
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
    /// Install a game: copy/extract a source into the managed library and
    /// register it. The source may be a directory, a DOS `.exe` installer,
    /// or an Amiga `.adf`/`.hdf`/`.rp9` disk image.
    Install {
        /// Catalog id (run `reliquaint list` to see options).
        id: String,
        /// Source directory, `.exe`, or disk image to install from.
        source: PathBuf,
        /// Library directory to install into (default `~/games`). The game
        /// lands at `<dir>/<id>`.
        #[arg(long)]
        dest: Option<PathBuf>,
        /// Register the install even if `[install].expects_files` are
        /// missing afterward.
        #[arg(long)]
        force: bool,
    },
    /// Scan a base directory (default `~/games`) for per-id subfolders
    /// matching catalog entries and register install records for each.
    /// Intended for users coming from the pre-redesign `~/games/<id>/`
    /// layout.
    MigrateInstalls {
        /// Base directory containing per-game subdirectories.
        #[arg(long, default_value = "~/games")]
        base: String,
    },
    /// Run host-dependency and install-record diagnostics.
    Doctor,
    /// Add a game from a directory the user already has on disk. Runs
    /// platform/entry-point detection, composes a draft manifest, and
    /// writes it to the user tap on confirmation.
    Add {
        /// Directory containing the game.
        path: PathBuf,
        /// Skip the confirm-or-edit prompt and accept the draft as-is.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Override the detected platform. Useful when the heuristic
        /// is ambiguous, or when running non-interactively.
        #[arg(long, value_enum)]
        platform: Option<PlatformArg>,
    },
    /// Print on-disk paths for a user-created entry's manifest,
    /// sibling config, and install record. Useful for hand-editing.
    Where {
        /// Catalog id.
        id: String,
    },
    /// Remove a user-created entry. Cascades the manifest, sibling
    /// config, and install record. Refuses to operate on bundled
    /// entries.
    Remove {
        /// Catalog id.
        id: String,
        /// Skip the confirmation prompt.
        #[arg(long)]
        force: bool,
    },
    /// Print a submission-ready version of a user-created entry that
    /// can be opened as a PR against the bundled tap. Validates the
    /// manifest and warns on completeness gaps.
    Submit {
        /// Catalog id.
        id: String,
        /// Copy the manifest body to the clipboard (requires
        /// `wl-copy` or `xclip` on PATH).
        #[arg(long)]
        clipboard: bool,
    },
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
        Commands::Install {
            id,
            source,
            dest,
            force,
        } => match load_view(&repo_root) {
            Ok(view) => cmd_install(&view, &id, &source, dest.as_deref(), force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::MigrateInstalls { base } => match load_view(&repo_root) {
            Ok(view) => cmd_migrate_installs(&view, &base),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Doctor => match load_view(&repo_root) {
            Ok(view) => cmd_doctor(&view),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Add {
            path,
            yes,
            platform,
        } => cmd_add(&repo_root, &path, yes, platform.map(Platform::from)),
        Commands::Where { id } => match load_view(&repo_root) {
            Ok(view) => cmd_where(&view, &id),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Remove { id, force } => match load_view(&repo_root) {
            Ok(view) => cmd_remove(&view, &id, force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Submit { id, clipboard } => match load_view(&repo_root) {
            Ok(view) => cmd_submit(&view, &id, clipboard),
            Err(()) => ExitCode::FAILURE,
        },
    }
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(r) = std::env::var("RELIQUAINT_REPO_ROOT") {
        return Some(PathBuf::from(r));
    }
    // Dev workflow: walk up from the cwd. Packaged install: fall back to the
    // tap bundled alongside the executable.
    std::env::current_dir()
        .ok()
        .and_then(|cwd| find_repo_root(&cwd))
        .or_else(crate::paths::packaged_repo_root)
}

/// Assemble a `CatalogView` from the bundled tap, the user tap (if
/// present), and install records.
///
/// A missing bundled tap is non-fatal (warn and continue with no
/// bundled entries); a structural bundled-tap error is fatal. The user
/// tap is fully optional — its absence means "no user-created games
/// yet," and a broken user tap is degraded to a warning so it can't
/// take down browsing of bundled content.
fn load_view(repo_root: &Path) -> Result<CatalogView, ()> {
    let mut taps: Vec<crate::tap::LoadedTap> = Vec::new();

    let tap_root = crate::paths::tap_root(repo_root);
    match crate::tap::load_tap(&tap_root) {
        Ok(t) => taps.push(t),
        Err(crate::tap::TapError::MissingRoot { .. }) => {
            tracing::warn!(
                root = %tap_root.display(),
                "bundled tap not found; treating catalog as empty"
            );
        }
        Err(e) => {
            eprintln!("error: {e}");
            return Err(());
        }
    }

    let user_tap_root = crate::paths::user_tap_dir();
    match crate::tap::load_user_tap(&user_tap_root) {
        Ok(t) => taps.push(t),
        Err(crate::tap::TapError::MissingRoot { .. }) => {}
        Err(e) => {
            eprintln!("warning: user tap failed to load: {e}");
        }
    }

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
            eprintln!("hint: run 'reliquaint install {id} <source>' first.");
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

fn cmd_install(
    view: &CatalogView,
    id: &str,
    source: &Path,
    dest: Option<&Path>,
    force: bool,
) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry for '{id}'");
            eprintln!("hint: run 'reliquaint list' to see available ids.");
            return ExitCode::FAILURE;
        }
    };

    let dest_base = dest
        .map(Path::to_path_buf)
        .unwrap_or_else(crate::paths::default_library_dir);
    let spec = game_install::EntrySpec::from_entry(&entry.catalog);

    let plan = match game_install::plan_install(&spec, source, &dest_base) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Stage: copy/extract into the staging dir, streaming output. On any
    // failure, clear staging so the next attempt isn't blocked.
    let run = |cmds: &[Vec<String>]| {
        installer::run_install(cmds.to_vec(), |line, is_err| {
            if is_err {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        })
    };
    if let Err(e) = game_install::stage(&plan, run) {
        let _ = game_install::discard_staging(&plan.staging_dir);
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    // Validate the staged files before committing.
    let missing = install_record::missing_expects_files(
        &plan.staged_install_path,
        &entry.catalog.install.expects_files,
    );
    if !missing.is_empty() && !force {
        eprintln!("warning: expected files not found after install:");
        for f in &missing {
            eprintln!("  - {f}");
        }
        if !prompt_yes_no("Install anyway? [y/N]") {
            // Nothing committed yet — drop the staged copy so a retry works.
            let _ = game_install::discard_staging(&plan.staging_dir);
            eprintln!("aborted");
            return ExitCode::FAILURE;
        }
    }

    // Commit: verify the staged tree contains the install path (e.g. the
    // declared subdir), move staging into place, then register. On any
    // failure, leave nothing behind so a retry isn't blocked.
    if let Err(e) =
        game_install::commit(&plan.staging_dir, &plan.staged_install_path, &plan.dest_dir)
    {
        let _ = game_install::discard_staging(&plan.staging_dir);
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    match install_record::register(
        &entry.catalog.game.id,
        &entry.tap_id,
        &plan.install_path,
        &crate::paths::installs_dir(),
    ) {
        Ok(record_path) => {
            println!(
                "installed {} to {} (record at {})",
                entry.catalog.game.id,
                plan.install_path.display(),
                record_path.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            // Roll back the just-committed dir so it doesn't block retries.
            let _ = game_install::discard_staging(&plan.dest_dir);
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_migrate_installs(view: &CatalogView, base: &str) -> ExitCode {
    let base_path = crate::paths::expand_tilde(base);
    if !base_path.is_dir() {
        eprintln!(
            "error: base directory {} does not exist",
            base_path.display()
        );
        return ExitCode::FAILURE;
    }

    let installs_dir = crate::paths::installs_dir();
    let mut migrated = 0;
    let mut skipped = 0;
    let mut missing = 0;
    let mut errors = 0;

    for entry in view.all() {
        let id = &entry.catalog.game.id;
        if entry.install.is_some() {
            skipped += 1;
            continue;
        }
        let game_dir = base_path.join(id);
        if !game_dir.is_dir() {
            missing += 1;
            continue;
        }
        // Files are already at the canonical `<base>/<id>` location, so we
        // only register a record — no copy. No expects_files prompt; this
        // is a bulk migration.
        match install_record::register(id, &entry.tap_id, &game_dir, &installs_dir) {
            Ok(_) => {
                println!("registered {id} at {}", game_dir.display());
                migrated += 1;
            }
            Err(e) => {
                eprintln!("error migrating {id}: {e}");
                errors += 1;
            }
        }
    }

    println!();
    println!(
        "Done: {migrated} migrated, {skipped} already installed, \
         {missing} not found under {}, {errors} errors.",
        base_path.display()
    );

    if errors > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
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

fn cmd_add(
    repo_root: &Path,
    source: &Path,
    yes: bool,
    platform_override: Option<Platform>,
) -> ExitCode {
    use crate::wizard::{self, AddOptions, WizardError};

    let view = match load_view(repo_root) {
        Ok(v) => v,
        Err(()) => return ExitCode::FAILURE,
    };
    let user_tap_root = crate::paths::user_tap_dir();
    let installs_dir = crate::paths::installs_dir();

    let bundled_ids: Vec<String> = view
        .all()
        .iter()
        .filter(|e| e.tap_id != crate::tap::RESERVED_USER_TAP_ID)
        .map(|e| e.catalog.game.id.clone())
        .collect();
    let user_ids: Vec<String> = view
        .all()
        .iter()
        .filter(|e| e.tap_id == crate::tap::RESERVED_USER_TAP_ID)
        .map(|e| e.catalog.game.id.clone())
        .collect();

    let prepared = wizard::prepare(source, platform_override);
    let (report, draft) = match prepared {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if report.platform.platform.is_none() && platform_override.is_none() {
        eprintln!(
            "error: could not determine platform. evidence:\n{}",
            report
                .platform
                .evidence
                .iter()
                .map(|s| format!("  - {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        eprintln!("hint: re-run with --platform dos|amiga");
        return ExitCode::FAILURE;
    }

    let rendered = match toml::to_string_pretty(&draft) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to render draft: {e}");
            return ExitCode::FAILURE;
        }
    };

    if !yes {
        eprintln!("\n--- draft manifest for {} ---", draft.game.id);
        eprintln!("{rendered}");
        eprintln!("--- end of draft ---");
        if !prompt_yes_no("write this manifest? [y/N]") {
            eprintln!("cancelled.");
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = wizard::validate(&draft) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    let result = wizard::commit(
        &AddOptions {
            source: source.to_path_buf(),
            user_tap_root,
            installs_dir,
            platform_override,
            bundled_ids: &bundled_ids,
            user_ids: &user_ids,
        },
        &draft,
    );
    match result {
        Ok(r) => {
            println!("added {} ({:?}):", r.id, r.platform);
            println!("  manifest:        {}", r.manifest_path.display());
            if let Some(c) = &r.config_path {
                println!("  config:          {}", c.display());
            }
            println!("  install record:  {}", r.install_record_path.display());
            ExitCode::SUCCESS
        }
        Err(WizardError::UserEntryExists(id)) => {
            eprintln!(
                "error: user-tap entry {id:?} already exists. Use `reliquaint remove {id}` first."
            );
            ExitCode::FAILURE
        }
        Err(WizardError::BundledEntryExists(id)) => {
            eprintln!(
                "error: id {id:?} is already used by the bundled catalog; pick a different id."
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_where(view: &CatalogView, id: &str) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry with id {id:?}");
            return ExitCode::FAILURE;
        }
    };
    let installs_dir = crate::paths::installs_dir();

    if entry.tap_id == crate::tap::RESERVED_USER_TAP_ID {
        let user_tap_root = crate::paths::user_tap_dir();
        let locs = crate::wizard::locate(id, &user_tap_root, &installs_dir);
        println!("user-tap entry {id} (tap=local):");
        match &locs.manifest_path {
            Some(p) => println!("  manifest:        {}", p.display()),
            None => println!("  manifest:        (missing)"),
        }
        match &locs.config_path {
            Some(p) => println!("  config:          {}", p.display()),
            None => println!("  config:          (none)"),
        }
        match &locs.install_record_path {
            Some(p) => println!("  install record:  {}", p.display()),
            None => println!("  install record:  (none)"),
        }
        ExitCode::SUCCESS
    } else {
        println!("bundled entry {id} (tap={}):", entry.tap_id);
        println!("  manifest:        {}", entry.source_path.display());
        if let Some(ip) = &entry.install {
            println!(
                "  install record:  {}",
                installs_dir.join(format!("{id}.toml")).display()
            );
            println!("  install path:    {}", ip.install.install_path.display());
        } else {
            println!("  install record:  (not installed)");
        }
        println!(
            "  note: bundled entries are not user-owned. To customize, copy the manifest into the user tap and edit there."
        );
        ExitCode::SUCCESS
    }
}

fn cmd_remove(view: &CatalogView, id: &str, force: bool) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry with id {id:?}");
            return ExitCode::FAILURE;
        }
    };
    if entry.tap_id != crate::tap::RESERVED_USER_TAP_ID {
        eprintln!(
            "error: {id:?} belongs to the bundled tap ({:?}); only user-tap entries can be removed via this command.",
            entry.tap_id
        );
        return ExitCode::FAILURE;
    }

    let user_tap_root = crate::paths::user_tap_dir();
    let installs_dir = crate::paths::installs_dir();
    let locs = crate::wizard::locate(id, &user_tap_root, &installs_dir);

    if !force {
        eprintln!("will remove:");
        if let Some(p) = &locs.manifest_path {
            eprintln!("  - {}", p.display());
        }
        if let Some(p) = &locs.config_path {
            eprintln!("  - {}", p.display());
        }
        if let Some(p) = &locs.install_record_path {
            eprintln!("  - {}", p.display());
        }
        if !prompt_yes_no("proceed? [y/N]") {
            eprintln!("cancelled.");
            return ExitCode::FAILURE;
        }
    }

    match crate::wizard::remove_files(&locs) {
        Ok(()) => {
            println!("removed {id}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_submit(view: &CatalogView, id: &str, clipboard: bool) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry with id {id:?}");
            return ExitCode::FAILURE;
        }
    };
    if entry.tap_id != crate::tap::RESERVED_USER_TAP_ID {
        eprintln!(
            "error: {id:?} belongs to the bundled tap ({:?}); only user-created entries are submission candidates.",
            entry.tap_id
        );
        return ExitCode::FAILURE;
    }
    let exported = match crate::export::export_manifest(&entry.catalog) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let platform = match entry.catalog.game.platform {
        Platform::Dos => "dos",
        Platform::Amiga => "amiga",
    };
    println!("{}", exported.content);
    eprintln!("--- next steps ---");
    eprintln!("target path:    tap/catalog/{platform}/{id}.toml");
    eprintln!(
        "create file:    {}",
        crate::export::github_new_file_url(platform, "develop")
    );
    eprintln!(
        "CONTRIBUTING:   https://github.com/syraenix/reliquaint/blob/develop/CONTRIBUTING.md"
    );
    if !exported.warnings.is_empty() {
        eprintln!();
        eprintln!("--- warnings (non-fatal) ---");
        for w in &exported.warnings {
            eprintln!("  - {w}");
        }
    }
    if clipboard {
        if let Err(e) = copy_to_clipboard(&exported.content) {
            eprintln!("clipboard copy failed: {e}");
        } else {
            eprintln!("(copied to clipboard)");
        }
    }
    ExitCode::SUCCESS
}

/// Pipe `content` to `wl-copy` or `xclip -selection clipboard` —
/// whichever is on PATH. Linux-only per CLAUDE.md.
fn copy_to_clipboard(content: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let (program, args): (&str, &[&str]) = if has_on_path("wl-copy") {
        ("wl-copy", &[])
    } else if has_on_path("xclip") {
        ("xclip", &["-selection", "clipboard"])
    } else {
        return Err(std::io::Error::other(
            "neither wl-copy nor xclip is on PATH",
        ));
    };
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(content.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "{program} exited with status {status}"
        )));
    }
    Ok(())
}

fn has_on_path(name: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|p| p.join(name).is_file())
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
