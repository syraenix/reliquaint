use crate::catalog::Platform;
use crate::catalog_view::{CatalogView, CatalogViewEntry};
use crate::doctor::{check_install, ProbeStatus};
use crate::game_install;
use crate::install_record;
use crate::installer;
use crate::launch;
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
    /// Uninstall a game: remove its install record so it shows as not
    /// installed. With `--delete-files`, also delete the game's directory
    /// on disk (irreversible). Record removal alone is reversible via
    /// `migrate-installs`. Distinct from `remove`, which deletes a
    /// user-created catalog entry.
    Uninstall {
        /// Catalog id (run `reliquaint list --installed` to see options).
        id: String,
        /// Also delete the game's files from disk. Off by default.
        #[arg(long)]
        delete_files: bool,
        /// Skip the confirmation prompt.
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
    /// Manage tap subscriptions.
    Tap {
        #[command(subcommand)]
        subcommand: TapSubcommand,
    },
    /// Detect installed games that reference a tap you are not subscribed to
    /// and suggest the subscribe command. Run after upgrading from v0.2.
    Upgrade,
}

#[derive(Subcommand)]
enum TapSubcommand {
    /// List subscribed taps and their status.
    List {
        #[arg(long, default_value = "tabular", value_enum)]
        format: Format,
        /// Contact each tap's remote to flag whether newer commits exist
        /// upstream ("out of date"). Requires network; failures show as
        /// "unknown".
        #[arg(long)]
        check_remote: bool,
    },
    /// Subscribe to a tap by short name, URL, or local path.
    Add {
        name_or_url: String,
        #[arg(long)]
        priority: Option<u32>,
    },
    /// Unsubscribe from a tap and remove its cache.
    Remove {
        id: String,
        #[arg(long)]
        force: bool,
    },
    /// Pull latest commits for one or all subscribed taps.
    Sync { id: Option<String> },
    /// Change a tap's priority (lower number = higher precedence).
    ///
    /// Either pass `<id> --priority N`, or `--interactive` to edit the full
    /// ordering in `$EDITOR`.
    Reorder {
        id: Option<String>,
        #[arg(long)]
        priority: Option<u32>,
        /// Edit the whole priority ordering in `$EDITOR`.
        #[arg(long, short, conflicts_with_all = ["id", "priority"])]
        interactive: bool,
    },
    /// Validate a tap directory structure (for CI use).
    Validate { path: std::path::PathBuf },
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
    /// After the listing, report game ids provided by more than one tap,
    /// naming each tap and its priority (the lowest-priority tap wins).
    #[arg(long)]
    show_conflicts: bool,
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

    match cli.command {
        Commands::List(opts) => match load_view() {
            Ok(view) => cmd_list(&view, &opts),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Run { id, dry_run } => match load_view() {
            Ok(view) => cmd_run(&view, &id, dry_run),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Install {
            id,
            source,
            dest,
            force,
        } => match load_view() {
            Ok(view) => cmd_install(&view, &id, &source, dest.as_deref(), force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Uninstall {
            id,
            delete_files,
            force,
        } => match load_view() {
            Ok(view) => cmd_uninstall(&view, &id, delete_files, force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::MigrateInstalls { base } => match load_view() {
            Ok(view) => cmd_migrate_installs(&view, &base),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Doctor => match load_view() {
            Ok(view) => cmd_doctor(&view),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Add {
            path,
            yes,
            platform,
        } => cmd_add(&path, yes, platform.map(Platform::from)),
        Commands::Where { id } => match load_view() {
            Ok(view) => cmd_where(&view, &id),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Remove { id, force } => match load_view() {
            Ok(view) => cmd_remove(&view, &id, force),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Submit { id, clipboard } => match load_view() {
            Ok(view) => cmd_submit(&view, &id, clipboard),
            Err(()) => ExitCode::FAILURE,
        },
        Commands::Tap { subcommand } => match subcommand {
            TapSubcommand::List {
                format,
                check_remote,
            } => cmd_tap_list(format, check_remote),
            TapSubcommand::Add {
                name_or_url,
                priority,
            } => cmd_tap_add(&name_or_url, priority),
            TapSubcommand::Remove { id, force } => cmd_tap_remove(&id, force),
            TapSubcommand::Sync { id } => cmd_tap_sync(id.as_deref()),
            TapSubcommand::Reorder {
                id,
                priority,
                interactive,
            } => cmd_tap_reorder(id.as_deref(), priority, interactive),
            TapSubcommand::Validate { path } => cmd_tap_validate(&path),
        },
        Commands::Upgrade => cmd_upgrade(),
    }
}

/// Assemble a `CatalogView` from the subscribed taps and the user tap, plus
/// install records.
///
/// The catalog is sourced entirely from subscriptions — there is no bundled
/// content (the former `reliquaint-core` tap now lives in its own repository;
/// subscribe with `reliquaint tap add reliquaint-core`). The user tap is
/// fully optional — its absence means "no user-created games yet," and a
/// broken tap is degraded to a warning so it can't take down browsing.
fn load_view() -> Result<CatalogView, ()> {
    let mut taps: Vec<crate::tap::LoadedTap> = Vec::new();
    let mut priorities: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    // 1. Subscribed taps — ordered by their stored priority value.
    let subs = crate::subscriptions::SubscriptionManifest::load_or_empty(
        &crate::paths::subscriptions_path(),
    )
    .unwrap_or_else(|e| {
        tracing::warn!("could not load subscriptions.toml: {e}");
        crate::subscriptions::SubscriptionManifest::empty()
    });
    for sub in &subs.taps {
        let cache_dir = crate::paths::tap_cache_dir_for(&sub.id);
        match crate::tap::load_tap(&cache_dir) {
            Ok(t) => {
                priorities.insert(t.metadata.id.clone(), sub.priority as i32);
                taps.push(t);
            }
            Err(crate::tap::TapError::MissingRoot { .. }) => {
                tracing::warn!(
                    tap = %sub.id,
                    "subscribed tap cache missing; run 'reliquaint tap sync' or re-add the tap"
                );
            }
            Err(e) => {
                tracing::warn!(tap = %sub.id, "subscribed tap failed to load: {e}");
            }
        }
    }

    // 2. Local user tap — always wins (priority -1).
    let user_tap_root = crate::paths::user_tap_dir();
    match crate::tap::load_user_tap(&user_tap_root) {
        Ok(t) => {
            priorities.insert(t.metadata.id.clone(), -1);
            taps.push(t);
        }
        Err(crate::tap::TapError::MissingRoot { .. }) => {}
        Err(e) => {
            eprintln!("warning: user tap failed to load: {e}");
        }
    }

    let installs = crate::install_record::load_all(&crate::paths::installs_dir());
    Ok(CatalogView::assemble_with_priorities(
        taps, installs, priorities,
    ))
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

    // Default view shows only the priority winner for each game id. Entries
    // arrive pre-sorted by (priority, tap_id, game_id), so the first time we
    // see an id it is already the winner.
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let winners: Vec<&CatalogViewEntry> = filtered
        .iter()
        .copied()
        .filter(|e| seen.insert(e.catalog.game.id.as_str()))
        .collect();

    match opts.format {
        Format::Tabular => print_tabular(&winners),
        Format::Json => match print_json(&winners) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: failed to serialize JSON: {e}");
                return ExitCode::FAILURE;
            }
        },
    }

    if opts.show_conflicts {
        print_conflicts(&filtered);
    }
    ExitCode::SUCCESS
}

/// Print every game id that more than one tap provides, listing each
/// contributing tap and its priority with the winner marked. Groups are
/// derived from `entries` (already platform/installed-filtered).
fn print_conflicts(entries: &[&CatalogViewEntry]) {
    use std::collections::BTreeMap;
    let mut by_id: BTreeMap<&str, Vec<&CatalogViewEntry>> = BTreeMap::new();
    for e in entries {
        by_id.entry(e.catalog.game.id.as_str()).or_default().push(e);
    }
    let conflicts: Vec<(&str, Vec<&CatalogViewEntry>)> = by_id
        .into_iter()
        .filter(|(_, taps)| taps.len() > 1)
        .collect();

    if conflicts.is_empty() {
        println!("\nNo conflicts: every game id is provided by a single tap.");
        return;
    }

    println!("\nConflicts (game id provided by more than one tap):");
    for (id, mut taps) in conflicts {
        taps.sort_by_key(|e| e.priority);
        println!("  {id}");
        for (rank, e) in taps.iter().enumerate() {
            let marker = if rank == 0 { "  (winner)" } else { "" };
            println!("    {:<20}  priority {}{marker}", e.tap_id, e.priority);
        }
    }
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

fn cmd_uninstall(view: &CatalogView, id: &str, delete_files: bool, force: bool) -> ExitCode {
    let entry = match view.by_id(id) {
        Some(e) => e,
        None => {
            eprintln!("error: no catalog entry for '{id}'");
            eprintln!("hint: run 'reliquaint list --installed' to see installed ids.");
            return ExitCode::FAILURE;
        }
    };

    let install = match entry.install.as_ref() {
        Some(i) => i,
        None => {
            eprintln!("error: '{id}' is not installed");
            return ExitCode::FAILURE;
        }
    };

    // Derive the directory to delete from the recorded install_path so a
    // custom-dest install is removed from wherever it actually lives.
    let dest_dir = game_install::dest_dir_from_install_path(
        &install.install.install_path,
        entry.catalog.install.subdir.as_deref(),
    );

    if !force {
        eprintln!("will remove the install record for {id}");
        if delete_files {
            eprintln!(
                "will DELETE files at {} (irreversible)",
                dest_dir.display()
            );
        } else {
            eprintln!("files at {} will be kept", dest_dir.display());
        }
        if !prompt_yes_no("Proceed? [y/N]") {
            eprintln!("cancelled");
            return ExitCode::FAILURE;
        }
    }

    match game_install::uninstall(
        &entry.catalog.game.id,
        &dest_dir,
        delete_files,
        &crate::paths::installs_dir(),
    ) {
        Ok(outcome) => {
            match outcome.deleted_dir {
                Some(dir) => println!("uninstalled {id} (deleted {})", dir.display()),
                None => println!(
                    "uninstalled {id} (kept files at {})",
                    dest_dir.display()
                ),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
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

fn cmd_add(source: &Path, yes: bool, platform_override: Option<Platform>) -> ExitCode {
    use crate::wizard::{self, AddOptions, WizardError};

    let view = match load_view() {
        Ok(v) => v,
        Err(()) => return ExitCode::FAILURE,
    };
    let user_tap_root = crate::paths::user_tap_dir();
    let installs_dir = crate::paths::installs_dir();

    // Ids already provided by a subscribed tap — a new local entry must not
    // collide with one.
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
            eprintln!("error: id {id:?} is already used by a subscribed tap; pick a different id.");
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
        println!("tap entry {id} (tap={}):", entry.tap_id);
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
            "  note: subscribed-tap entries are not user-owned. To customize, copy the manifest into the user tap and edit there."
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
            "error: {id:?} belongs to a subscribed tap ({:?}); only user-tap entries can be removed via this command.",
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
            "error: {id:?} belongs to a subscribed tap ({:?}); only user-created entries are submission candidates.",
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
    eprintln!("target path:    catalog/{platform}/{id}.toml");
    eprintln!(
        "create file:    {}",
        crate::export::github_new_file_url(platform, "main")
    );
    eprintln!(
        "CONTRIBUTING:   https://github.com/syraenix/reliquaint-core/blob/main/CONTRIBUTING.md"
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
    match crate::tap_fetch::check_git() {
        Ok(version) => println!("git:        ok  ({version})"),
        Err(_) => println!("git:        MISSING  (install with: apt install git)"),
    }
    let user_config = user_config::load_or_default(&crate::paths::user_config_path());
    let results = check_install(view, &user_config);
    let mut any_missing = false;
    for r in &results {
        let label = match r.status {
            ProbeStatus::Ok => "ok     ",
            ProbeStatus::Missing => "missing",
            ProbeStatus::Unknown => "unknown",
            ProbeStatus::Warn => "warn   ",
        };
        let detail = r.detail.as_deref().unwrap_or("");
        println!("[ {label} ] {}  {}", r.name, detail);
        // Only host/install failures affect the exit code; Warn is advisory.
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

/// Computed display state for one subscribed tap row.
struct TapRow {
    id: String,
    priority: Option<u32>,
    entries: usize,
    last_synced: Option<String>,
    status: String,
    source: String,
}

/// Gather a subscribed tap's display state. When `check_remote` is set, the
/// tap's remote is contacted to mark "up to date" / "out of date"; otherwise a
/// healthy cache simply reports "ok".
fn tap_row(sub: &crate::subscriptions::TapSubscription, check_remote: bool) -> TapRow {
    let cache_dir = crate::paths::tap_cache_dir_for(&sub.id);
    let cache_ok = cache_dir.join("tap.toml").exists();
    let entries = if cache_ok {
        crate::tap::load_tap(&cache_dir)
            .map(|t| t.entries.len())
            .unwrap_or(0)
    } else {
        0
    };
    let last_synced = cache_ok
        .then(|| crate::tap_fetch::last_commit_date(&cache_dir))
        .flatten();
    let status = if !cache_ok {
        "missing cache".to_string()
    } else if check_remote {
        match (
            crate::tap_fetch::local_head(&cache_dir),
            crate::tap_fetch::remote_head(&sub.source),
        ) {
            (Ok(local), Ok(remote)) if local == remote => "up to date".to_string(),
            (Ok(_), Ok(_)) => "out of date".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        "ok".to_string()
    };
    TapRow {
        id: sub.id.clone(),
        priority: Some(sub.priority),
        entries,
        last_synced,
        status,
        source: sub.source.clone(),
    }
}

fn cmd_tap_list(format: Format, check_remote: bool) -> ExitCode {
    let subs_path = crate::paths::subscriptions_path();
    let manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(&subs_path)
        .unwrap_or_else(|e| {
            tracing::warn!("could not load subscriptions.toml: {e}");
            crate::subscriptions::SubscriptionManifest::empty()
        });

    match format {
        Format::Tabular => {
            println!(
                "{:<20}  {:<8}  {:<7}  {:<12}  {:<14}  SOURCE",
                "ID", "PRIORITY", "ENTRIES", "LAST SYNCED", "STATUS"
            );
            println!(
                "{:<20}  {:<8}  {:<7}  {:<12}  {:<14}  (your custom games)",
                "local", "-", 0, "-", "local"
            );
            for sub in &manifest.taps {
                let row = tap_row(sub, check_remote);
                println!(
                    "{:<20}  {:<8}  {:<7}  {:<12}  {:<14}  {}",
                    row.id,
                    row.priority.map(|p| p.to_string()).unwrap_or_default(),
                    row.entries,
                    row.last_synced.as_deref().unwrap_or("-"),
                    row.status,
                    row.source
                );
            }
        }
        Format::Json => {
            #[derive(serde::Serialize)]
            struct Row {
                id: String,
                priority: Option<u32>,
                entries: usize,
                last_synced: Option<String>,
                status: String,
                source: String,
            }
            let mut rows: Vec<Row> = vec![Row {
                id: "local".into(),
                priority: None,
                entries: 0,
                last_synced: None,
                status: "local".into(),
                source: "(your custom games)".into(),
            }];
            for sub in &manifest.taps {
                let r = tap_row(sub, check_remote);
                rows.push(Row {
                    id: r.id,
                    priority: r.priority,
                    entries: r.entries,
                    last_synced: r.last_synced,
                    status: r.status,
                    source: r.source,
                });
            }
            match serde_json::to_string_pretty(&rows) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to serialize JSON: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn cmd_tap_add(name_or_url: &str, priority: Option<u32>) -> ExitCode {
    if let Err(e) = crate::tap_fetch::check_git() {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    let source = crate::known_taps::resolve_tap_source(name_or_url);

    let subs_path = crate::paths::subscriptions_path();
    let mut manifest = match crate::subscriptions::SubscriptionManifest::load_or_empty(&subs_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = crate::tap_fetch::clone_tap(source, tmp.path()) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    let meta = match crate::tap::validate_tap_dir(tmp.path(), None) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if manifest.taps.iter().any(|t| t.id == meta.id) {
        eprintln!("error: already subscribed to tap {:?}", meta.id);
        return ExitCode::FAILURE;
    }

    let cache_dest = crate::paths::tap_cache_dir_for(&meta.id);
    if let Some(parent) = cache_dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    }

    let tmp_path = tmp.keep();
    if std::fs::rename(&tmp_path, &cache_dest).is_err() {
        if let Err(e) = copy_dir_recursive(&tmp_path, &cache_dest) {
            eprintln!("error: {e}");
            let _ = std::fs::remove_dir_all(&tmp_path);
            return ExitCode::FAILURE;
        }
        let _ = std::fs::remove_dir_all(&tmp_path);
    }

    let added_at = match std::str::FromStr::from_str(&crate::install_record::now_iso8601()) {
        Ok(dt) => dt,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let tap_priority = priority.unwrap_or_else(|| manifest.next_priority());
    manifest.taps.push(crate::subscriptions::TapSubscription {
        id: meta.id.clone(),
        source: source.to_string(),
        added_at,
        priority: tap_priority,
    });

    if let Err(e) = manifest.write(&subs_path) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    let entries = crate::tap::load_tap(&cache_dest)
        .map(|t| t.entries.len())
        .unwrap_or(0);
    println!("subscribed to {:?}: {entries} catalog entries", meta.id);
    ExitCode::SUCCESS
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn cmd_tap_remove(id: &str, force: bool) -> ExitCode {
    if id == "local" {
        eprintln!("error: the 'local' tap is built-in and cannot be removed");
        return ExitCode::FAILURE;
    }

    let subs_path = crate::paths::subscriptions_path();
    let mut manifest = match crate::subscriptions::SubscriptionManifest::load_or_empty(&subs_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if !manifest.taps.iter().any(|t| t.id == id) {
        eprintln!("error: tap {id:?} is not subscribed");
        return ExitCode::FAILURE;
    }

    if !force {
        let installs = crate::install_record::load_all(&crate::paths::installs_dir());
        let orphaned: Vec<_> = installs
            .iter()
            .filter(|r| r.record.install.tap == id)
            .collect();
        if !orphaned.is_empty() {
            eprintln!("warning: the following install records reference tap {id:?}:");
            for r in &orphaned {
                eprintln!("  - {}", r.record.install.catalog_id);
            }
            if !prompt_yes_no("Remove anyway? [y/N]") {
                eprintln!("cancelled.");
                return ExitCode::SUCCESS;
            }
        }
    }

    manifest.taps.retain(|t| t.id != id);
    if let Err(e) = manifest.write(&subs_path) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    let cache_dir = crate::paths::tap_cache_dir_for(id);
    if cache_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&cache_dir) {
            eprintln!(
                "warning: failed to remove cache at {}: {e}",
                cache_dir.display()
            );
        }
    }

    println!("unsubscribed from {id:?}");
    ExitCode::SUCCESS
}

fn cmd_tap_sync(id: Option<&str>) -> ExitCode {
    let subs_path = crate::paths::subscriptions_path();
    let manifest = match crate::subscriptions::SubscriptionManifest::load_or_empty(&subs_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let taps_to_sync: Vec<_> = match id {
        Some(id) => manifest.taps.iter().filter(|t| t.id == id).collect(),
        None => manifest.taps.iter().collect(),
    };

    if let Some(id) = id {
        if taps_to_sync.is_empty() {
            eprintln!("error: tap {id:?} is not subscribed");
            return ExitCode::FAILURE;
        }
    }

    let mut any_failed = false;
    for sub in taps_to_sync {
        let cache_dir = crate::paths::tap_cache_dir_for(&sub.id);
        if !cache_dir.join("tap.toml").exists() {
            eprintln!(
                "error: cache for tap {:?} is missing — run 'reliquaint tap remove {}' then 'reliquaint tap add' to re-add it",
                sub.id, sub.id
            );
            any_failed = true;
            continue;
        }
        match crate::tap_fetch::pull_tap(&cache_dir) {
            Ok(result) => {
                if result.already_up_to_date {
                    println!("{}: up to date", sub.id);
                } else {
                    let before = &result.before_hash[..result.before_hash.len().min(7)];
                    let after = &result.after_hash[..result.after_hash.len().min(7)];
                    println!("{}: updated {before} -> {after}", sub.id);
                }
            }
            Err(e) => {
                eprintln!("error syncing {:?}: {e}", sub.id);
                any_failed = true;
            }
        }
    }

    if any_failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn cmd_tap_reorder(id: Option<&str>, priority: Option<u32>, interactive: bool) -> ExitCode {
    let subs_path = crate::paths::subscriptions_path();
    let manifest = match crate::subscriptions::SubscriptionManifest::load_or_empty(&subs_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if interactive {
        return cmd_tap_reorder_interactive(manifest, &subs_path);
    }

    match (id, priority) {
        (Some(id), Some(priority)) => cmd_tap_reorder_direct(manifest, &subs_path, id, priority),
        _ => {
            eprintln!(
                "error: provide a tap id and --priority, or use --interactive to edit the ordering"
            );
            ExitCode::FAILURE
        }
    }
}

fn cmd_tap_reorder_direct(
    mut manifest: crate::subscriptions::SubscriptionManifest,
    subs_path: &Path,
    id: &str,
    priority: u32,
) -> ExitCode {
    if !manifest.taps.iter().any(|t| t.id == id) {
        eprintln!("error: tap {id:?} is not subscribed");
        return ExitCode::FAILURE;
    }

    if let Some(conflict) = manifest
        .taps
        .iter()
        .find(|t| t.id != id && t.priority == priority)
    {
        eprintln!(
            "error: priority {priority} is already used by tap {:?}",
            conflict.id
        );
        return ExitCode::FAILURE;
    }

    for tap in &mut manifest.taps {
        if tap.id == id {
            tap.priority = priority;
            break;
        }
    }

    if let Err(e) = manifest.write(subs_path) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    println!("{id}: priority set to {priority}");
    ExitCode::SUCCESS
}

/// Open the current tap ordering in `$EDITOR`, then apply the edited
/// priorities. The editable text is `<priority> <tap-id>` lines; comments and
/// blank lines are ignored. Priorities must remain unique.
fn cmd_tap_reorder_interactive(
    mut manifest: crate::subscriptions::SubscriptionManifest,
    subs_path: &Path,
) -> ExitCode {
    if manifest.taps.is_empty() {
        println!("No subscribed taps to reorder.");
        return ExitCode::SUCCESS;
    }

    // Present taps sorted by current priority for easy reordering.
    let mut ordered = manifest.taps.clone();
    ordered.sort_by_key(|t| t.priority);
    let mut text = String::from(
        "# Reliquaint tap priorities — lower number wins in conflicts.\n\
         # Edit the priority (first column). One tap per line: <priority> <tap-id>\n\
         # Lines starting with '#' and blank lines are ignored.\n",
    );
    for t in &ordered {
        text.push_str(&format!("{} {}\n", t.priority, t.id));
    }

    let edited = match edit_in_editor(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Parse "<priority> <id>" lines into id -> priority.
    let mut new_priorities: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for (lineno, line) in edited.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(prio_s), Some(id)) = (parts.next(), parts.next()) else {
            eprintln!(
                "error: line {}: expected '<priority> <tap-id>', got {line:?}",
                lineno + 1
            );
            return ExitCode::FAILURE;
        };
        let prio: u32 = match prio_s.parse() {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "error: line {}: {prio_s:?} is not a valid priority",
                    lineno + 1
                );
                return ExitCode::FAILURE;
            }
        };
        if !manifest.taps.iter().any(|t| t.id == id) {
            eprintln!("error: line {}: unknown tap id {id:?}", lineno + 1);
            return ExitCode::FAILURE;
        }
        if new_priorities.insert(id.to_string(), prio).is_some() {
            eprintln!("error: tap {id:?} listed more than once",);
            return ExitCode::FAILURE;
        }
    }

    // Apply edits over the current priorities (unedited taps keep theirs).
    for tap in &mut manifest.taps {
        if let Some(p) = new_priorities.get(&tap.id) {
            tap.priority = *p;
        }
    }

    // Priorities must be unique across all subscribed taps.
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for tap in &manifest.taps {
        if !seen.insert(tap.priority) {
            eprintln!(
                "error: priority {} is shared by more than one tap; no two taps may share a priority",
                tap.priority
            );
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = manifest.write(subs_path) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    println!("updated tap priorities");
    ExitCode::SUCCESS
}

/// Write `contents` to a temp file, open it in `$VISUAL`/`$EDITOR` (falling
/// back to `vi`), and return the edited contents.
fn edit_in_editor(contents: &str) -> std::io::Result<String> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let mut tmp = std::env::temp_dir();
    tmp.push(format!("reliquaint-reorder-{}.txt", std::process::id()));
    std::fs::write(&tmp, contents)?;

    // Run through a shell so `$EDITOR` may carry arguments; the temp path is
    // passed positionally as "$1".
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(&tmp)
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(std::io::Error::other(
            "editor exited with a non-zero status",
        ));
    }

    let edited = std::fs::read_to_string(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(edited)
}

fn cmd_tap_validate(path: &std::path::Path) -> ExitCode {
    match crate::tap::validate_tap_dir(path, None) {
        Ok(meta) => {
            let entries = crate::tap::load_tap(path)
                .map(|t| t.entries.len())
                .unwrap_or(0);
            println!(
                "ok: tap {:?} v{} ({entries} catalog entries)",
                meta.id, meta.version
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_upgrade() -> ExitCode {
    let installs = crate::install_record::load_all(&crate::paths::installs_dir());
    let manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(
        &crate::paths::subscriptions_path(),
    )
    .unwrap_or_else(|_| crate::subscriptions::SubscriptionManifest::empty());

    // Collect tap ids referenced by install records that are known taps but
    // not currently subscribed.
    let missing: std::collections::BTreeSet<String> = installs
        .iter()
        .map(|r| r.record.install.tap.as_str())
        .filter(|tap_id| {
            crate::known_taps::KNOWN_TAPS
                .iter()
                .any(|(name, _)| name == tap_id)
                && !manifest.taps.iter().any(|t| t.id.as_str() == *tap_id)
        })
        .map(|s| s.to_string())
        .collect();

    if missing.is_empty() {
        println!("Nothing to upgrade — all install records reference subscribed taps.");
        return ExitCode::SUCCESS;
    }

    println!(
        "Found {} installed game(s) referencing taps you are not subscribed to.",
        installs
            .iter()
            .filter(|r| missing.contains(&r.record.install.tap))
            .count()
    );
    println!();
    for tap_id in &missing {
        let count = installs
            .iter()
            .filter(|r| &r.record.install.tap == tap_id)
            .count();
        println!("  {tap_id}: {count} game(s)");
        println!("  → run: reliquaint tap add {tap_id}");
    }
    ExitCode::SUCCESS
}
