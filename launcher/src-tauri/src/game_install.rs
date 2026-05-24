//! Game installation: copy/extract a user-supplied source into a managed
//! library directory (default `~/games/<id>`), so the resulting directory
//! can be recorded as an install record's `install_path`.
//!
//! This module is the generic, catalog-driven replacement for the
//! pre-redesign per-collection installer. It is split into a pure planning
//! step ([`plan_install`]) — which classifies the source, computes the
//! destination, and builds the copy/extract commands without touching the
//! filesystem beyond classification — and an executor ([`execute`]) that
//! either runs those shell commands through an injected runner (so the
//! CLI / GUI can stream output) or unzips an `.rp9` bundle in process.
//!
//! Supported sources:
//! - **directory** (any platform) → recursive copy
//! - **`.exe`** (DOS) → `innoextract` into the destination
//! - **`.adf` / `.hdf`** (Amiga) → copy the disk image in
//! - **`.rp9`** (Amiga) → unzip the RetroPlatform bundle into the destination

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

/// How the planned install is carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyAction {
    /// Shell commands to run in order via the injected runner.
    Commands(Vec<Vec<String>>),
    /// Unzip this `.rp9` archive into `dest_dir` in process.
    UnzipRp9 { archive: PathBuf },
}

/// A fully-resolved install, ready to [`execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub kind: SourceKind,
    /// `<dest_base>/<id>` — what gets populated.
    pub dest_dir: PathBuf,
    /// The directory recorded as `install_path`: `dest_dir` or
    /// `dest_dir/<subdir>` when the entry declares a `subdir`.
    pub install_path: PathBuf,
    pub action: CopyAction,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("source path {path} does not exist")]
    SourceMissing { path: PathBuf },

    #[error(
        "unsupported source {path} for {platform:?}: expected a directory{hint}"
    )]
    UnsupportedSource {
        path: PathBuf,
        platform: Platform,
        hint: &'static str,
    },

    #[error("destination {path} already exists and is not empty")]
    DestinationOccupied { path: PathBuf },

    #[error("install command failed with exit code {code}")]
    CommandFailed { code: i32 },

    #[error("install command could not run: {message}")]
    CommandSpawn { message: String },

    #[error("failed to create {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to extract rp9 {path}: {message}")]
    Rp9 { path: PathBuf, message: String },
}

/// Classify `source` for `platform`, compute the destination under
/// `dest_base`, and build the copy/extract plan. Reads the filesystem only
/// to classify the source (file vs directory) and to check destination
/// occupancy; performs no copying.
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

    let dest_dir = dest_base.join(spec.id);
    if dir_is_nonempty(&dest_dir) {
        return Err(InstallError::DestinationOccupied { path: dest_dir });
    }

    let install_path = match spec.subdir {
        Some(sub) => dest_dir.join(sub),
        None => dest_dir.clone(),
    };

    let action = match kind {
        SourceKind::Directory => CopyAction::Commands(vec![
            mkdir_p(&dest_dir),
            vec![
                "cp".into(),
                "-r".into(),
                "--".into(),
                format!("{}/.", source.to_string_lossy()),
                dest_dir.to_string_lossy().into_owned(),
            ],
        ]),
        SourceKind::DosInstaller => CopyAction::Commands(vec![
            mkdir_p(&dest_dir),
            vec![
                "innoextract".into(),
                "--exclude-temp".into(),
                "--silent".into(),
                "--output-dir".into(),
                dest_dir.to_string_lossy().into_owned(),
                source.to_string_lossy().into_owned(),
            ],
        ]),
        SourceKind::AmigaImage => amiga_action(spec, source, &dest_dir),
    };

    Ok(InstallPlan {
        kind,
        dest_dir,
        install_path,
        action,
    })
}

/// Carry out a planned install. `run_commands` runs a batch of argv vectors
/// in order and returns the final exit code (the CLI prints to stderr; the
/// GUI streams Tauri events). `.rp9` bundles are unzipped in process and
/// bypass the runner.
pub fn execute<R>(plan: &InstallPlan, run_commands: R) -> Result<(), InstallError>
where
    R: FnOnce(&[Vec<String>]) -> Result<i32, String>,
{
    match &plan.action {
        CopyAction::Commands(cmds) => match run_commands(cmds) {
            Ok(0) => Ok(()),
            Ok(code) => Err(InstallError::CommandFailed { code }),
            Err(message) => Err(InstallError::CommandSpawn { message }),
        },
        CopyAction::UnzipRp9 { archive } => unzip_rp9(archive, &plan.dest_dir),
    }
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

/// Build the action for an Amiga disk-image source. `.rp9` is unzipped;
/// `.adf`/`.hdf` are copied in. When the entry declares exactly one disk of
/// the matching kind, the copied file is renamed to that declared name so a
/// differently-named source still launches.
fn amiga_action(spec: &EntrySpec, source: &Path, dest_dir: &Path) -> CopyAction {
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
        mkdir_p(dest_dir),
        vec![
            "cp".into(),
            "--".into(),
            source.to_string_lossy().into_owned(),
            dest_dir.join(target_name).to_string_lossy().into_owned(),
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

    #[test]
    fn directory_source_plans_recursive_copy() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("GAME.EXE"), b"x").unwrap();
        let base = tempfile::tempdir().unwrap();

        let plan = plan_install(&spec("kq5", Platform::Dos), src.path(), base.path()).unwrap();

        assert_eq!(plan.kind, SourceKind::Directory);
        assert_eq!(plan.dest_dir, base.path().join("kq5"));
        assert_eq!(plan.install_path, base.path().join("kq5"));
        match &plan.action {
            CopyAction::Commands(cmds) => {
                assert_eq!(cmds[0][0], "mkdir");
                assert_eq!(cmds.last().unwrap()[0], "cp");
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn dos_exe_plans_innoextract_with_subdir() {
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
        assert_eq!(plan.dest_dir, base.path().join("qfg1-ega"));
        assert_eq!(plan.install_path, base.path().join("qfg1-ega").join("EGA"));
        match &plan.action {
            CopyAction::Commands(cmds) => {
                assert!(cmds.iter().any(|c| c[0] == "innoextract"));
                assert!(cmds
                    .iter()
                    .any(|c| c.iter().any(|a| a.ends_with("qfg1.exe"))));
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    #[test]
    fn amiga_adf_renames_to_declared_floppy() {
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

        assert_eq!(plan.kind, SourceKind::AmigaImage);
        match &plan.action {
            CopyAction::Commands(cmds) => {
                let cp = cmds.iter().find(|c| c[0] == "cp").unwrap();
                assert!(
                    cp.last().unwrap().ends_with("fatman/fatman.adf"),
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
                    cp.last().unwrap().ends_with("lemmings/lemmings.adf"),
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
                assert!(cp.last().unwrap().ends_with("wb/system.hdf"), "got {cp:?}");
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
        assert!(matches!(err, InstallError::UnsupportedSource { .. }), "got {err:?}");
    }

    #[test]
    fn adf_on_dos_is_unsupported() {
        let tmp = tempfile::tempdir().unwrap();
        let adf = tmp.path().join("game.adf");
        std::fs::write(&adf, b"x").unwrap();
        let base = tempfile::tempdir().unwrap();

        let err = plan_install(&spec("game", Platform::Dos), &adf, base.path()).unwrap_err();
        assert!(matches!(err, InstallError::UnsupportedSource { .. }), "got {err:?}");
    }

    #[test]
    fn missing_source_errors() {
        let base = tempfile::tempdir().unwrap();
        let err =
            plan_install(&spec("game", Platform::Dos), Path::new("/no/such/path"), base.path())
                .unwrap_err();
        assert!(matches!(err, InstallError::SourceMissing { .. }), "got {err:?}");
    }

    #[test]
    fn occupied_destination_errors() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a"), b"x").unwrap();
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(base.path().join("kq5")).unwrap();
        std::fs::write(base.path().join("kq5/existing"), b"x").unwrap();

        let err = plan_install(&spec("kq5", Platform::Dos), src.path(), base.path()).unwrap_err();
        assert!(matches!(err, InstallError::DestinationOccupied { .. }), "got {err:?}");
    }

    #[test]
    fn execute_runs_commands_in_order() {
        let plan = InstallPlan {
            kind: SourceKind::Directory,
            dest_dir: PathBuf::from("/x/kq5"),
            install_path: PathBuf::from("/x/kq5"),
            action: CopyAction::Commands(vec![vec!["mkdir".into(), "-p".into(), "/x/kq5".into()]]),
        };
        let captured = std::cell::RefCell::new(Vec::new());
        execute(&plan, |cmds| {
            captured.borrow_mut().extend_from_slice(cmds);
            Ok(0)
        })
        .unwrap();
        assert_eq!(captured.borrow().len(), 1);
    }

    #[test]
    fn execute_propagates_nonzero_exit() {
        let plan = InstallPlan {
            kind: SourceKind::DosInstaller,
            dest_dir: PathBuf::from("/x/q"),
            install_path: PathBuf::from("/x/q"),
            action: CopyAction::Commands(vec![vec!["false".into()]]),
        };
        let err = execute(&plan, |_| Ok(7)).unwrap_err();
        assert!(matches!(err, InstallError::CommandFailed { code: 7 }), "got {err:?}");
    }

    #[test]
    fn execute_unzips_rp9_into_dest() {
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
        let dest = tmp.path().join("dest");
        let plan = InstallPlan {
            kind: SourceKind::AmigaImage,
            dest_dir: dest.clone(),
            install_path: dest.clone(),
            action: CopyAction::UnzipRp9 { archive: rp9 },
        };

        execute(&plan, |_| Ok(0)).unwrap();

        let inner = std::fs::read(dest.join("inner.adf")).unwrap();
        assert_eq!(inner, b"ADFDATA");
    }
}
