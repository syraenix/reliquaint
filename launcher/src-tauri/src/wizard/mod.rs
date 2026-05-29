//! Manifest creation wizard. Glues the heuristic engine
//! ([`crate::heuristic`]) and the draft composer ([`crate::draft`])
//! into a single user-facing flow that the CLI (M4) and the GUI (M5)
//! both consume.
//!
//! The wizard's I/O surface is intentionally small: a `Prompter` for
//! interactive disambiguation, a path for the user tap root, and a
//! path for install records. Everything else is pure inspection.

use crate::catalog::{CatalogEntry, Platform};
use crate::heuristic::{analyze, HeuristicReport};
use std::path::{Path, PathBuf};

pub mod editor;
pub mod prompts;
pub mod write;

pub use prompts::{Prompter, ScriptedPrompter};

#[derive(Debug, thiserror::Error)]
pub enum WizardError {
    #[error("source directory not found: {0}")]
    SourceMissing(PathBuf),

    #[error("source path is not a directory: {0}")]
    SourceNotDir(PathBuf),

    #[error("a user-tap entry with id {0:?} already exists; remove it first")]
    UserEntryExists(String),

    #[error("id {0:?} is already used by the bundled catalog; pick a different one")]
    BundledEntryExists(String),

    #[error(transparent)]
    Draft(#[from] crate::draft::DraftError),

    #[error(transparent)]
    Tap(#[from] crate::tap::TapError),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("write failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("prompt cancelled")]
    Cancelled,

    #[error("editor failed: {0}")]
    Editor(String),
}

/// Inputs the wizard needs to add a user-created manifest. The CLI
/// command (M4) builds this from its own arg parsing; the GUI command
/// (M5) builds it from the wizard form payload.
pub struct AddOptions<'a> {
    /// Path to the game directory the user wants to add. The install
    /// record points at this path.
    pub source: PathBuf,
    /// Where the user tap lives. Lazily initialized on first write.
    pub user_tap_root: PathBuf,
    /// Where install records live (typically `paths::installs_dir()`).
    pub installs_dir: PathBuf,
    /// Optional platform override — bypasses the heuristic's platform
    /// classification. Useful when the user knows better.
    pub platform_override: Option<Platform>,
    /// Bundled-tap entry ids, for collision detection.
    pub bundled_ids: &'a [String],
    /// Existing user-tap entry ids, for collision detection.
    pub user_ids: &'a [String],
}

/// Result of a successful add: paths the wizard wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddResult {
    pub manifest_path: PathBuf,
    pub config_path: Option<PathBuf>,
    pub install_record_path: PathBuf,
    pub id: String,
    pub platform: Platform,
}

/// Run the heuristic and prepare the draft, without prompting or
/// writing. Returns the report and the draft entry; the caller decides
/// what to show the user before committing.
pub fn prepare(
    source: &Path,
    platform_override: Option<Platform>,
) -> Result<(HeuristicReport, CatalogEntry), WizardError> {
    if !source.exists() {
        return Err(WizardError::SourceMissing(source.to_path_buf()));
    }
    if !source.is_dir() {
        return Err(WizardError::SourceNotDir(source.to_path_buf()));
    }
    let mut report = analyze(source);
    if let Some(p) = platform_override {
        report.platform.platform = Some(p);
    }
    let draft = crate::draft::from_report(&report)?;
    Ok((report, draft))
}

/// Validate the (possibly user-edited) draft against the v0.1 catalog
/// parser. Returns `Validation` on any structural issue.
pub fn validate(draft: &CatalogEntry) -> Result<(), WizardError> {
    let text = toml::to_string_pretty(draft).map_err(|e| WizardError::Validation(e.to_string()))?;
    crate::catalog::parse_str(&text, Path::new("<wizard>/catalog.toml"))
        .map_err(|e| WizardError::Validation(e.to_string()))?;
    Ok(())
}

/// Commit the (validated) draft to disk: write the manifest, the
/// sibling `.conf` for DOS, and the install record. Caller is
/// responsible for collision and validation checks.
pub fn commit(opts: &AddOptions, draft: &CatalogEntry) -> Result<AddResult, WizardError> {
    if opts.user_ids.iter().any(|i| i == &draft.game.id) {
        return Err(WizardError::UserEntryExists(draft.game.id.clone()));
    }
    if opts.bundled_ids.iter().any(|i| i == &draft.game.id) {
        return Err(WizardError::BundledEntryExists(draft.game.id.clone()));
    }
    crate::tap::ensure_user_tap(&opts.user_tap_root)?;
    let platform_dir = match draft.game.platform {
        Platform::Dos => "dos",
        Platform::Amiga => "amiga",
    };
    let catalog_dir = opts.user_tap_root.join("catalog").join(platform_dir);
    std::fs::create_dir_all(&catalog_dir)?;
    let manifest_path = catalog_dir.join(format!("{}.toml", draft.game.id));
    write::write_atomic(
        &manifest_path,
        &toml::to_string_pretty(draft).map_err(|e| WizardError::Validation(e.to_string()))?,
    )?;

    let config_path = if matches!(draft.game.platform, Platform::Dos) {
        let conf_path = catalog_dir.join(format!("{}.conf", draft.game.id));
        write::write_atomic(&conf_path, &crate::draft::dos::default_dosbox_config())?;
        Some(conf_path)
    } else {
        None
    };

    let install_record_path = crate::install_record::register(
        &draft.game.id,
        crate::tap::RESERVED_USER_TAP_ID,
        &opts.source,
        &opts.installs_dir,
    )
    .map_err(|e| WizardError::Io(std::io::Error::other(e.to_string())))?;

    Ok(AddResult {
        manifest_path,
        config_path,
        install_record_path,
        id: draft.game.id.clone(),
        platform: draft.game.platform,
    })
}

/// Locate the on-disk files for a user-created entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryLocations {
    pub manifest_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub install_record_path: Option<PathBuf>,
}

/// Find the user-tap manifest, sibling config, and install record for
/// `id`. Missing files map to `None` so the caller can report partial
/// state honestly.
pub fn locate(id: &str, user_tap_root: &Path, installs_dir: &Path) -> EntryLocations {
    let candidates = [("dos", "toml"), ("amiga", "toml")];
    let mut manifest_path = None;
    let mut config_path = None;
    for (platform, ext) in candidates {
        let p = user_tap_root
            .join("catalog")
            .join(platform)
            .join(format!("{id}.{ext}"));
        if p.is_file() {
            manifest_path = Some(p);
            let c = user_tap_root
                .join("catalog")
                .join(platform)
                .join(format!("{id}.conf"));
            if c.is_file() {
                config_path = Some(c);
            }
            break;
        }
    }
    let install_record = installs_dir.join(format!("{id}.toml"));
    let install_record_path = if install_record.is_file() {
        Some(install_record)
    } else {
        None
    };
    EntryLocations {
        manifest_path,
        config_path,
        install_record_path,
    }
}

/// Remove a user-tap entry's manifest, sibling config, and install
/// record. Caller must verify the entry actually belongs to the user
/// tap (the CLI does so via `CatalogView`).
pub fn remove_files(locations: &EntryLocations) -> Result<(), WizardError> {
    if let Some(p) = &locations.manifest_path {
        std::fs::remove_file(p)?;
    }
    if let Some(p) = &locations.config_path {
        std::fs::remove_file(p)?;
    }
    if let Some(p) = &locations.install_record_path {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture_source(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("SIERRA.BAT"), "").unwrap();
        std::fs::write(dir.join("SIERRA.EXE"), "").unwrap();
        std::fs::write(dir.join("RESOURCE.000"), "").unwrap();
    }

    #[test]
    fn commit_writes_manifest_conf_and_install_record_for_dos() {
        let root = tempdir().unwrap();
        let source = root.path().join("my-custom-game");
        fixture_source(&source);
        let user_tap_root = root.path().join("user-tap");
        let installs_dir = root.path().join("installs");
        std::fs::create_dir_all(&installs_dir).unwrap();

        let (_report, draft) = prepare(&source, None).unwrap();
        validate(&draft).unwrap();
        let result = commit(
            &AddOptions {
                source: source.clone(),
                user_tap_root: user_tap_root.clone(),
                installs_dir: installs_dir.clone(),
                platform_override: None,
                bundled_ids: &[],
                user_ids: &[],
            },
            &draft,
        )
        .unwrap();

        assert_eq!(result.id, "my-custom-game");
        assert_eq!(result.platform, Platform::Dos);
        assert!(result.manifest_path.is_file());
        let conf = result
            .config_path
            .as_ref()
            .expect("DOS draft writes a .conf");
        assert!(conf.is_file());
        let conf_text = std::fs::read_to_string(conf).unwrap();
        assert!(!conf_text.to_ascii_lowercase().contains("[autoexec]"));
        assert!(result.install_record_path.is_file());

        // The written manifest round-trips through the v0.1 parser.
        let loaded = crate::tap::load_user_tap(&user_tap_root).unwrap();
        assert!(loaded.entries.contains_key("my-custom-game"));
    }

    #[test]
    fn commit_rejects_collision_with_bundled_id() {
        let root = tempdir().unwrap();
        let source = root.path().join("qfg1-ega");
        fixture_source(&source);
        let user_tap_root = root.path().join("user-tap");
        let installs_dir = root.path().join("installs");
        std::fs::create_dir_all(&installs_dir).unwrap();

        let (_report, draft) = prepare(&source, None).unwrap();
        let err = commit(
            &AddOptions {
                source,
                user_tap_root,
                installs_dir,
                platform_override: None,
                bundled_ids: &["qfg1-ega".to_string()],
                user_ids: &[],
            },
            &draft,
        )
        .unwrap_err();
        assert!(matches!(err, WizardError::BundledEntryExists(_)));
    }

    #[test]
    fn locate_finds_files_after_commit() {
        let root = tempdir().unwrap();
        let source = root.path().join("my-custom-game");
        fixture_source(&source);
        let user_tap_root = root.path().join("user-tap");
        let installs_dir = root.path().join("installs");
        std::fs::create_dir_all(&installs_dir).unwrap();

        let (_, draft) = prepare(&source, None).unwrap();
        commit(
            &AddOptions {
                source,
                user_tap_root: user_tap_root.clone(),
                installs_dir: installs_dir.clone(),
                platform_override: None,
                bundled_ids: &[],
                user_ids: &[],
            },
            &draft,
        )
        .unwrap();

        let locs = locate("my-custom-game", &user_tap_root, &installs_dir);
        assert!(locs.manifest_path.is_some());
        assert!(locs.config_path.is_some());
        assert!(locs.install_record_path.is_some());

        remove_files(&locs).unwrap();
        let after = locate("my-custom-game", &user_tap_root, &installs_dir);
        assert!(after.manifest_path.is_none());
        assert!(after.config_path.is_none());
        assert!(after.install_record_path.is_none());
    }

    #[test]
    fn prepare_errors_on_missing_source() {
        let err = prepare(Path::new("/nonexistent/path/abc123"), None).unwrap_err();
        assert!(matches!(err, WizardError::SourceMissing(_)));
    }
}
