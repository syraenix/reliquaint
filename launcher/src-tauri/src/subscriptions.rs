use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SubscriptionManifest {
    pub schema_version: u32,
    #[serde(default, rename = "tap")]
    pub taps: Vec<TapSubscription>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct TapSubscription {
    pub id: String,
    pub source: String,
    pub added_at: toml::value::Datetime,
    pub priority: u32,
}

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("schema_version must be {SCHEMA_VERSION}, got {0}")]
    BadSchemaVersion(u32),
    #[error("duplicate tap id: {0}")]
    DuplicateId(String),
    #[error("tap id 'local' is reserved and cannot appear in subscriptions.toml")]
    ReservedId,
    #[error("invalid tap id {0:?}: must be lowercase letters, digits, and hyphens; start and end with a letter or digit")]
    InvalidId(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}

impl SubscriptionManifest {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            taps: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, SubscriptionError> {
        let text = std::fs::read_to_string(path)?;
        Self::parse_str(&text)
    }

    /// Load from disk if the file exists; return an empty manifest otherwise.
    pub fn load_or_empty(path: &Path) -> Result<Self, SubscriptionError> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::empty())
        }
    }

    pub fn parse_str(text: &str) -> Result<Self, SubscriptionError> {
        let m: Self = toml::from_str(text)?;
        m.validate()?;
        Ok(m)
    }

    pub fn write(&self, path: &Path) -> Result<(), SubscriptionError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).expect("SubscriptionManifest always serializes");
        std::fs::write(path, text)?;
        Ok(())
    }

    fn validate(&self) -> Result<(), SubscriptionError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(SubscriptionError::BadSchemaVersion(self.schema_version));
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for tap in &self.taps {
            if tap.id == "local" {
                return Err(SubscriptionError::ReservedId);
            }
            if !is_valid_tap_id(&tap.id) {
                return Err(SubscriptionError::InvalidId(tap.id.clone()));
            }
            if !seen.insert(tap.id.as_str()) {
                return Err(SubscriptionError::DuplicateId(tap.id.clone()));
            }
        }
        Ok(())
    }

    /// Priority to assign when appending a new tap (max existing + 1, or 0 if none).
    pub fn next_priority(&self) -> u32 {
        self.taps
            .iter()
            .map(|t| t.priority)
            .max()
            .map_or(0, |m| m + 1)
    }
}

fn is_valid_tap_id(id: &str) -> bool {
    if id.len() < 2 || id.len() > 64 {
        return false;
    }
    let b = id.as_bytes();
    if !b[0].is_ascii_lowercase() {
        return false;
    }
    let last = b[b.len() - 1];
    if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn sample_toml() -> &'static str {
        r#"schema_version = 1

[[tap]]
id = "reliquaint-core"
source = "https://github.com/syraenix/reliquaint-core"
added_at = 2026-06-01T12:00:00Z
priority = 0
"#
    }

    #[test]
    fn round_trip_parse() {
        let m = SubscriptionManifest::parse_str(sample_toml()).unwrap();
        assert_eq!(m.taps.len(), 1);
        assert_eq!(m.taps[0].id, "reliquaint-core");
        assert_eq!(m.taps[0].source, "https://github.com/syraenix/reliquaint-core");
        assert_eq!(m.taps[0].priority, 0);
    }

    #[test]
    fn empty_manifest_is_valid() {
        let m = SubscriptionManifest::parse_str("schema_version = 1\n").unwrap();
        assert!(m.taps.is_empty());
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let toml = "schema_version = 99\n";
        let err = SubscriptionManifest::parse_str(toml).unwrap_err();
        assert!(
            err.to_string().contains("schema_version"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_duplicate_id() {
        let toml = r#"schema_version = 1
[[tap]]
id = "foo"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 0

[[tap]]
id = "foo"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 1
"#;
        let err = SubscriptionManifest::parse_str(toml).unwrap_err();
        assert!(
            err.to_string().contains("duplicate"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_reserved_id_local() {
        let toml = r#"schema_version = 1
[[tap]]
id = "local"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 0
"#;
        let err = SubscriptionManifest::parse_str(toml).unwrap_err();
        assert!(
            err.to_string().contains("reserved"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_invalid_tap_id() {
        let toml = r#"schema_version = 1
[[tap]]
id = "BAD-ID"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 0
"#;
        let err = SubscriptionManifest::parse_str(toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid tap id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn write_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subscriptions.toml");
        let mut m = SubscriptionManifest::empty();
        m.taps.push(TapSubscription {
            id: "reliquaint-core".into(),
            source: "https://github.com/syraenix/reliquaint-core".into(),
            added_at: toml::value::Datetime::from_str("2026-06-01T12:00:00Z").unwrap(),
            priority: 0,
        });
        m.write(&path).unwrap();
        let reloaded = SubscriptionManifest::load(&path).unwrap();
        assert_eq!(reloaded.taps.len(), 1);
        assert_eq!(reloaded.taps[0].id, "reliquaint-core");
    }

    #[test]
    fn load_or_empty_returns_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let m = SubscriptionManifest::load_or_empty(&path).unwrap();
        assert!(m.taps.is_empty());
    }

    #[test]
    fn next_priority_is_zero_when_empty() {
        let m = SubscriptionManifest::empty();
        assert_eq!(m.next_priority(), 0);
    }

    #[test]
    fn next_priority_increments_max() {
        let toml = r#"schema_version = 1
[[tap]]
id = "aa"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 3

[[tap]]
id = "bb"
source = "https://example.com"
added_at = 2026-01-01T00:00:00Z
priority = 1
"#;
        let m = SubscriptionManifest::parse_str(toml).unwrap();
        assert_eq!(m.next_priority(), 4);
    }

    #[test]
    fn write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/subscriptions.toml");
        SubscriptionManifest::empty().write(&path).unwrap();
        assert!(path.exists());
    }
}
