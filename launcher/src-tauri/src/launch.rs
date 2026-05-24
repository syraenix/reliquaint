//! Launch command composition.
//!
//! Takes a `(CatalogEntry, InstallRecord, UserConfig)` triple and
//! produces a [`LaunchPlan`] — the program + args the launcher would
//! spawn, plus any sidecars to run alongside. Pure data; no processes
//! are spawned here. The actual spawn-and-supervise lifecycle lives in
//! [`crate::sidecar`].
//!
//! - DOS games: see ADR-0002 for the split-config rationale. We compose
//!   the [autoexec] section as `-c` flags rather than writing a merged
//!   tempfile.

use std::path::{Path, PathBuf};

use crate::catalog::{CatalogEntry, Platform};
use crate::install_record::InstallRecord;
use crate::user_config::{SidecarsConfig, UserConfig};

/// A fully-resolved program invocation. Plain struct rather than
/// [`std::process::Command`] so it can be `Clone`/`PartialEq`/`Debug` for
/// testing and dry-run printing. Converted to a real `Command` at spawn
/// time via [`PreparedCommand::to_command`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
}

impl PreparedCommand {
    pub fn to_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.program);
        cmd.args(&self.args);
        if let Some(wd) = &self.working_dir {
            cmd.current_dir(wd);
        }
        cmd
    }

    /// Single-line human-readable rendering for dry-run output and
    /// diagnostics. Quotes args that contain whitespace so the line is
    /// re-runnable in a shell.
    pub fn display_line(&self) -> String {
        let mut out = quote_if_needed(&self.program);
        for a in &self.args {
            out.push(' ');
            out.push_str(&quote_if_needed(a));
        }
        out
    }
}

fn quote_if_needed(s: &str) -> String {
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || c == '"') {
        let escaped = s.replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarSpec {
    pub name: String,
    pub command: PreparedCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub primary: PreparedCommand,
    pub sidecars: Vec<SidecarSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("DOS entry {entry_id:?} is missing [runtime.dosbox]")]
    MissingDosboxRuntime { entry_id: String },

    #[error("could not resolve sibling shipped config relative to {path}")]
    InvalidEntrySource { path: PathBuf },

    #[error("shipped emulator config not found: {path}")]
    ShippedConfigNotFound { path: PathBuf },

    #[error(
        "emulator command is empty in user config; set [emulators.<emulator>].command \
         (see docs/schema.md §\"User launcher config\")"
    )]
    EmptyEmulatorCommand,

    #[error("unknown sidecar {name:?} (v0.1 supports: fluidsynth)")]
    UnknownSidecar { name: String },
}

/// Compose the launch command for a DOS game. Caller must ensure
/// `entry.game.platform == Platform::Dos`; this is enforced as a
/// defensive check that returns [`LaunchError::MissingDosboxRuntime`]
/// when violated.
pub fn compose_dosbox(
    entry: &CatalogEntry,
    entry_source: &Path,
    install: &InstallRecord,
    user_config: &UserConfig,
) -> Result<LaunchPlan, LaunchError> {
    debug_assert_eq!(entry.game.platform, Platform::Dos);

    let dosbox = entry.runtime.dosbox.as_ref().ok_or_else(|| {
        LaunchError::MissingDosboxRuntime {
            entry_id: entry.game.id.clone(),
        }
    })?;

    let conf_path = entry_source
        .parent()
        .map(|p| p.join(&dosbox.config))
        .ok_or_else(|| LaunchError::InvalidEntrySource {
            path: entry_source.to_path_buf(),
        })?;
    if !conf_path.is_file() {
        return Err(LaunchError::ShippedConfigNotFound { path: conf_path });
    }

    let (program, mut args) = split_command(&user_config.emulators.dosbox_staging.command)?;

    args.push("-conf".into());
    args.push(conf_path.to_string_lossy().into_owned());

    // Generated [autoexec] section, expressed as -c flags. Per ADR-0002,
    // the shipped .conf never carries an [autoexec] block; this is where
    // the install path joins the per-game config.
    let install_path = install.install.install_path.to_string_lossy().into_owned();
    let mount = &dosbox.mount;
    args.push("-c".into());
    args.push(format!(r#"MOUNT {mount} "{install_path}""#));
    args.push("-c".into());
    args.push(format!("{mount}:"));
    args.push("-c".into());
    args.push(dosbox.entry.clone());
    args.push("-c".into());
    args.push("EXIT".into());

    let primary = PreparedCommand {
        program,
        args,
        working_dir: None,
    };
    let sidecars = compose_sidecars(&entry.runtime.sidecars, &user_config.sidecars)?;

    Ok(LaunchPlan { primary, sidecars })
}

fn compose_sidecars(
    declared: &[String],
    config: &SidecarsConfig,
) -> Result<Vec<SidecarSpec>, LaunchError> {
    let mut out = Vec::with_capacity(declared.len());
    for name in declared {
        let spec = match name.as_str() {
            "fluidsynth" => {
                let (program, mut args) = split_command(&config.fluidsynth.command)?;
                args.push("-i".into());
                args.push(config.fluidsynth.soundfont.to_string_lossy().into_owned());
                SidecarSpec {
                    name: name.clone(),
                    command: PreparedCommand {
                        program,
                        args,
                        working_dir: None,
                    },
                }
            }
            other => {
                return Err(LaunchError::UnknownSidecar {
                    name: other.to_string(),
                })
            }
        };
        out.push(spec);
    }
    Ok(out)
}

fn split_command(command: &str) -> Result<(String, Vec<String>), LaunchError> {
    let mut tokens = command.split_whitespace();
    let program = tokens
        .next()
        .ok_or(LaunchError::EmptyEmulatorCommand)?
        .to_string();
    let args = tokens.map(String::from).collect();
    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_record::{Install as InstallRec, InstallRecord};
    use std::str::FromStr;

    fn fixture_catalog_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog")
    }

    fn synthetic_install(catalog_id: &str, install_path: &str) -> InstallRecord {
        InstallRecord {
            schema_version: 1,
            install: InstallRec {
                catalog_id: catalog_id.into(),
                tap: "reliquaint-core".into(),
                install_path: PathBuf::from(install_path),
                installed_at: toml::value::Datetime::from_str("2026-05-23T14:32:00Z").unwrap(),
            },
        }
    }

    /// Stand up a temp tap layout so the shipped-config file exists at
    /// the location compose_dosbox expects (sibling of the .toml).
    fn temp_tap_with_qfg1_ega() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let catalog_dos = tmp.path().join("catalog/dos");
        std::fs::create_dir_all(&catalog_dos).unwrap();
        let toml_src = std::fs::read_to_string(fixture_catalog_dir().join("dos/qfg1-ega.toml"))
            .unwrap();
        let toml_dst = catalog_dos.join("qfg1-ega.toml");
        std::fs::write(&toml_dst, toml_src).unwrap();
        // Sibling shipped .conf — content irrelevant to composition,
        // just has to exist.
        std::fs::write(catalog_dos.join("qfg1-ega.conf"), "[sdl]\nfullscreen=false\n").unwrap();
        (tmp, toml_dst)
    }

    #[test]
    fn compose_dosbox_happy_path_against_qfg1_ega() {
        let (_tmp, source_path) = temp_tap_with_qfg1_ega();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("qfg1-ega", "/home/test/games/qfg1-ega");
        let user = UserConfig::default();

        let plan = compose_dosbox(&entry, &source_path, &install, &user).unwrap();

        assert_eq!(plan.primary.program, "flatpak");
        let expected_conf = source_path.parent().unwrap().join("qfg1-ega.conf");
        let expected_args = vec![
            "run".to_string(),
            "io.github.dosbox-staging".to_string(),
            "-conf".to_string(),
            expected_conf.to_string_lossy().into_owned(),
            "-c".to_string(),
            r#"MOUNT c "/home/test/games/qfg1-ega""#.to_string(),
            "-c".to_string(),
            "c:".to_string(),
            "-c".to_string(),
            "SIERRA.BAT".to_string(),
            "-c".to_string(),
            "EXIT".to_string(),
        ];
        assert_eq!(plan.primary.args, expected_args);
    }

    #[test]
    fn compose_dosbox_populates_fluidsynth_sidecar() {
        let (_tmp, source_path) = temp_tap_with_qfg1_ega();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("qfg1-ega", "/games/qfg1-ega");
        let user = UserConfig::default();

        let plan = compose_dosbox(&entry, &source_path, &install, &user).unwrap();

        assert_eq!(plan.sidecars.len(), 1);
        let fs = &plan.sidecars[0];
        assert_eq!(fs.name, "fluidsynth");
        assert_eq!(fs.command.program, "fluidsynth");
        assert_eq!(
            fs.command.args,
            vec![
                "-i".to_string(),
                "/usr/share/sounds/sf2/FluidR3_GM.sf2".to_string(),
            ]
        );
    }

    #[test]
    fn compose_dosbox_errors_on_empty_emulator_command() {
        let (_tmp, source_path) = temp_tap_with_qfg1_ega();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("qfg1-ega", "/games/qfg1-ega");
        let mut user = UserConfig::default();
        user.emulators.dosbox_staging.command = "   ".into();

        let err = compose_dosbox(&entry, &source_path, &install, &user).unwrap_err();
        assert!(matches!(err, LaunchError::EmptyEmulatorCommand));
    }

    #[test]
    fn compose_dosbox_errors_when_shipped_conf_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let catalog_dos = tmp.path().join("catalog/dos");
        std::fs::create_dir_all(&catalog_dos).unwrap();
        // Write the .toml but NOT the .conf.
        let toml_src = std::fs::read_to_string(fixture_catalog_dir().join("dos/qfg1-ega.toml"))
            .unwrap();
        let source_path = catalog_dos.join("qfg1-ega.toml");
        std::fs::write(&source_path, toml_src).unwrap();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("qfg1-ega", "/games/qfg1-ega");

        let err = compose_dosbox(&entry, &source_path, &install, &UserConfig::default())
            .unwrap_err();
        assert!(matches!(err, LaunchError::ShippedConfigNotFound { .. }));
    }

    #[test]
    fn display_line_quotes_args_containing_whitespace() {
        let cmd = PreparedCommand {
            program: "flatpak".into(),
            args: vec![
                "run".into(),
                "io.github.dosbox-staging".into(),
                r#"MOUNT C "/home/me/games/qfg1""#.into(),
            ],
            working_dir: None,
        };
        let line = cmd.display_line();
        assert!(line.contains(r#""MOUNT C \"/home/me/games/qfg1\"""#), "got {line}");
    }
}
