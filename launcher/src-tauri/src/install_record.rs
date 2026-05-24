//! Installation record types and parser. See `docs/schema.md`
//! §"Installation record" and `docs/adr-0001-two-layer-manifest-model.md`.
//!
//! Per-user, per-machine state. Written by the install flow, read at every
//! launch. Lives under `${XDG_DATA_HOME}/reliquaint/installs/<id>.toml`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

/// A successfully parsed install record paired with its source path on
/// disk. Returned by [`load_all`].
#[derive(Debug, Clone)]
pub struct LoadedInstallRecord {
    pub source_path: PathBuf,
    pub record: InstallRecord,
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

    #[error(
        "install_path {install_path} in {path} is not absolute (after ~ expansion); \
         docs/schema.md requires an absolute path"
    )]
    NonAbsoluteInstallPath { path: PathBuf, install_path: PathBuf },

    #[error("cannot access {path}: {source}")]
    PathAccess {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not a directory")]
    NotADirectory { path: PathBuf },

    #[error("failed to create installs directory {path}: {source}")]
    CreateInstallsDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("internal error formatting timestamp: {source}")]
    TimestampFormat {
        #[source]
        source: toml::value::DatetimeParseError,
    },
}

/// Write an install record for a directory that has already been populated
/// by the install flow (copy/extract) or is already present at the managed
/// location. Canonicalizes `install_path` (so it is absolute, as the schema
/// requires) and verifies it is a directory, then writes
/// `installs_dir/<catalog_id>.toml`. Returns the record path.
///
/// Unlike the removed `install()`, this performs no `expects_files`
/// validation — that is the orchestrator's job, after the copy/extract.
pub fn register(
    catalog_id: &str,
    tap_id: &str,
    install_path: &Path,
    installs_dir: &Path,
) -> Result<PathBuf, InstallError> {
    let abs = install_path
        .canonicalize()
        .map_err(|source| InstallError::PathAccess {
            path: install_path.to_path_buf(),
            source,
        })?;
    if !abs.is_dir() {
        return Err(InstallError::NotADirectory { path: abs });
    }

    let installed_at = toml::value::Datetime::from_str(&now_iso8601())
        .map_err(|source| InstallError::TimestampFormat { source })?;

    let record = InstallRecord {
        schema_version: 1,
        install: Install {
            catalog_id: catalog_id.to_string(),
            tap: tap_id.to_string(),
            install_path: abs,
            installed_at,
        },
    };

    std::fs::create_dir_all(installs_dir).map_err(|source| InstallError::CreateInstallsDir {
        path: installs_dir.to_path_buf(),
        source,
    })?;
    let record_path = installs_dir.join(format!("{catalog_id}.toml"));
    write(&record, &record_path)?;
    Ok(record_path)
}

/// Case-insensitive check for each expected filename at the top level of
/// `install_dir`. Returns the names that weren't found.
pub fn missing_expects_files(install_dir: &Path, expected: &[String]) -> Vec<String> {
    let entries: Vec<String> = match std::fs::read_dir(install_dir) {
        Ok(read) => read
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect(),
        Err(_) => Vec::new(),
    };
    expected
        .iter()
        .filter(|exp| !entries.iter().any(|e| e.eq_ignore_ascii_case(exp)))
        .cloned()
        .collect()
}

/// Current UTC time formatted as ISO 8601 (YYYY-MM-DDTHH:MM:SSZ), built
/// from libc::gmtime_r to avoid pulling in chrono / time crates.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: gmtime_r is reentrant and takes valid pointers to a
    // time_t and a tm we own.
    unsafe {
        libc::gmtime_r(&secs, &mut tm);
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
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
    if !record.install.install_path.is_absolute() {
        return Err(InstallError::NonAbsoluteInstallPath {
            path: path.to_path_buf(),
            install_path: record.install.install_path.clone(),
        });
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

/// Scan `dir` for `*.toml` install records and return every one that
/// parses. Records that fail to parse are logged at WARN and skipped (a
/// single bad record can't break the launcher's view of the rest).
///
/// A missing directory returns an empty Vec — the user simply hasn't
/// installed anything yet; that is not an error.
///
/// Does **not** check whether each record's `install_path` exists, nor
/// whether the referenced `(tap, catalog_id)` actually appears in any
/// loaded catalog. Orphan flagging is the join layer's job
/// (`catalog_view::CatalogView`).
pub fn load_all(dir: &Path) -> Vec<LoadedInstallRecord> {
    if !dir.is_dir() {
        tracing::debug!(dir = %dir.display(), "installs directory absent; treating as empty");
        return Vec::new();
    }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                dir = %dir.display(),
                error = %e,
                "failed to read installs directory; treating as empty"
            );
            return Vec::new();
        }
    };

    let mut paths = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".toml") {
            continue;
        }
        paths.push(path);
    }
    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        match load(&path) {
            Ok(record) => out.push(LoadedInstallRecord {
                source_path: path,
                record,
            }),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "skipping unparseable install record"
                );
            }
        }
    }
    tracing::info!(dir = %dir.display(), count = out.len(), "loaded install records");
    out
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
    fn rejects_relative_install_path() {
        let text = r#"
schema_version = 1

[install]
catalog_id   = "qfg1-ega"
tap          = "reliquaint-core"
install_path = "games/qfg1-ega"
installed_at = 2026-05-23T14:32:00Z
"#;
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(
            matches!(err, InstallError::NonAbsoluteInstallPath { .. }),
            "relative install_path should be rejected, got {err:?}"
        );
    }

    fn write_sample_record(dir: &Path, filename: &str, catalog_id: &str) {
        let body = format!(
            r#"schema_version = 1

[install]
catalog_id   = "{catalog_id}"
tap          = "reliquaint-core"
install_path = "/home/test/games/{catalog_id}"
installed_at = 2026-05-23T14:32:00Z
"#
        );
        std::fs::write(dir.join(filename), body).unwrap();
    }

    #[test]
    fn load_all_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        write_sample_record(tmp.path(), "qfg1-ega.toml", "qfg1-ega");
        write_sample_record(tmp.path(), "fatman.toml", "fatman");

        let loaded = load_all(tmp.path());
        assert_eq!(loaded.len(), 2);
        let ids: Vec<&str> = loaded
            .iter()
            .map(|l| l.record.install.catalog_id.as_str())
            .collect();
        // load_all sorts source paths alphabetically (fatman before qfg).
        assert_eq!(ids, vec!["fatman", "qfg1-ega"]);
    }

    #[test]
    fn load_all_missing_dir_returns_empty() {
        let loaded = load_all(Path::new("/definitely/not/a/real/installs/dir"));
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_all_skips_unparseable_records_with_warning() {
        let tmp = tempfile::tempdir().unwrap();
        write_sample_record(tmp.path(), "good.toml", "qfg1-ega");
        std::fs::write(tmp.path().join("broken.toml"), "not valid toml [[[").unwrap();

        let loaded = load_all(tmp.path());
        assert_eq!(loaded.len(), 1, "valid record should still load");
        assert_eq!(loaded[0].record.install.catalog_id, "qfg1-ega");
    }

    #[test]
    fn load_all_ignores_non_toml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_sample_record(tmp.path(), "qfg1-ega.toml", "qfg1-ega");
        std::fs::write(tmp.path().join("readme.txt"), "not a record").unwrap();
        std::fs::write(tmp.path().join("notes.md"), "not a record").unwrap();

        let loaded = load_all(tmp.path());
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn register_writes_record_for_existing_dir() {
        let installs = tempfile::tempdir().unwrap();
        let game = tempfile::tempdir().unwrap();
        let record_path =
            register("kq5", "reliquaint-core", game.path(), installs.path()).unwrap();
        assert!(record_path.is_file());
        let r = load(&record_path).unwrap();
        assert_eq!(r.install.catalog_id, "kq5");
        assert_eq!(r.install.tap, "reliquaint-core");
        assert_eq!(r.install.install_path, game.path().canonicalize().unwrap());
    }

    #[test]
    fn register_errors_when_path_missing() {
        let installs = tempfile::tempdir().unwrap();
        let err = register(
            "kq5",
            "reliquaint-core",
            Path::new("/no/such/dir/xyz123"),
            installs.path(),
        )
        .unwrap_err();
        assert!(matches!(err, InstallError::PathAccess { .. }));
    }

    #[test]
    fn missing_expects_files_is_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("sierra.bat"), b"").unwrap();
        let missing = missing_expects_files(tmp.path(), &["SIERRA.BAT".into()]);
        assert!(missing.is_empty(), "should match case-insensitively");
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
