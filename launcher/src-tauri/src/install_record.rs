//! Installation record types and parser. See `docs/schema.md`
//! §"Installation record" and `docs/adr-0001-two-layer-manifest-model.md`.
//!
//! Per-user, per-machine state. Written by the install flow, read at every
//! launch. Lives under `${XDG_DATA_HOME}/reliquaint/installs/<id>.toml`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct InstallRecord {
    pub schema_version: u32,
    pub install: Install,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Install {
    pub catalog_id: String,
    pub tap: String,
    pub install_path: PathBuf,
    pub installed_at: toml::value::Datetime,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("failed to read install record {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write install record {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse install record {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize install record: {source}")]
    Serialize {
        #[source]
        source: toml::ser::Error,
    },

    #[error(
        "unsupported schema_version {version} in {path} (expected 1 — see docs/schema.md)"
    )]
    UnsupportedSchema { path: PathBuf, version: u32 },
}

/// Read and parse an installation record from disk. Does not check whether
/// `install_path` exists — that's the launch path / `doctor` command's job.
pub fn load(path: &Path) -> Result<InstallRecord, InstallError> {
    let text = std::fs::read_to_string(path).map_err(|source| InstallError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_str(&text, path)
}

/// Parse an installation record from in-memory TOML text. `path` is used
/// only for error context. Tilde-prefixed `install_path` values are
/// expanded in the returned struct; the on-disk record is unchanged.
pub fn parse_str(text: &str, path: &Path) -> Result<InstallRecord, InstallError> {
    let mut record: InstallRecord = toml::from_str(text).map_err(|source| {
        let owned = path.to_path_buf();
        tracing::error!(path = %owned.display(), error = %source, "install record parse failed");
        InstallError::Parse { path: owned, source }
    })?;
    if record.schema_version != 1 {
        return Err(InstallError::UnsupportedSchema {
            path: path.to_path_buf(),
            version: record.schema_version,
        });
    }
    if let Some(s) = record.install.install_path.to_str() {
        if s.starts_with('~') {
            record.install.install_path = PathBuf::from(shellexpand::tilde(s).into_owned());
        }
    }
    Ok(record)
}

/// Serialize an installation record and write it to disk. The caller is
/// responsible for ensuring `install_path` is absolute.
pub fn write(record: &InstallRecord, path: &Path) -> Result<(), InstallError> {
    let text = toml::to_string(record).map_err(|source| InstallError::Serialize { source })?;
    std::fs::write(path, text).map_err(|source| InstallError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn sample_record() -> InstallRecord {
        InstallRecord {
            schema_version: 1,
            install: Install {
                catalog_id: "qfg1-ega".into(),
                tap: "reliquaint-core".into(),
                install_path: PathBuf::from("/home/derek/games/qfg1-ega"),
                installed_at: toml::value::Datetime::from_str("2026-05-23T14:32:00Z").unwrap(),
            },
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("qfg1-ega.toml");
        let record = sample_record();
        write(&record, &path).unwrap();
        let read_back = load(&path).unwrap();
        assert_eq!(record, read_back);
    }

    #[test]
    fn parses_schema_example_verbatim() {
        let text = r#"
schema_version = 1

[install]
catalog_id   = "qfg1-ega"
tap          = "reliquaint-core"
install_path = "/home/derek/games/qfg1-ega"
installed_at = 2026-05-23T14:32:00Z
"#;
        let record = parse_str(text, Path::new("test.toml")).unwrap();
        assert_eq!(record.install.catalog_id, "qfg1-ega");
        assert_eq!(record.install.tap, "reliquaint-core");
        assert_eq!(
            record.install.install_path,
            PathBuf::from("/home/derek/games/qfg1-ega")
        );
    }

    #[test]
    fn load_does_not_check_install_path_existence() {
        let tmp = tempfile::tempdir().unwrap();
        let record_path = tmp.path().join("qfg1-ega.toml");
        let mut record = sample_record();
        record.install.install_path = PathBuf::from("/definitely/does/not/exist/qfg1-ega");
        write(&record, &record_path).unwrap();
        let loaded = load(&record_path).expect("missing install_path must not be an error");
        assert_eq!(loaded.install.install_path, record.install.install_path);
    }

    #[test]
    fn tilde_in_install_path_is_expanded_on_read() {
        let text = r#"
schema_version = 1

[install]
catalog_id   = "qfg1-ega"
tap          = "reliquaint-core"
install_path = "~/games/qfg1-ega"
installed_at = 2026-05-23T14:32:00Z
"#;
        let record = parse_str(text, Path::new("test.toml")).unwrap();
        let s = record.install.install_path.to_string_lossy();
        assert!(!s.starts_with('~'), "tilde should be expanded, got {s}");
        assert!(s.ends_with("/games/qfg1-ega"), "expanded path should retain tail, got {s}");
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let text = r#"
schema_version = 2

[install]
catalog_id   = "qfg1-ega"
tap          = "reliquaint-core"
install_path = "/x"
installed_at = 2026-05-23T14:32:00Z
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, InstallError::UnsupportedSchema { version: 2, .. }));
    }
}
