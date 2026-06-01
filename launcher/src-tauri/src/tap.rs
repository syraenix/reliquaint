//! Tap metadata types and parser. See `docs/schema.md` §"tap.toml" and ADR-0003.
//!
//! A tap is a versioned source of catalog entries and (in future)
//! companion content. This module only parses the tap's metadata file;
//! Task 2.1 walks the tap's `catalog/<platform>/` directories.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Tap id reserved for the user-writable local pseudo-tap. Any externally
/// supplied tap claiming this id is rejected on load (see `load_tap`).
pub const RESERVED_USER_TAP_ID: &str = "local";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TapMetadata {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub maintainer: String,
    pub url: String,
    pub license: String,
}

/// A tap fully loaded from disk: metadata plus every catalog entry found
/// under `catalog/<platform>/`. BTreeMap gives deterministic iteration
/// order keyed by game id, so downstream listing is stable without an
/// explicit sort.
#[derive(Debug, Clone)]
pub struct LoadedTap {
    pub metadata: TapMetadata,
    pub root: PathBuf,
    pub entries: BTreeMap<String, LoadedEntry>,
}

#[derive(Debug, Clone)]
pub struct LoadedEntry {
    pub source_path: PathBuf,
    pub entry: crate::catalog::CatalogEntry,
}

#[derive(Debug, thiserror::Error)]
pub enum TapError {
    #[error("failed to read tap metadata {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse tap metadata {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("unsupported schema_version {version} in {path} (expected 1 — see docs/schema.md)")]
    UnsupportedSchema { path: PathBuf, version: u32 },

    #[error(
        "invalid tap id {id:?} in {path}: must match ^[a-z][a-z0-9-]*[a-z0-9]$ and be \u{2264}64 chars"
    )]
    InvalidId { path: PathBuf, id: String },

    #[error("tap root not found: {root}")]
    MissingRoot { root: PathBuf },

    #[error("failed to read catalog directory {dir}: {source}")]
    CatalogDirRead {
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("duplicate game id {game_id:?} in tap {tap_id:?}: {first} and {second}")]
    DuplicateGameId {
        tap_id: String,
        game_id: String,
        first: PathBuf,
        second: PathBuf,
    },

    #[error(
        "tap id {id:?} in {path} is reserved for the user tap; external taps must choose a different id"
    )]
    ReservedTapId { path: PathBuf, id: String },

    #[error("user tap at {path} has id {id:?}; must be {expected:?}")]
    UnexpectedUserTapId {
        path: PathBuf,
        id: String,
        expected: &'static str,
    },

    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Read and parse a tap.toml from disk.
pub fn load(path: &Path) -> Result<TapMetadata, TapError> {
    let text = std::fs::read_to_string(path).map_err(|source| TapError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_str(&text, path)
}

/// Load an external tap from disk: parses `<root>/tap.toml` and walks
/// `<root>/catalog/<platform>/*.toml` for entries.
///
/// - Per-entry parse failures are logged at WARN and skipped — they are
///   not fatal, so a single broken entry can't take down `reliquaint list`.
/// - A duplicate game id within the tap IS fatal — silently ignoring would
///   lose data.
/// - Files whose name starts with `_` or `.` are skipped so contributors
///   can stash drafts in the tree without breaking startup.
/// - Taps claiming the reserved id `local` are rejected; that id belongs
///   to the user-writable tap loaded via [`load_user_tap`].
pub fn load_tap(root: &Path) -> Result<LoadedTap, TapError> {
    let loaded = load_tap_inner(root)?;
    if loaded.metadata.id == RESERVED_USER_TAP_ID {
        return Err(TapError::ReservedTapId {
            path: root.join("tap.toml"),
            id: loaded.metadata.id,
        });
    }
    Ok(loaded)
}

/// Load the user-writable local tap. Behaves like [`load_tap`] but
/// requires the tap to claim the reserved id `local`; any other id is
/// rejected.
pub fn load_user_tap(root: &Path) -> Result<LoadedTap, TapError> {
    let loaded = load_tap_inner(root)?;
    if loaded.metadata.id != RESERVED_USER_TAP_ID {
        return Err(TapError::UnexpectedUserTapId {
            path: root.join("tap.toml"),
            id: loaded.metadata.id,
            expected: RESERVED_USER_TAP_ID,
        });
    }
    Ok(loaded)
}

fn load_tap_inner(root: &Path) -> Result<LoadedTap, TapError> {
    if !root.is_dir() {
        return Err(TapError::MissingRoot {
            root: root.to_path_buf(),
        });
    }
    let metadata = load(&root.join("tap.toml"))?;
    let tap_id = metadata.id.clone();
    let mut entries: BTreeMap<String, LoadedEntry> = BTreeMap::new();

    for platform_dir in ["dos", "amiga"] {
        let dir = root.join("catalog").join(platform_dir);
        if !dir.is_dir() {
            continue;
        }
        let mut paths = collect_entry_paths(&dir)?;
        paths.sort();
        for path in paths {
            match crate::catalog::load(&path) {
                Ok(entry) => {
                    let game_id = entry.game.id.clone();
                    if let Some(existing) = entries.get(&game_id) {
                        return Err(TapError::DuplicateGameId {
                            tap_id,
                            game_id,
                            first: existing.source_path.clone(),
                            second: path,
                        });
                    }
                    entries.insert(
                        game_id,
                        LoadedEntry {
                            source_path: path,
                            entry,
                        },
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "skipping unparseable catalog entry"
                    );
                }
            }
        }
    }

    tracing::info!(
        tap_id = %tap_id,
        root = %root.display(),
        count = entries.len(),
        "loaded tap"
    );
    Ok(LoadedTap {
        metadata,
        root: root.to_path_buf(),
        entries,
    })
}

/// Validate a fetched tap directory before registering it.
///
/// Checks that `tap.toml` is present and valid (fatal) and walks catalog
/// entries, logging WARN for any that fail to parse (non-fatal).
///
/// `expected_id`: when `Some`, logs a warning if the tap's actual id differs
/// (non-fatal — used during `tap sync` to detect upstream renames without
/// breaking the user's existing subscription).
pub fn validate_tap_dir(root: &Path, expected_id: Option<&str>) -> Result<TapMetadata, TapError> {
    let meta = load_tap_inner(root)?.metadata;
    if let Some(expected) = expected_id {
        if meta.id != expected {
            tracing::warn!(
                "tap id mismatch: expected {:?}, got {:?}. \
                 Keeping local id. Use 'tap remove' then 'tap add' to reset.",
                expected,
                meta.id
            );
        }
    }
    Ok(meta)
}

/// Lazily initialize the user tap on disk and return its metadata.
///
/// If `<root>/tap.toml` already exists, parses and returns it (and
/// rejects unexpected ids). Otherwise creates `<root>` and writes a
/// default `tap.toml` claiming the reserved id `local`.
///
/// The catalog subdirectories (`<root>/catalog/<platform>/`) are NOT
/// created here — the wizard creates the platform dir it needs when it
/// writes the first manifest.
pub fn ensure_user_tap(root: &Path) -> Result<TapMetadata, TapError> {
    let metadata_path = root.join("tap.toml");
    if metadata_path.is_file() {
        let meta = load(&metadata_path)?;
        if meta.id != RESERVED_USER_TAP_ID {
            return Err(TapError::UnexpectedUserTapId {
                path: metadata_path,
                id: meta.id,
                expected: RESERVED_USER_TAP_ID,
            });
        }
        return Ok(meta);
    }
    std::fs::create_dir_all(root).map_err(|source| TapError::Write {
        path: root.to_path_buf(),
        source,
    })?;
    let meta = default_user_tap_metadata();
    let text = toml::to_string_pretty(&meta).expect("TapMetadata serializes");
    std::fs::write(&metadata_path, text).map_err(|source| TapError::Write {
        path: metadata_path,
        source,
    })?;
    tracing::info!(root = %root.display(), "initialized user tap");
    Ok(meta)
}

fn default_user_tap_metadata() -> TapMetadata {
    TapMetadata {
        schema_version: 1,
        id: RESERVED_USER_TAP_ID.to_string(),
        title: "Local games".to_string(),
        description: "User-created catalog entries managed by the Reliquaint wizard.".to_string(),
        version: "0.1.0".to_string(),
        maintainer: "local".to_string(),
        url: "https://github.com/syraenix/reliquaint".to_string(),
        license: "CC0-1.0".to_string(),
    }
}

fn collect_entry_paths(dir: &Path) -> Result<Vec<PathBuf>, TapError> {
    let read = std::fs::read_dir(dir).map_err(|source| TapError::CatalogDirRead {
        dir: dir.to_path_buf(),
        source,
    })?;
    let mut out = Vec::new();
    for entry in read {
        let entry = entry.map_err(|source| TapError::CatalogDirRead {
            dir: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if !name.ends_with(".toml") {
            continue;
        }
        out.push(path);
    }
    Ok(out)
}

/// Parse tap metadata from in-memory TOML text. `path` is used only for
/// error context.
pub fn parse_str(text: &str, path: &Path) -> Result<TapMetadata, TapError> {
    let meta: TapMetadata = toml::from_str(text).map_err(|source| {
        let owned = path.to_path_buf();
        tracing::error!(path = %owned.display(), error = %source, "tap metadata parse failed");
        TapError::Parse {
            path: owned,
            source,
        }
    })?;
    if meta.schema_version != 1 {
        return Err(TapError::UnsupportedSchema {
            path: path.to_path_buf(),
            version: meta.schema_version,
        });
    }
    if !crate::catalog::is_valid_id(&meta.id) {
        return Err(TapError::InvalidId {
            path: path.to_path_buf(),
            id: meta.id.clone(),
        });
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tap/tap.toml")
    }

    fn write_tap_metadata(root: &Path, id: &str) {
        std::fs::write(
            root.join("tap.toml"),
            format!(
                r#"schema_version = 1
id          = "{id}"
title       = "Test Tap"
description = "test"
version     = "0.1.0"
maintainer  = "test"
url         = "https://example.test"
license     = "MIT"
"#
            ),
        )
        .unwrap();
    }

    fn write_catalog_entry(root: &Path, platform_dir: &str, id: &str) {
        let dir = root.join("catalog").join(platform_dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = if platform_dir == "dos" {
            format!(
                r#"schema_version = 1
[game]
id = "{id}"
title = "Test Game"
platform = "dos"
[runtime]
emulator = "dosbox-staging"
[runtime.dosbox]
config = "{id}.conf"
entry = "TEST.EXE"
"#
            )
        } else {
            format!(
                r#"schema_version = 1
[game]
id = "{id}"
title = "Test Game"
platform = "amiga"
[runtime]
emulator = "fs-uae"
[runtime.fs_uae]
model = "a500"
"#
            )
        };
        std::fs::write(dir.join(format!("{id}.toml")), body).unwrap();
    }

    #[test]
    fn load_tap_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "qfg1-ega");
        write_catalog_entry(root, "dos", "qfg2");
        write_catalog_entry(root, "amiga", "fatman");

        let loaded = load_tap(root).unwrap();

        assert_eq!(loaded.metadata.id, "test-tap");
        assert_eq!(loaded.entries.len(), 3);
        assert!(loaded.entries.contains_key("qfg1-ega"));
        assert!(loaded.entries.contains_key("qfg2"));
        assert!(loaded.entries.contains_key("fatman"));
        // Iteration order is stable (BTreeMap, alphabetical by game id).
        let ids: Vec<&String> = loaded.entries.keys().collect();
        assert_eq!(ids, vec!["fatman", "qfg1-ega", "qfg2"]);
    }

    #[test]
    fn load_tap_skips_underscore_and_dot_prefixed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "qfg1-ega");
        // Drafts and hidden files alongside the real entry — both have
        // bodies that would fail to parse if the skip logic let them through.
        std::fs::write(
            root.join("catalog/dos/_draft.toml"),
            "this is not valid toml [[[",
        )
        .unwrap();
        std::fs::write(
            root.join("catalog/dos/.hidden.toml"),
            "this is not valid toml [[[",
        )
        .unwrap();

        let loaded = load_tap(root).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key("qfg1-ega"));
    }

    #[test]
    fn load_tap_skips_unparseable_entries_with_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "qfg1-ega");
        std::fs::write(root.join("catalog/dos/broken.toml"), "not valid toml [[[").unwrap();

        let loaded = load_tap(root).unwrap();
        assert_eq!(
            loaded.entries.len(),
            1,
            "valid entry should still load when a sibling is broken"
        );
        assert!(loaded.entries.contains_key("qfg1-ega"));
    }

    #[test]
    fn load_tap_rejects_duplicate_game_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "duplicate");
        write_catalog_entry(root, "amiga", "duplicate");

        let err = load_tap(root).unwrap_err();
        match err {
            TapError::DuplicateGameId {
                tap_id,
                game_id,
                first,
                second,
            } => {
                assert_eq!(tap_id, "test-tap");
                assert_eq!(game_id, "duplicate");
                // DOS dir is walked before Amiga, so DOS path is "first".
                assert!(first.to_string_lossy().contains("/dos/"));
                assert!(second.to_string_lossy().contains("/amiga/"));
            }
            other => panic!("expected DuplicateGameId, got {other:?}"),
        }
    }

    #[test]
    fn load_tap_missing_root_errors() {
        let err = load_tap(Path::new("/definitely/not/a/real/path/for/this/test")).unwrap_err();
        assert!(matches!(err, TapError::MissingRoot { .. }));
    }

    #[test]
    fn loads_bundled_tap_fixture() {
        let meta = load(&fixture_path()).expect("tap fixture should parse");
        assert_eq!(meta.schema_version, 1);
        assert_eq!(meta.id, "reliquaint-core");
        assert_eq!(meta.title, "Reliquaint Core");
        assert_eq!(meta.version, "0.1.0");
        assert_eq!(meta.license, "CC-BY-SA-4.0");
        assert!(meta.url.starts_with("https://"));
        assert!(!meta.description.is_empty());
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let text = r#"
schema_version = 2
id          = "x"
title       = "x"
description = "x"
version     = "0.1.0"
maintainer  = "x"
url         = "https://x"
license     = "x"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(
            err,
            TapError::UnsupportedSchema { version: 2, .. }
        ));
    }

    #[test]
    fn load_tap_rejects_reserved_user_tap_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, RESERVED_USER_TAP_ID);

        let err = load_tap(root).unwrap_err();
        match err {
            TapError::ReservedTapId { id, path } => {
                assert_eq!(id, RESERVED_USER_TAP_ID);
                assert!(path.ends_with("tap.toml"));
            }
            other => panic!("expected ReservedTapId, got {other:?}"),
        }
    }

    #[test]
    fn load_user_tap_requires_reserved_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "not-local");

        let err = load_user_tap(root).unwrap_err();
        match err {
            TapError::UnexpectedUserTapId { id, expected, .. } => {
                assert_eq!(id, "not-local");
                assert_eq!(expected, RESERVED_USER_TAP_ID);
            }
            other => panic!("expected UnexpectedUserTapId, got {other:?}"),
        }
    }

    #[test]
    fn load_user_tap_accepts_reserved_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, RESERVED_USER_TAP_ID);
        write_catalog_entry(root, "dos", "my-custom-game");

        let loaded = load_user_tap(root).unwrap();
        assert_eq!(loaded.metadata.id, RESERVED_USER_TAP_ID);
        assert!(loaded.entries.contains_key("my-custom-game"));
    }

    #[test]
    fn ensure_user_tap_creates_directory_and_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("tap");
        assert!(!root.exists());

        let meta = ensure_user_tap(&root).unwrap();
        assert_eq!(meta.id, RESERVED_USER_TAP_ID);
        assert_eq!(meta.schema_version, 1);
        assert!(root.is_dir());
        assert!(root.join("tap.toml").is_file());

        // load_user_tap reads what ensure_user_tap wrote.
        let loaded = load_user_tap(&root).unwrap();
        assert_eq!(loaded.metadata.id, RESERVED_USER_TAP_ID);
    }

    #[test]
    fn ensure_user_tap_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("tap");
        let first = ensure_user_tap(&root).unwrap();
        let mtime_before = std::fs::metadata(root.join("tap.toml"))
            .unwrap()
            .modified()
            .unwrap();
        let second = ensure_user_tap(&root).unwrap();
        let mtime_after = std::fs::metadata(root.join("tap.toml"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            mtime_before, mtime_after,
            "tap.toml should not be rewritten"
        );
    }

    #[test]
    fn ensure_user_tap_rejects_existing_with_wrong_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "not-local");

        let err = ensure_user_tap(root).unwrap_err();
        assert!(matches!(err, TapError::UnexpectedUserTapId { .. }));
    }

    #[test]
    fn validate_tap_dir_succeeds_on_valid_tap() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "game1");
        let meta = validate_tap_dir(root, None).unwrap();
        assert_eq!(meta.id, "test-tap");
    }

    #[test]
    fn validate_tap_dir_fails_on_missing_tap_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_tap_dir(tmp.path(), None).unwrap_err();
        // MissingRoot because load_tap_inner checks root is a dir (it is), then
        // tries to read tap.toml — this produces a Read error, but since the
        // dir exists it goes to TapError::Read.
        assert!(
            matches!(err, TapError::Read { .. } | TapError::MissingRoot { .. }),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn validate_tap_dir_is_non_fatal_on_id_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        // expected_id differs — should still succeed with a warning
        let meta = validate_tap_dir(root, Some("other-id")).unwrap();
        assert_eq!(meta.id, "test-tap");
    }

    #[test]
    fn validate_tap_dir_skips_bad_catalog_entries_non_fatally() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_tap_metadata(root, "test-tap");
        write_catalog_entry(root, "dos", "good-game");
        std::fs::create_dir_all(root.join("catalog/dos")).unwrap();
        std::fs::write(root.join("catalog/dos/broken.toml"), "not valid [[[[").unwrap();
        // Should succeed — bad entry is a warning, not fatal
        let meta = validate_tap_dir(root, None).unwrap();
        assert_eq!(meta.id, "test-tap");
    }

    #[test]
    fn rejects_invalid_id() {
        let text = r#"
schema_version = 1
id          = "Bad ID"
title       = "x"
description = "x"
version     = "0.1.0"
maintainer  = "x"
url         = "https://x"
license     = "x"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, TapError::InvalidId { .. }));
    }
}
