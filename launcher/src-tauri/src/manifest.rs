use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Dos,
    Amiga,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub emulator: String,
    pub config: Option<String>,
    #[serde(default)]
    pub sidecars: Vec<String>,
    pub file: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub title: String,
    pub platform: Platform,
    pub collection: String,
    pub runtime: Runtime,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("TOML parse error in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("DOS manifest missing runtime.config")]
    DosConfigMissing,
    #[error("Amiga manifest must not declare sidecars")]
    AmigaSidecarsForbidden,
}

pub fn parse_str(src: &str) -> Result<Manifest, ManifestError> {
    let m: Manifest = toml::from_str(src).map_err(|e| ManifestError::Parse {
        path: PathBuf::from("<string>"),
        source: e,
    })?;
    validate(&m)?;
    Ok(m)
}

pub fn parse_file(path: &Path) -> Result<Manifest, ManifestError> {
    let src = std::fs::read_to_string(path).map_err(|e| ManifestError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let m: Manifest = toml::from_str(&src).map_err(|e| ManifestError::Parse {
        path: path.to_path_buf(),
        source: e,
    })?;
    validate(&m)?;
    Ok(m)
}

fn validate(m: &Manifest) -> Result<(), ManifestError> {
    match m.platform {
        Platform::Dos => {
            if m.runtime.config.is_none() {
                return Err(ManifestError::DosConfigMissing);
            }
        }
        Platform::Amiga => {
            if !m.runtime.sidecars.is_empty() {
                return Err(ManifestError::AmigaSidecarsForbidden);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOS_MANIFEST: &str = r#"
id         = "qfg1-ega"
title      = "Quest for Glory 1 (EGA)"
platform   = "dos"
collection = "quest-for-glory"

[runtime]
emulator = "dosbox-staging"
config   = "../config/qfg1-ega.conf"
sidecars = ["fluidsynth"]
"#;

    const AMIGA_MANIFEST: &str = r#"
id         = "lemmings"
title      = "Lemmings"
platform   = "amiga"
collection = "amiga-classics"

[runtime]
emulator = "fs-uae"
file     = "../games/lemmings.adf"
model    = "a500"
"#;

    #[test]
    fn dos_manifest_parses() {
        let m = parse_str(DOS_MANIFEST).unwrap();
        assert_eq!(m.id, "qfg1-ega");
        assert_eq!(m.platform, Platform::Dos);
        assert_eq!(m.runtime.config.as_deref(), Some("../config/qfg1-ega.conf"));
        assert_eq!(m.runtime.sidecars, vec!["fluidsynth"]);
    }

    #[test]
    fn amiga_manifest_parses() {
        let m = parse_str(AMIGA_MANIFEST).unwrap();
        assert_eq!(m.id, "lemmings");
        assert_eq!(m.platform, Platform::Amiga);
        assert_eq!(m.runtime.file.as_deref(), Some("../games/lemmings.adf"));
        assert_eq!(m.runtime.model.as_deref(), Some("a500"));
        assert!(m.runtime.sidecars.is_empty());
    }

    #[test]
    fn sidecars_defaults_to_empty() {
        let src = r#"
id = "kq1sci"
title = "King's Quest 1"
platform = "dos"
collection = "kings-quest"
[runtime]
emulator = "dosbox-staging"
config = "../config/kq1sci.conf"
"#;
        let m = parse_str(src).unwrap();
        assert!(m.runtime.sidecars.is_empty());
    }

    #[test]
    fn dos_missing_config_errors() {
        let src = r#"
id = "x"
title = "X"
platform = "dos"
collection = "c"
[runtime]
emulator = "dosbox-staging"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::DosConfigMissing)));
    }

    #[test]
    fn dos_without_install_section_is_valid() {
        let src = r#"
id = "x"
title = "X"
platform = "dos"
collection = "c"
[runtime]
emulator = "dosbox-staging"
config = "../config/x.conf"
"#;
        assert!(parse_str(src).is_ok());
    }

    #[test]
    fn amiga_without_file_field_is_valid() {
        let src = r#"
id = "x"
title = "X"
platform = "amiga"
collection = "c"
[runtime]
emulator = "fs-uae"
model = "a500"
"#;
        assert!(parse_str(src).is_ok());
    }

    #[test]
    fn amiga_sidecars_forbidden() {
        let src = r#"
id = "x"
title = "X"
platform = "amiga"
collection = "c"
[runtime]
emulator = "fs-uae"
file = "../games/x.adf"
sidecars = ["fluidsynth"]
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::AmigaSidecarsForbidden)));
    }

    #[test]
    fn unknown_field_errors() {
        let src = r#"
id = "x"
title = "X"
platform = "dos"
collection = "c"
bogus_field = "oops"
[runtime]
emulator = "dosbox-staging"
config = "../config/x.conf"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::Parse { .. })));
    }
}
