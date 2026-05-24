//! User launcher config loader. See `docs/schema.md` §"User launcher
//! config".
//!
//! Per-user, per-machine settings that don't belong in any catalog
//! entry: how to invoke each emulator, where the FluidSynth soundfont
//! lives, etc. Lives at `${XDG_CONFIG_HOME}/reliquaint/config.toml`.
//!
//! The defaults are Debian-friendly (Flatpak DOSBox-Staging, apt
//! `fluidsynth` + `fluid-soundfont-gm`). A missing file is fine — the
//! launcher just uses defaults. Partial overrides are supported: a user
//! who only wants to change the soundfont can write a four-line config.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct UserConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub emulators: EmulatorsConfig,
    #[serde(default)]
    pub sidecars: SidecarsConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct EmulatorsConfig {
    #[serde(rename = "dosbox-staging", default)]
    pub dosbox_staging: DosboxStagingConfig,
    #[serde(rename = "fs-uae", default)]
    pub fs_uae: FsUaeConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DosboxStagingConfig {
    /// Invocation prefix. May be a multi-word command (e.g.
    /// `"flatpak run io.github.dosbox-staging"`); the launch composer
    /// splits on whitespace. No shell expansion is performed.
    #[serde(default = "default_dosbox_command")]
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FsUaeConfig {
    #[serde(default = "default_fs_uae_command")]
    pub command: String,
    /// Directory containing the user's Amiga Kickstart ROMs. Optional;
    /// FS-UAE has its own fallbacks.
    #[serde(default)]
    pub kickstart_path: Option<PathBuf>,
    /// Launch FS-UAE fullscreen. Default false (windowed) — the launcher
    /// presents the native Amiga frame in a window, integer-scaled.
    #[serde(default)]
    pub fullscreen: bool,
    /// Integer scale factor for the windowed view. Default 3 (a crisp ~3×
    /// view of the native PAL frame). Ignored when `fullscreen` is set.
    #[serde(default = "default_fs_uae_window_scale")]
    pub window_scale: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct SidecarsConfig {
    #[serde(default)]
    pub fluidsynth: FluidsynthConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FluidsynthConfig {
    #[serde(default = "default_fluidsynth_command")]
    pub command: String,
    #[serde(default = "default_soundfont_path")]
    pub soundfont: PathBuf,
}

// --- Defaults -------------------------------------------------------------

fn default_dosbox_command() -> String {
    "flatpak run io.github.dosbox-staging".to_string()
}

fn default_fs_uae_command() -> String {
    "fs-uae".to_string()
}

fn default_fs_uae_window_scale() -> u32 {
    3
}

fn default_fluidsynth_command() -> String {
    "fluidsynth".to_string()
}

fn default_soundfont_path() -> PathBuf {
    PathBuf::from("/usr/share/sounds/sf2/FluidR3_GM.sf2")
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            emulators: EmulatorsConfig::default(),
            sidecars: SidecarsConfig::default(),
        }
    }
}

impl Default for DosboxStagingConfig {
    fn default() -> Self {
        Self {
            command: default_dosbox_command(),
        }
    }
}

impl Default for FsUaeConfig {
    fn default() -> Self {
        Self {
            command: default_fs_uae_command(),
            kickstart_path: None,
            fullscreen: false,
            window_scale: default_fs_uae_window_scale(),
        }
    }
}

impl Default for FluidsynthConfig {
    fn default() -> Self {
        Self {
            command: default_fluidsynth_command(),
            soundfont: default_soundfont_path(),
        }
    }
}

// --- Errors ---------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read user config {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse user config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error(
        "unsupported schema_version {version} in {path} (expected 1 — see docs/schema.md)"
    )]
    UnsupportedSchema { path: PathBuf, version: u32 },
}

// --- API ------------------------------------------------------------------

/// Read and parse the user config from disk. Returns
/// `UserConfig::default()` if the file does not exist (this is by
/// design — "no config" means "use defaults"). Other IO errors and
/// parse failures return `Err`.
pub fn load(path: &Path) -> Result<UserConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_str(&text, path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(path = %path.display(), "no user config file; using defaults");
            Ok(UserConfig::default())
        }
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Convenience wrapper: log the error and fall back to defaults on any
/// failure. Use this from the CLI / GUI entry points where the user
/// would otherwise never see why their override isn't taking effect —
/// the warning surfaces in the launcher's log output.
pub fn load_or_default(path: &Path) -> UserConfig {
    match load(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "user config failed to load; using defaults");
            UserConfig::default()
        }
    }
}

/// Parse a user config from in-memory TOML text. `path` is used only
/// for error context.
pub fn parse_str(text: &str, path: &Path) -> Result<UserConfig, ConfigError> {
    let config: UserConfig = toml::from_str(text).map_err(|source| {
        let owned = path.to_path_buf();
        tracing::error!(path = %owned.display(), error = %source, "user config parse failed");
        ConfigError::Parse {
            path: owned,
            source,
        }
    })?;
    if config.schema_version != 1 {
        return Err(ConfigError::UnsupportedSchema {
            path: path.to_path_buf(),
            version: config.schema_version,
        });
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_default() {
        let result = load(Path::new("/definitely/not/a/real/config/path.toml")).unwrap();
        assert_eq!(result, UserConfig::default());
    }

    #[test]
    fn empty_config_with_only_schema_version_equals_default() {
        let text = "schema_version = 1\n";
        let parsed = parse_str(text, Path::new("test.toml")).unwrap();
        assert_eq!(parsed, UserConfig::default());
    }

    #[test]
    fn partial_override_only_soundfont_path() {
        // The explicit "Done when" criterion for Task 3.1.
        let text = r#"
schema_version = 1

[sidecars.fluidsynth]
soundfont = "/opt/sounds/custom.sf2"
"#;
        let parsed = parse_str(text, Path::new("test.toml")).unwrap();

        // The overridden field changed:
        assert_eq!(
            parsed.sidecars.fluidsynth.soundfont,
            PathBuf::from("/opt/sounds/custom.sf2")
        );

        // Every other field remained at defaults:
        assert_eq!(parsed.sidecars.fluidsynth.command, "fluidsynth");
        assert_eq!(
            parsed.emulators.dosbox_staging.command,
            "flatpak run io.github.dosbox-staging"
        );
        assert_eq!(parsed.emulators.fs_uae.command, "fs-uae");
        assert_eq!(parsed.emulators.fs_uae.kickstart_path, None);
        assert!(!parsed.emulators.fs_uae.fullscreen);
        assert_eq!(parsed.emulators.fs_uae.window_scale, 3);
    }

    #[test]
    fn full_override_across_all_sections() {
        let text = r#"
schema_version = 1

[emulators.dosbox-staging]
command = "/usr/bin/dosbox-staging"

[emulators.fs-uae]
command = "/opt/fs-uae/bin/fs-uae"
kickstart_path = "/home/me/kickstarts"

[sidecars.fluidsynth]
command = "/usr/local/bin/fluidsynth"
soundfont = "/opt/sounds/custom.sf2"
"#;
        let parsed = parse_str(text, Path::new("test.toml")).unwrap();

        assert_eq!(parsed.emulators.dosbox_staging.command, "/usr/bin/dosbox-staging");
        assert_eq!(parsed.emulators.fs_uae.command, "/opt/fs-uae/bin/fs-uae");
        assert_eq!(
            parsed.emulators.fs_uae.kickstart_path,
            Some(PathBuf::from("/home/me/kickstarts"))
        );
        assert_eq!(parsed.sidecars.fluidsynth.command, "/usr/local/bin/fluidsynth");
        assert_eq!(
            parsed.sidecars.fluidsynth.soundfont,
            PathBuf::from("/opt/sounds/custom.sf2")
        );
    }

    #[test]
    fn round_trips_default_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let original = UserConfig::default();
        std::fs::write(&path, toml::to_string(&original).unwrap()).unwrap();
        let read_back = load(&path).unwrap();
        assert_eq!(original, read_back);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let text = "schema_version = 2\n";
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedSchema { version: 2, .. }));
    }

    #[test]
    fn rejects_malformed_toml() {
        let text = "schema_version = 1\nthis is not valid toml [[[";
        let err = parse_str(text, Path::new("test.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn load_or_default_falls_back_on_parse_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "not valid [[[").unwrap();
        let result = load_or_default(&path);
        assert_eq!(result, UserConfig::default());
    }
}
