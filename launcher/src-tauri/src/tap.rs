//! Tap metadata types and parser. See `docs/schema.md` §"tap.toml" and
//! `docs/adr-0003-tap-based-distribution.md`.
//!
//! A tap is a versioned source of catalog entries and (in future)
//! companion content. This module only parses the tap's metadata file;
//! Task 2.1 walks the tap's `catalog/<platform>/` directories.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

    #[error(
        "unsupported schema_version {version} in {path} (expected 1 — see docs/schema.md)"
    )]
    UnsupportedSchema { path: PathBuf, version: u32 },

    #[error(
        "invalid tap id {id:?} in {path}: must match ^[a-z][a-z0-9-]*[a-z0-9]$ and be \u{2264}64 chars"
    )]
    InvalidId { path: PathBuf, id: String },
}

/// Read and parse a tap.toml from disk.
pub fn load(path: &Path) -> Result<TapMetadata, TapError> {
    let text = std::fs::read_to_string(path).map_err(|source| TapError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_str(&text, path)
}

/// Parse tap metadata from in-memory TOML text. `path` is used only for
/// error context.
pub fn parse_str(text: &str, path: &Path) -> Result<TapMetadata, TapError> {
    let meta: TapMetadata = toml::from_str(text).map_err(|source| {
        let owned = path.to_path_buf();
        tracing::error!(path = %owned.display(), error = %source, "tap metadata parse failed");
        TapError::Parse { path: owned, source }
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
        assert!(matches!(err, TapError::UnsupportedSchema { version: 2, .. }));
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
