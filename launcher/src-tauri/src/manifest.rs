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
pub struct Install {
    pub expects_dir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    pub artwork: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub title: String,
    pub platform: Platform,
    pub collection: String,
    pub runtime: Runtime,
    pub install: Option<Install>,
    pub ui: Option<Ui>,
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
    #[error("DOS manifest missing [install].expects_dir")]
    DosInstallMissing,
    #[error("Amiga manifest missing runtime.file")]
    AmigaFileMissing,
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
            if m.install.is_none() {
                return Err(ManifestError::DosInstallMissing);
            }
        }
        Platform::Amiga => {
            if m.runtime.file.is_none() {
                return Err(ManifestError::AmigaFileMissing);
            }
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

[install]
expects_dir = "~/games/qfg1-ega"
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
        assert_eq!(m.install.unwrap().expects_dir, "~/games/qfg1-ega");
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
[install]
expects_dir = "~/games/kq1sci"
"#;
        let m = parse_str(src).unwrap();
        assert!(m.runtime.sidecars.is_empty());
    }

    #[test]
    fn ui_section_is_optional() {
        let m = parse_str(DOS_MANIFEST).unwrap();
        assert!(m.ui.is_none());

        let with_ui = DOS_MANIFEST.to_string() + "\n[ui]\nartwork = \"../img/cover.png\"\n";
        let m2 = parse_str(&with_ui).unwrap();
        assert_eq!(m2.ui.unwrap().artwork.as_deref(), Some("../img/cover.png"));
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
[install]
expects_dir = "~/games/x"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::DosConfigMissing)));
    }

    #[test]
    fn dos_missing_install_errors() {
        let src = r#"
id = "x"
title = "X"
platform = "dos"
collection = "c"
[runtime]
emulator = "dosbox-staging"
config = "../config/x.conf"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::DosInstallMissing)));
    }

    #[test]
    fn amiga_missing_file_errors() {
        let src = r#"
id = "x"
title = "X"
platform = "amiga"
collection = "c"
[runtime]
emulator = "fs-uae"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::AmigaFileMissing)));
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
[install]
expects_dir = "~/games/x"
"#;
        assert!(matches!(parse_str(src), Err(ManifestError::Parse { .. })));
    }
}
