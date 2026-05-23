//! Catalog entry types and parser. See `docs/schema.md` §"Catalog entry"
//! and `docs/adr-0001-two-layer-manifest-model.md`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A single catalog entry — what the game is and how to run it. Lives in
/// a tap at `catalog/<platform>/<id>.toml`. Carries no user-specific paths.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CatalogEntry {
    pub schema_version: u32,
    pub game: Game,
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub acquisition: Acquisition,
    #[serde(default)]
    pub install: Install,
    pub runtime: Runtime,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub platform: Platform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Dos,
    Amiga,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genre: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Acquisition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Install {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expects_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Runtime {
    pub emulator: Emulator,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidecars: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dosbox: Option<DosboxRuntime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs_uae: Option<FsUaeRuntime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Emulator {
    DosboxStaging,
    FsUae,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DosboxRuntime {
    pub config: String,
    pub entry: String,
    #[serde(default = "default_mount")]
    pub mount: String,
}

fn default_mount() -> String {
    "c".to_string()
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FsUaeRuntime {
    pub model: AmigaModel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub floppies: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AmigaModel {
    A500,
    A600,
    A1200,
    A4000,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("failed to read catalog entry {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse catalog entry {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error(
        "unsupported schema_version {version} in {path} (expected 1 — see docs/schema.md)"
    )]
    UnsupportedSchema { path: PathBuf, version: u32 },

    #[error(
        "invalid game id {id:?} in {path}: must match ^[a-z][a-z0-9-]*[a-z0-9]$ and be \u{2264}64 chars"
    )]
    InvalidId { path: PathBuf, id: String },

    #[error(
        "platform/runtime mismatch in {path}: platform={platform:?} requires [runtime.{required}] block"
    )]
    PlatformRuntimeMismatch {
        path: PathBuf,
        platform: Platform,
        required: &'static str,
    },

    #[error(
        "platform/emulator mismatch in {path}: platform={platform:?} requires emulator={required:?}"
    )]
    PlatformEmulatorMismatch {
        path: PathBuf,
        platform: Platform,
        required: &'static str,
    },
}

/// Read and parse a catalog entry from disk.
pub fn load(path: &Path) -> Result<CatalogEntry, CatalogError> {
    let text = std::fs::read_to_string(path).map_err(|source| CatalogError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_str(&text, path)
}

/// Parse a catalog entry from in-memory TOML text. `path` is used only for
/// error context.
pub fn parse_str(text: &str, path: &Path) -> Result<CatalogEntry, CatalogError> {
    let entry: CatalogEntry = toml::from_str(text).map_err(|source| {
        let owned = path.to_path_buf();
        tracing::error!(path = %owned.display(), error = %source, "catalog entry parse failed");
        CatalogError::Parse { path: owned, source }
    })?;
    validate(&entry, path)?;
    Ok(entry)
}

fn validate(entry: &CatalogEntry, path: &Path) -> Result<(), CatalogError> {
    if entry.schema_version != 1 {
        return Err(CatalogError::UnsupportedSchema {
            path: path.to_path_buf(),
            version: entry.schema_version,
        });
    }
    if !is_valid_id(&entry.game.id) {
        return Err(CatalogError::InvalidId {
            path: path.to_path_buf(),
            id: entry.game.id.clone(),
        });
    }
    match entry.game.platform {
        Platform::Dos => {
            if entry.runtime.dosbox.is_none() {
                return Err(CatalogError::PlatformRuntimeMismatch {
                    path: path.to_path_buf(),
                    platform: Platform::Dos,
                    required: "dosbox",
                });
            }
            if entry.runtime.emulator != Emulator::DosboxStaging {
                return Err(CatalogError::PlatformEmulatorMismatch {
                    path: path.to_path_buf(),
                    platform: Platform::Dos,
                    required: "dosbox-staging",
                });
            }
        }
        Platform::Amiga => {
            if entry.runtime.fs_uae.is_none() {
                return Err(CatalogError::PlatformRuntimeMismatch {
                    path: path.to_path_buf(),
                    platform: Platform::Amiga,
                    required: "fs_uae",
                });
            }
            if entry.runtime.emulator != Emulator::FsUae {
                return Err(CatalogError::PlatformEmulatorMismatch {
                    path: path.to_path_buf(),
                    platform: Platform::Amiga,
                    required: "fs-uae",
                });
            }
        }
    }
    Ok(())
}

/// Returns true if `id` matches `^[a-z][a-z0-9-]*[a-z0-9]$`, has no
/// consecutive hyphens, and is between 2 and 64 characters.
pub(crate) fn is_valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 2 || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let last = bytes[bytes.len() - 1];
    if !(last.is_ascii_lowercase() || last.is_ascii_digit()) {
        return false;
    }
    let mut prev_hyphen = false;
    for &b in bytes {
        match b {
            b'a'..=b'z' | b'0'..=b'9' => prev_hyphen = false,
            b'-' => {
                if prev_hyphen {
                    return false;
                }
                prev_hyphen = true;
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog")
    }

    #[test]
    fn id_validation_accepts_canonical_examples() {
        for id in &["qfg1-ega", "kings-quest-6", "fatman", "reliquaint-core", "kq1sci"] {
            assert!(is_valid_id(id), "{id} should be valid");
        }
    }

    #[test]
    fn id_validation_rejects_bad_forms() {
        let bad = [
            "",
            "a",
            "-leading-hyphen",
            "trailing-hyphen-",
            "double--hyphen",
            "UPPER",
            "with_underscore",
            "with space",
            "0-leading-digit",
        ];
        for id in &bad {
            assert!(!is_valid_id(id), "{id} should be invalid");
        }
    }

    #[test]
    fn loads_dos_fixture_qfg1_ega() {
        let entry = load(&fixtures_dir().join("dos/qfg1-ega.toml"))
            .expect("qfg1-ega fixture should parse");
        assert_eq!(entry.schema_version, 1);
        assert_eq!(entry.game.id, "qfg1-ega");
        assert_eq!(entry.game.platform, Platform::Dos);
        assert_eq!(entry.runtime.emulator, Emulator::DosboxStaging);
        let dosbox = entry.runtime.dosbox.as_ref().expect("dosbox block");
        assert_eq!(dosbox.config, "qfg1-ega.conf");
        assert_eq!(dosbox.entry, "SIERRA.BAT");
        assert_eq!(dosbox.mount, "c");
        assert!(entry.runtime.sidecars.contains(&"fluidsynth".to_string()));
        assert_eq!(entry.meta.year, Some(1989));
    }

    #[test]
    fn loads_amiga_fixture_fatman() {
        let entry = load(&fixtures_dir().join("amiga/fatman.toml"))
            .expect("fatman fixture should parse");
        assert_eq!(entry.game.id, "fatman");
        assert_eq!(entry.game.platform, Platform::Amiga);
        assert_eq!(entry.runtime.emulator, Emulator::FsUae);
        let fs_uae = entry.runtime.fs_uae.as_ref().expect("fs_uae block");
        assert_eq!(fs_uae.model, AmigaModel::A500);
        assert_eq!(fs_uae.floppies, vec!["fatman.adf".to_string()]);
    }

    #[test]
    fn round_trips_dos_fixture() {
        let path = fixtures_dir().join("dos/qfg1-ega.toml");
        let entry = load(&path).unwrap();
        let serialized = toml::to_string(&entry).unwrap();
        let reparsed = parse_str(&serialized, &path).unwrap();
        assert_eq!(entry, reparsed);
    }

    #[test]
    fn round_trips_amiga_fixture() {
        let path = fixtures_dir().join("amiga/fatman.toml");
        let entry = load(&path).unwrap();
        let serialized = toml::to_string(&entry).unwrap();
        let reparsed = parse_str(&serialized, &path).unwrap();
        assert_eq!(entry, reparsed);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let text = r#"
schema_version = 2

[game]
id = "qfg1-ega"
title = "QFG1"
platform = "dos"

[runtime]
emulator = "dosbox-staging"

[runtime.dosbox]
config = "x.conf"
entry = "X.BAT"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, CatalogError::UnsupportedSchema { version: 2, .. }));
    }

    #[test]
    fn rejects_invalid_id() {
        let text = r#"
schema_version = 1

[game]
id = "Bad ID"
title = "X"
platform = "dos"

[runtime]
emulator = "dosbox-staging"

[runtime.dosbox]
config = "x.conf"
entry = "X.BAT"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, CatalogError::InvalidId { .. }));
    }

    #[test]
    fn rejects_dos_platform_with_fs_uae_runtime() {
        let text = r#"
schema_version = 1

[game]
id = "wrong"
title = "X"
platform = "dos"

[runtime]
emulator = "fs-uae"

[runtime.fs_uae]
model = "a500"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, CatalogError::PlatformRuntimeMismatch { .. }));
    }

    #[test]
    fn rejects_amiga_platform_with_dosbox_emulator() {
        let text = r#"
schema_version = 1

[game]
id = "wrong"
title = "X"
platform = "amiga"

[runtime]
emulator = "dosbox-staging"

[runtime.fs_uae]
model = "a500"
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, CatalogError::PlatformEmulatorMismatch { .. }));
    }
}
