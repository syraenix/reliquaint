//! Game installation: copy/extract a user-supplied source into a managed
//! library directory (default `~/games/<id>`), so the resulting directory
//! can be recorded as an install record's `install_path`.
//!
//! This module is the generic, catalog-driven replacement for the
//! pre-redesign per-collection installer. Installs follow a
//! **stage-then-commit** model so a declined or failed install never strands
//! a half-populated `<library>/<id>` (which would dead-end the next attempt
//! against the occupancy guard):
//!
//! 1. [`plan_install`] (pure) classifies the source and computes the
//!    destination, a sibling **staging** dir (`<library>/.<id>.staging`), and
//!    the copy/extract commands — without touching the filesystem beyond
//!    classification + an occupancy check.
//! 2. [`stage`] clears any stale staging dir and copies/extracts the source
//!    into it (running shell commands via an injected runner, or unzipping an
//!    `.rp9` in process). The managed `<library>/<id>` is *not* created yet.
//! 3. The caller validates `staged_install_path` (expects_files) and then
//!    either [`commit_dirs`] (atomic rename staging → `<library>/<id>`, same
//!    filesystem) or [`discard_staging`] (remove the staging dir).
//!
//! Supported sources:
//! - **directory** (any platform) → recursive copy
//! - **`.exe`** (DOS) → `innoextract`
//! - **`.adf` / `.hdf`** (Amiga) → copy the disk image in
//! - **`.rp9`** (Amiga) → unzip the RetroPlatform bundle

use crate::catalog::{CatalogEntry, Platform};
use std::path::{Path, PathBuf};

/// The subset of a [`CatalogEntry`] the install planner needs. Keeping it
/// explicit (rather than passing the whole entry) makes the planner's
/// inputs obvious and the unit tests light.
pub struct EntrySpec<'a> {
    pub id: &'a str,
    pub platform: Platform,
    pub subdir: Option<&'a str>,
    pub floppies: &'a [String],
    pub hard_drives: &'a [String],
}

impl<'a> EntrySpec<'a> {
    pub fn from_entry(entry: &'a CatalogEntry) -> Self {
        let (floppies, hard_drives): (&[String], &[String]) = match entry.runtime.fs_uae.as_ref() {
            Some(fs) => (&fs.floppies, &fs.hard_drives),
            None => (&[], &[]),
        };
        EntrySpec {
            id: &entry.game.id,
            platform: entry.game.platform,
            subdir: entry.install.subdir.as_deref(),
            floppies,
            hard_drives,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A directory whose contents are copied in.
    Directory,
    /// A DOS GOG-style `.exe` installer, extracted with `innoextract`.
    DosInstaller,
    /// An Amiga `.adf` / `.hdf` / `.rp9` disk image or bundle.
    AmigaImage,
}

/// How the planned install populates the staging dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyAction {
    /// Shell commands to run in order via the injected runner.
    Commands(Vec<Vec<String>>),
    /// Unzip this `.rp9` archive into the staging dir in process.
    UnzipRp9 { archive: PathBuf },
}

/// The canonical filesystem locations for an install, derived purely from
/// the entry id, its optional `subdir`, and the chosen library base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallLocations {
    /// Final `<library>/<id>` — created only on commit.
    pub dest_dir: PathBuf,
    /// Scratch `<library>/.<id>.staging` — populated by [`stage`], then
    /// renamed to `dest_dir` on commit (same filesystem → atomic).
    pub staging_dir: PathBuf,
    /// Directory recorded as `install_path` after commit: `dest_dir[/subdir]`.
    pub install_path: PathBuf,
    /// Where to validate `expects_files` before commit: `staging_dir[/subdir]`.
    pub staged_install_path: PathBuf,
}

/// A fully-resolved install, ready to [`stage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub kind: SourceKind,
    pub dest_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub install_path: PathBuf,
    pub staged_install_path: PathBuf,
    pub action: CopyAction,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("source path {path} does not exist")]
    SourceMissing { path: PathBuf },

    #[error("unsupported source {path} for {platform:?}: expected a directory{hint}")]
    UnsupportedSource {
        path: PathBuf,
        platform: Platform,
        hint: &'static str,
    },

    #[error("destination {path} already exists and is not empty")]
    DestinationOccupied { path: PathBuf },

    #[error(
        "staged install is incomplete: {path} does not exist (the source is \
         missing the entry's required subdirectory)"
    )]
    IncompleteStaging { path: PathBuf },

    #[error("install command failed with exit code {code}")]
    CommandFailed { code: i32 },

    #[error("install command could not run: {message}")]
    CommandSpawn { message: String },

    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to extract rp9 {path}: {message}")]
    Rp9 { path: PathBuf, message: String },
}

/// Compute the canonical install locations for `id` (with optional `subdir`)
/// under `dest_base`. Pure — no filesystem access. Shared by [`plan_install`]
/// and the GUI commit/discard commands so they agree on paths.
pub fn locations(id: &str, subdir: Option<&str>, dest_base: &Path) -> InstallLocations {
    let dest_dir = dest_base.join(id);
    let staging_dir = dest_base.join(format!(".{id}.staging"));
    let install_path = join_subdir(&dest_dir, subdir);
    let staged_install_path = join_subdir(&staging_dir, subdir);
    InstallLocations {
        dest_dir,
        staging_dir,
        install_path,
        staged_install_path,
    }
}

fn join_subdir(dir: &Path, subdir: Option<&str>) -> PathBuf {
    match subdir {
        Some(s) => dir.join(s),
        None => dir.to_path_buf(),
    }
}

/// Classify `source` for the entry's platform, compute the destination +
/// staging locations under `dest_base`, and build the copy/extract action
/// (which targets the staging dir). Reads the filesystem only to classify the
/// source and to reject an already-occupied final destination.
pub fn plan_install(
    spec: &EntrySpec,
    source: &Path,
    dest_base: &Path,
) -> Result<InstallPlan, InstallError> {
    if !source.exists() {
        return Err(InstallError::SourceMissing {
            path: source.to_path_buf(),
        });
    }

    let kind = classify(source, spec.platform)?;

    let loc = locations(spec.id, spec.subdir, dest_base);
    // Reject only if a real install already occupies the final dir; staging
    // is our own scratch space and is cleared by `stage`.
    if dir_is_nonempty(&loc.dest_dir) {
        return Err(InstallError::DestinationOccupied { path: loc.dest_dir });
    }

    let action = match kind {
        SourceKind::Directory => CopyAction::Commands(vec![
            mkdir_p(&loc.staging_dir),
            vec![
                "cp".into(),
                "-r".into(),
                "--".into(),
                format!("{}/.", source.to_string_lossy()),
                loc.staging_dir.to_string_lossy().into_owned(),
            ],
        ]),
        SourceKind::DosInstaller => CopyAction::Commands(vec![
            mkdir_p(&loc.staging_dir),
            vec![
                "innoextract".into(),
                "--exclude-temp".into(),
                "--silent".into(),
                "--output-dir".into(),
                loc.staging_dir.to_string_lossy().into_owned(),
                source.to_string_lossy().into_owned(),
            ],
        ]),
        SourceKind::AmigaImage => amiga_action(spec, source, &loc.staging_dir),
    };

    Ok(InstallPlan {
        kind,
        dest_dir: loc.dest_dir,
        staging_dir: loc.staging_dir,
        install_path: loc.install_path,
        staged_install_path: loc.staged_install_path,
        action,
    })
}

/// Clear any stale staging dir (from an abandoned prior attempt), then
/// copy/extract the source into it. `run_commands` runs a batch of argv
/// vectors in order and returns the final exit code (the CLI prints to
/// stderr; the GUI streams Tauri events). `.rp9` bundles are unzipped in
/// process and bypass the runner.
pub fn stage<R>(plan: &InstallPlan, run_commands: R) -> Result<(), InstallError>
where
    R: FnOnce(&[Vec<String>]) -> Result<i32, String>,
{
    discard_staging(&plan.staging_dir)?;
    match &plan.action {
        CopyAction::Commands(cmds) => match run_commands(cmds) {
            Ok(0) => Ok(()),
            Ok(code) => Err(InstallError::CommandFailed { code }),
            Err(message) => Err(InstallError::CommandSpawn { message }),
        },
        CopyAction::UnzipRp9 { archive } => unzip_rp9(archive, &plan.staging_dir),
    }
}

/// Commit a staged install. First verifies the staged tree actually contains
/// `staged_install_path` (the entry's declared `subdir`, if any) — otherwise
/// the post-commit `install_path` would not exist and registration would
/// fail, leaving an orphan `dest_dir` that blocks retries. Then renames
/// `staging_dir` → `dest_dir`. Nothing is moved when the precondition fails.
pub fn commit(
    staging_dir: &Path,
    staged_install_path: &Path,
    dest_dir: &Path,
) -> Result<(), InstallError> {
    if !staged_install_path.is_dir() {
        return Err(InstallError::IncompleteStaging {
            path: staged_install_path.to_path_buf(),
        });
    }
    commit_dirs(staging_dir, dest_dir)
}

/// Atomically rename `staging_dir` → `dest_dir` (same filesystem, no copy).
/// Errors if `dest_dir` is already a non-empty install. Prefer [`commit`],
/// which adds the staged-path precondition.
pub fn commit_dirs(staging_dir: &Path, dest_dir: &Path) -> Result<(), InstallError> {
    if dir_is_nonempty(dest_dir) {
        return Err(InstallError::DestinationOccupied {
            path: dest_dir.to_path_buf(),
        });
    }
    // `rename` replaces an empty `dest_dir` and creates an absent one; a
    // non-empty one was rejected above.
    std::fs::rename(staging_dir, dest_dir).map_err(|source| InstallError::Io {
        path: dest_dir.to_path_buf(),
        source,
    })
}

/// Remove a directory tree if it exists. Used to drop a staging dir on
/// cancel/failure, and to roll back a just-committed `dest_dir` when the
/// record write fails (so an orphan dir never blocks retries). No-op if the
/// path is absent, so calling it twice is safe.
pub fn discard_staging(dir: &Path) -> Result<(), InstallError> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|source| InstallError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

/// Outcome of an [`uninstall`], for caller messaging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallOutcome {
    /// The record path that was removed (or that no longer existed).
    pub record_path: PathBuf,
    /// `Some(dest_dir)` iff files were deleted from disk.
    pub deleted_dir: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum UninstallError {
    #[error(transparent)]
    Record(#[from] crate::install_record::InstallError),
    #[error(transparent)]
    Files(#[from] InstallError),
}

/// Recover the `dest_dir` (`<library>/<id>`) from a recorded `install_path`.
///
/// An install record stores only the full `install_path`, which is
/// `dest_dir[/subdir]` (see [`locations`]). When the entry declares a `subdir`
/// and `install_path` ends with exactly that component, strip it to get
/// `dest_dir`; otherwise the recorded path *is* the `dest_dir`. Catalog parsing
/// rejects path separators in `subdir`, so it is always a single component and
/// [`Path::parent`] strips it exactly. This is the inverse of [`join_subdir`].
pub fn dest_dir_from_install_path(install_path: &Path, subdir: Option<&str>) -> PathBuf {
    match subdir {
        Some(s) if install_path.ends_with(s) => install_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| install_path.to_path_buf()),
        _ => install_path.to_path_buf(),
    }
}

/// Uninstall `catalog_id`: remove its install record (so it shows as not
/// installed), and — when `delete_files` is true — delete `dest_dir` from disk.
///
/// The record is removed **first**, then the files: if file deletion fails
/// partway, the game is already marked not-installed and the user can clean up
/// the directory manually, rather than being left with a record pointing at a
/// half-deleted dir. `dest_dir` must come from [`dest_dir_from_install_path`],
/// so nothing outside the game's own directory is ever touched. Idempotent on a
/// missing record or a missing `dest_dir`.
pub fn uninstall(
    catalog_id: &str,
    dest_dir: &Path,
    delete_files: bool,
    installs_dir: &Path,
) -> Result<UninstallOutcome, UninstallError> {
    let record_path = crate::install_record::deregister(catalog_id, installs_dir)?;
    let deleted_dir = if delete_files {
        discard_staging(dest_dir)?;
        Some(dest_dir.to_path_buf())
    } else {
        None
    };
    Ok(UninstallOutcome {
        record_path,
        deleted_dir,
    })
}

fn classify(source: &Path, platform: Platform) -> Result<SourceKind, InstallError> {
    if source.is_dir() {
        return Ok(SourceKind::Directory);
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match platform {
        Platform::Dos => {
            if ext == "exe" {
                Ok(SourceKind::DosInstaller)
            } else {
                Err(InstallError::UnsupportedSource {
                    path: source.to_path_buf(),
                    platform,
                    hint: " or a .exe installer",
                })
            }
        }
        Platform::Amiga => {
            if matches!(ext.as_str(), "adf" | "hdf" | "rp9") {
                Ok(SourceKind::AmigaImage)
            } else {
                Err(InstallError::UnsupportedSource {
                    path: source.to_path_buf(),
                    platform,
                    hint: " or a .adf/.hdf/.rp9 disk image",
                })
            }
        }
    }
}

/// Build the action for an Amiga disk-image source, copying/extracting into
/// `target_dir` (the staging dir). `.rp9` is unzipped; `.adf`/`.hdf` are
/// copied in. When the entry declares exactly one disk of the matching kind,
/// the copied file is renamed to that declared name so a differently-named
/// source still launches.
fn amiga_action(spec: &EntrySpec, source: &Path, target_dir: &Path) -> CopyAction {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "rp9" {
        return CopyAction::UnzipRp9 {
            archive: source.to_path_buf(),
        };
    }

    let source_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let target_name = match ext.as_str() {
        "adf" if spec.floppies.len() == 1 => spec.floppies[0].clone(),
        "hdf" if spec.hard_drives.len() == 1 => spec.hard_drives[0].clone(),
        _ => source_name,
    };

    CopyAction::Commands(vec![
        mkdir_p(target_dir),
        vec![
            "cp".into(),
            "--".into(),
            source.to_string_lossy().into_owned(),
            target_dir.join(target_name).to_string_lossy().into_owned(),
        ],
    ])
}

fn mkdir_p(dir: &Path) -> Vec<String> {
    vec![
        "mkdir".into(),
        "-p".into(),
        dir.to_string_lossy().into_owned(),
    ]
}

fn dir_is_nonempty(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Extract every file in `archive` into `dest_dir`, recreating inner
/// directory structure. Mirrors the pre-redesign `.rp9` handling.
fn unzip_rp9(archive: &Path, dest_dir: &Path) -> Result<(), InstallError> {
    std::fs::create_dir_all(dest_dir).map_err(|source| InstallError::Io {
        path: dest_dir.to_path_buf(),
        source,
    })?;

    let file = std::fs::File::open(archive).map_err(|source| InstallError::Io {
        path: archive.to_path_buf(),
        source,
    })?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| InstallError::Rp9 {
        path: archive.to_path_buf(),
        message: e.to_string(),
    })?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| InstallError::Rp9 {
            path: archive.to_path_buf(),
            message: e.to_string(),
        })?;
        if entry.is_dir() {
            continue;
        }
        // Normalize Windows separators and strip any path traversal so a
        // crafted archive can't escape dest_dir.
        let rel = entry.name().replace('\\', "/");
        let safe: PathBuf = Path::new(&rel)
            .components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .collect();
        if safe.as_os_str().is_empty() {
            continue;
        }
        let out_path = dest_dir.join(safe);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| InstallError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut out = std::fs::File::create(&out_path).map_err(|source| InstallError::Io {
            path: out_path.clone(),
            source,
        })?;
        std::io::copy(&mut entry, &mut out).map_err(|source| InstallError::Io {
            path: out_path.clone(),
            source,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Platform;
    use std::path::{Path, PathBuf};

    fn spec<'a>(id: &'a str, platform: Platform) -> EntrySpec<'a> {
        EntrySpec {
            id,
            platform,
            subdir: None,
            floppies: &[],
            hard_drives: &[],
        }
    }

    // --- locations ---------------------------------------------------------

    #[test]
    fn locations_computes_staging_and_subdir() {
        let base = Path::new("/lib");
        let loc = locations("kq5", Some("EGA"), base);
        assert_eq!(loc.dest_dir, PathBuf::from("/lib/kq5"));
        assert_eq!(loc.staging_dir, PathBuf::from("/lib/.kq5.staging"));
        assert_eq!(loc.install_path, PathBuf::from("/lib/kq5/EGA"));
        assert_eq!(
            loc.staged_install_path,
            PathBuf::from("/lib/.kq5.staging/EGA")
        );
    }

    #[test]
    fn locations_without_subdir_uses_dirs_directly() {
        let loc = locations("fatman", None, Path::new("/lib"));
        assert_eq!(loc.install_path, loc.dest_dir);
        assert_eq!(loc.staged_install_path, loc.staging_dir);
    }

    // --- plan_install ------------------------------------------------------

    #[test]
    fn directory_source_stages_recursive_copy() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("GAME.EXE"), b"x").unwrap();
        let base = tempfile::tempdir().unwrap();

        let plan = plan_install(&spec("kq5", Platform::Dos), src.path(), base.path()).unwrap();

        assert_eq!(plan.kind, SourceKind::Directory);
        assert_eq!(plan.dest_dir, base.path().join("kq5"));
        assert_eq!(plan.staging_dir, base.path().join(".kq5.staging"));
        assert_eq!(plan.install_path, base.path().join("kq5"));
        assert_eq!(plan.staged_install_path, base.path().join(".kq5.staging"));
        match &plan.action {
            CopyAction::Commands(cmds) => {
                assert_eq!(cmds[0][0], "mkdir");
                let cp = cmds.last().unwrap();
                assert_eq!(cp[0], "cp");
                // Copy targets the staging dir, not the final dest.
                assert!(cp.last().unwrap().ends_with(".kq5.staging"), "got {cp:?}");
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn dos_exe_stages_innoextract_with_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("qfg1.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let base = tempfile::tempdir().unwrap();
        let s = EntrySpec {
            id: "qfg1-ega",
            platform: Platform::Dos,
            subdir: Some("EGA"),
            floppies: &[],
            hard_drives: &[],
        };

        let plan = plan_install(&s, &exe, base.path()).unwrap();

        assert_eq!(plan.kind, SourceKind::DosInstaller);
        assert_eq!(plan.install_path, base.path().join("qfg1-ega").join("EGA"));
        assert_eq!(
            plan.staged_install_path,
            base.path().join(".qfg1-ega.staging").join("EGA")
        );
        match &plan.action {
            CopyAction::Commands(cmds) => {
                let inno = cmds
                    .iter()
                    .find(|c| c[0] == "innoextract")
                    .expect("innoextract cmd");
                // Extract into staging.
                let out_idx = inno.iter().position(|a| a == "--output-dir").unwrap();
                assert!(
                    inno[out_idx + 1].ends_with(".qfg1-ega.staging"),
                    "got {inno:?}"
                );
                assert!(inno.iter().any(|a| a.ends_with("qfg1.exe")));
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn amiga_adf_renames_to_declared_floppy_in_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let adf = tmp.path().join("MyDisk.adf");
        std::fs::write(&adf, b"DOS").unwrap();
        let base = tempfile::tempdir().unwrap();
        let floppies = vec!["fatman.adf".to_string()];
        let s = EntrySpec {
            id: "fatman",
            platform: Platform::Amiga,
            subdir: None,
            floppies: &floppies,
            hard_drives: &[],
        };

        let plan = plan_install(&s, &adf, base.path()).unwrap();

        match &plan.action {
            CopyAction::Commands(cmds) => {
                let cp = cmds.iter().find(|c| c[0] == "cp").unwrap();
                assert!(
                    cp.last().unwrap().ends_with(".fatman.staging/fatman.adf"),
                    "got {cp:?}"
                );
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn amiga_adf_without_declared_floppy_keeps_source_name() {
        let tmp = tempfile::tempdir().unwrap();
        let adf = tmp.path().join("lemmings.adf");
        std::fs::write(&adf, b"DOS").unwrap();
        let base = tempfile::tempdir().unwrap();

        let plan = plan_install(&spec("lemmings", Platform::Amiga), &adf, base.path()).unwrap();

        match &plan.action {
            CopyAction::Commands(cmds) => {
                let cp = cmds.iter().find(|c| c[0] == "cp").unwrap();
                assert!(
                    cp.last()
                        .unwrap()
                        .ends_with(".lemmings.staging/lemmings.adf"),
                    "got {cp:?}"
                );
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn amiga_hdf_renames_to_declared_hard_drive() {
        let tmp = tempfile::tempdir().unwrap();
        let hdf = tmp.path().join("whatever.hdf");
        std::fs::write(&hdf, b"RDSK").unwrap();
        let base = tempfile::tempdir().unwrap();
        let hard_drives = vec!["system.hdf".to_string()];
        let s = EntrySpec {
            id: "wb",
            platform: Platform::Amiga,
            subdir: None,
            floppies: &[],
            hard_drives: &hard_drives,
        };

        let plan = plan_install(&s, &hdf, base.path()).unwrap();

        match &plan.action {
            CopyAction::Commands(cmds) => {
                let cp = cmds.iter().find(|c| c[0] == "cp").unwrap();
                assert!(
                    cp.last().unwrap().ends_with(".wb.staging/system.hdf"),
                    "got {cp:?}"
                );
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn rp9_plans_inprocess_unzip() {
        let tmp = tempfile::tempdir().unwrap();
        let rp9 = tmp.path().join("game.rp9");
        std::fs::write(&rp9, b"PK").unwrap();
        let base = tempfile::tempdir().unwrap();

        let plan = plan_install(&spec("game", Platform::Amiga), &rp9, base.path()).unwrap();

        assert_eq!(plan.kind, SourceKind::AmigaImage);
        match &plan.action {
            CopyAction::UnzipRp9 { archive } => assert_eq!(archive, &rp9),
            other => panic!("expected UnzipRp9, got {other:?}"),
        }
    }

    #[test]
    fn exe_on_amiga_is_unsupported() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("game.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let base = tempfile::tempdir().unwrap();

        let err = plan_install(&spec("game", Platform::Amiga), &exe, base.path()).unwrap_err();
        assert!(
            matches!(err, InstallError::UnsupportedSource { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn adf_on_dos_is_unsupported() {
        let tmp = tempfile::tempdir().unwrap();
        let adf = tmp.path().join("game.adf");
        std::fs::write(&adf, b"x").unwrap();
        let base = tempfile::tempdir().unwrap();

        let err = plan_install(&spec("game", Platform::Dos), &adf, base.path()).unwrap_err();
        assert!(
            matches!(err, InstallError::UnsupportedSource { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_source_errors() {
        let base = tempfile::tempdir().unwrap();
        let err = plan_install(
            &spec("game", Platform::Dos),
            Path::new("/no/such/path"),
            base.path(),
        )
        .unwrap_err();
        assert!(
            matches!(err, InstallError::SourceMissing { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn occupied_final_destination_errors() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a"), b"x").unwrap();
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(base.path().join("kq5")).unwrap();
        std::fs::write(base.path().join("kq5/existing"), b"x").unwrap();

        let err = plan_install(&spec("kq5", Platform::Dos), src.path(), base.path()).unwrap_err();
        assert!(
            matches!(err, InstallError::DestinationOccupied { .. }),
            "got {err:?}"
        );
    }

    // --- stage -------------------------------------------------------------

    fn plan_with(staging_dir: PathBuf, action: CopyAction) -> InstallPlan {
        InstallPlan {
            kind: SourceKind::Directory,
            dest_dir: staging_dir.with_extension("dest"),
            staging_dir: staging_dir.clone(),
            install_path: staging_dir.with_extension("dest"),
            staged_install_path: staging_dir,
            action,
        }
    }

    #[test]
    fn stage_runs_commands_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = plan_with(
            tmp.path().join(".x.staging"),
            CopyAction::Commands(vec![vec!["mkdir".into(), "-p".into(), "x".into()]]),
        );
        let captured = std::cell::RefCell::new(Vec::new());
        stage(&plan, |cmds| {
            captured.borrow_mut().extend_from_slice(cmds);
            Ok(0)
        })
        .unwrap();
        assert_eq!(captured.borrow().len(), 1);
    }

    #[test]
    fn stage_propagates_nonzero_exit() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = plan_with(
            tmp.path().join(".x.staging"),
            CopyAction::Commands(vec![vec!["false".into()]]),
        );
        let err = stage(&plan, |_| Ok(7)).unwrap_err();
        assert!(
            matches!(err, InstallError::CommandFailed { code: 7 }),
            "got {err:?}"
        );
    }

    #[test]
    fn stage_clears_stale_staging_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".x.staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("junk"), b"old").unwrap();
        // Fake runner is a no-op, so after the clear step staging is gone.
        let plan = plan_with(
            staging.clone(),
            CopyAction::Commands(vec![vec!["true".into()]]),
        );

        stage(&plan, |_| Ok(0)).unwrap();

        assert!(!staging.exists(), "stale staging should have been cleared");
    }

    #[test]
    fn stage_unzips_rp9_into_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let rp9 = tmp.path().join("game.rp9");
        {
            use std::io::Write;
            let f = std::fs::File::create(&rp9).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            zw.start_file("inner.adf", zip::write::FileOptions::default())
                .unwrap();
            zw.write_all(b"ADFDATA").unwrap();
            zw.finish().unwrap();
        }
        let staging = tmp.path().join(".game.staging");
        let plan = plan_with(staging.clone(), CopyAction::UnzipRp9 { archive: rp9 });

        stage(&plan, |_| Ok(0)).unwrap();

        let inner = std::fs::read(staging.join("inner.adf")).unwrap();
        assert_eq!(inner, b"ADFDATA");
    }

    // --- commit / discard --------------------------------------------------

    #[test]
    fn commit_renames_staging_to_dest() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".kq5.staging");
        let dest = tmp.path().join("kq5");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("GAME"), b"x").unwrap();

        commit_dirs(&staging, &dest).unwrap();

        assert!(dest.join("GAME").is_file(), "files should be at dest");
        assert!(!staging.exists(), "staging should be gone after rename");
    }

    #[test]
    fn commit_succeeds_when_staged_install_path_present() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".g.staging");
        let staged_install = staging.join("EGA");
        std::fs::create_dir_all(&staged_install).unwrap();
        std::fs::write(staged_install.join("GAME"), b"x").unwrap();
        let dest = tmp.path().join("g");

        commit(&staging, &staged_install, &dest).unwrap();

        assert!(dest.join("EGA/GAME").is_file());
        assert!(!staging.exists());
    }

    #[test]
    fn commit_errors_when_staged_subdir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".g.staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("GAME"), b"x").unwrap();
        // The entry declares subdir EGA, but the source didn't include it.
        let staged_install = staging.join("EGA");
        let dest = tmp.path().join("g");

        let err = commit(&staging, &staged_install, &dest).unwrap_err();

        assert!(
            matches!(err, InstallError::IncompleteStaging { .. }),
            "got {err:?}"
        );
        assert!(
            !dest.exists(),
            "must not commit when the staged subdir is missing"
        );
        assert!(
            staging.exists(),
            "staging is left for the caller to discard"
        );
    }

    #[test]
    fn commit_errors_when_dest_occupied() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".kq5.staging");
        let dest = tmp.path().join("kq5");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("existing"), b"x").unwrap();

        let err = commit_dirs(&staging, &dest).unwrap_err();
        assert!(
            matches!(err, InstallError::DestinationOccupied { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn discard_removes_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join(".kq5.staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("GAME"), b"x").unwrap();

        discard_staging(&staging).unwrap();

        assert!(!staging.exists());
    }

    #[test]
    fn discard_absent_staging_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        discard_staging(&tmp.path().join(".nope.staging")).unwrap();
    }

    #[test]
    fn dest_dir_from_install_path_strips_matching_subdir() {
        let got = dest_dir_from_install_path(Path::new("/lib/kq5/EGA"), Some("EGA"));
        assert_eq!(got, PathBuf::from("/lib/kq5"));
    }

    #[test]
    fn dest_dir_from_install_path_no_subdir_is_identity() {
        let got = dest_dir_from_install_path(Path::new("/lib/fatman"), None);
        assert_eq!(got, PathBuf::from("/lib/fatman"));
    }

    #[test]
    fn dest_dir_from_install_path_mismatched_subdir_is_identity() {
        // Defensive: a recorded path that doesn't end with the declared subdir
        // (e.g. an old record) must not have an unrelated tail stripped.
        let got = dest_dir_from_install_path(Path::new("/lib/kq5"), Some("EGA"));
        assert_eq!(got, PathBuf::from("/lib/kq5"));
    }

    #[test]
    fn uninstall_removes_record_only_when_delete_files_false() {
        let installs = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let game = library.path().join("kq5");
        std::fs::create_dir(&game).unwrap();
        std::fs::write(game.join("SIERRA.BAT"), b"").unwrap();
        let record_path =
            crate::install_record::register("kq5", "reliquaint-core", &game, installs.path())
                .unwrap();
        assert!(record_path.is_file());

        let outcome = uninstall("kq5", &game, false, installs.path()).unwrap();
        assert_eq!(outcome.deleted_dir, None);
        assert!(!record_path.exists(), "record should be removed");
        assert!(
            game.exists(),
            "files should be kept when delete_files is false"
        );
    }

    #[test]
    fn uninstall_removes_record_and_dir_when_delete_files_true() {
        let installs = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let game = library.path().join("kq5");
        std::fs::create_dir(&game).unwrap();
        std::fs::write(game.join("SIERRA.BAT"), b"").unwrap();
        let record_path =
            crate::install_record::register("kq5", "reliquaint-core", &game, installs.path())
                .unwrap();

        let outcome = uninstall("kq5", &game, true, installs.path()).unwrap();
        assert_eq!(outcome.deleted_dir, Some(game.clone()));
        assert!(!record_path.exists());
        assert!(!game.exists(), "dest_dir should be deleted");
    }

    #[test]
    fn uninstall_missing_dir_is_ok_with_delete_files() {
        let installs = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let game = library.path().join("kq5");
        std::fs::create_dir(&game).unwrap();
        crate::install_record::register("kq5", "reliquaint-core", &game, installs.path()).unwrap();
        std::fs::remove_dir_all(&game).unwrap();

        // dest_dir no longer exists; uninstall must still succeed (idempotent).
        let outcome = uninstall("kq5", &game, true, installs.path()).unwrap();
        assert_eq!(outcome.deleted_dir, Some(game));
    }

    #[test]
    fn uninstall_missing_record_is_ok() {
        let installs = tempfile::tempdir().unwrap();
        let game = tempfile::tempdir().unwrap();
        uninstall("kq5", game.path(), false, installs.path()).unwrap();
    }
}
