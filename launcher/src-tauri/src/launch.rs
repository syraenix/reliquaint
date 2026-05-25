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
//! - Amiga games: shipped FS-UAE config if present, otherwise an inline
//!   `--amiga_model=...` based on `runtime.fs_uae.model`.

use std::path::{Path, PathBuf};

use crate::catalog::{AmigaModel, CatalogEntry, Platform};
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

    #[error("Amiga entry {entry_id:?} is missing [runtime.fs_uae]")]
    MissingFsUaeRuntime { entry_id: String },

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

    let dosbox =
        entry
            .runtime
            .dosbox
            .as_ref()
            .ok_or_else(|| LaunchError::MissingDosboxRuntime {
                entry_id: entry.game.id.clone(),
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

/// Compose the launch command for an Amiga game. Caller must ensure
/// `entry.game.platform == Platform::Amiga`; violation returns
/// [`LaunchError::MissingFsUaeRuntime`].
pub fn compose_fs_uae(
    entry: &CatalogEntry,
    entry_source: &Path,
    install: &InstallRecord,
    user_config: &UserConfig,
) -> Result<LaunchPlan, LaunchError> {
    debug_assert_eq!(entry.game.platform, Platform::Amiga);

    let fs_uae = entry
        .runtime
        .fs_uae
        .as_ref()
        .ok_or_else(|| LaunchError::MissingFsUaeRuntime {
            entry_id: entry.game.id.clone(),
        })?;

    let (program, mut args) = split_command(&user_config.emulators.fs_uae.command)?;

    let install_path = &install.install.install_path;

    // Explicitly declared disks, resolved relative to install_path.
    let mut floppies: Vec<PathBuf> = fs_uae
        .floppies
        .iter()
        .map(|f| install_path.join(f))
        .collect();
    let mut hard_drives: Vec<PathBuf> = fs_uae
        .hard_drives
        .iter()
        .map(|h| install_path.join(h))
        .collect();

    // Positional config (shipped sibling `.fs-uae`) OR inline model
    // template. Per the schema doc, if `config` is set we use the sibling
    // file; otherwise we drive the model directly.
    let mut used_config = false;
    if let Some(config_name) = fs_uae.config.as_deref() {
        let config_path = entry_source
            .parent()
            .map(|p| p.join(config_name))
            .ok_or_else(|| LaunchError::InvalidEntrySource {
                path: entry_source.to_path_buf(),
            })?;
        if !config_path.is_file() {
            return Err(LaunchError::ShippedConfigNotFound { path: config_path });
        }
        args.push(config_path.to_string_lossy().into_owned());
        used_config = true;
    }

    // Autodetect fallback: when the entry declares no config and no disks
    // (e.g. the game was installed by unzipping an .rp9, or from a single
    // bare image), discover an inner `.fs-uae` config, else `.hdf`, else
    // `.adf`, inside the permanent install_path. No temp dir is needed —
    // the files live there for good — so the plan stays a pure value.
    if !used_config && floppies.is_empty() && hard_drives.is_empty() {
        match autodetect_amiga_source(install_path) {
            Some(AmigaSource::Config(p)) => {
                args.push(p.to_string_lossy().into_owned());
                used_config = true;
            }
            Some(AmigaSource::HardDrive(p)) => hard_drives.push(p),
            Some(AmigaSource::Floppy(p)) => floppies.push(p),
            None => {}
        }
    }

    if !used_config {
        args.push(format!(
            "--amiga_model={}",
            fs_uae_model_string(fs_uae.model)
        ));
    }

    // Historic A500: one internal drive (DF0). Disk 1 boots in DF0; every
    // disk is registered in the swap list so the user cycles them via the
    // FS-UAE disk menu when the game prompts. Mounting all disks in separate
    // drives is not authentic and confuses real games' loaders.
    if let Some(first) = floppies.first() {
        args.push("--floppy_drive_count=1".into());
        args.push(format!("--floppy_drive_0={}", first.to_string_lossy()));
        for (i, floppy) in floppies.iter().enumerate() {
            args.push(format!("--floppy_image_{i}={}", floppy.to_string_lossy()));
        }
    }
    for (i, hd) in hard_drives.iter().enumerate() {
        args.push(format!("--hard_drive_{i}={}", hd.to_string_lossy()));
    }

    if let Some(kickstart_dir) = &user_config.emulators.fs_uae.kickstart_path {
        args.push(format!(
            "--kickstarts_dir={}",
            kickstart_dir.to_string_lossy()
        ));
    }

    // Display: windowed and integer-scaled by default (a crisp native A500
    // view), or fullscreen if the user opts in. Command-line flags override
    // any shipped `.fs-uae` config, so this host preference always wins.
    if user_config.emulators.fs_uae.fullscreen {
        args.push("--fullscreen=1".into());
    } else {
        args.push("--fullscreen=0".into());
        // FS-UAE doesn't derive width from height, so set both — an integer
        // multiple of its 640×480 (4:3) output frame, so the window keeps the
        // correct Amiga proportions instead of going tall and narrow.
        let scale = user_config.emulators.fs_uae.window_scale.max(1);
        args.push(format!("--window_width={}", 640 * scale));
        args.push(format!("--window_height={}", 480 * scale));
    }

    let primary = PreparedCommand {
        program,
        args,
        working_dir: None,
    };
    let sidecars = compose_sidecars(&entry.runtime.sidecars, &user_config.sidecars)?;

    Ok(LaunchPlan { primary, sidecars })
}

/// An Amiga disk source discovered inside an install directory.
enum AmigaSource {
    Config(PathBuf),
    HardDrive(PathBuf),
    Floppy(PathBuf),
}

/// Scan `install_path` for a runnable Amiga source when the catalog entry
/// declares none. Preference order matches the old launcher: a `.fs-uae`
/// config wins, then a `.hdf` hard disk, then a `.adf` floppy. Recurses so
/// an unzipped `.rp9` with nested contents still resolves.
fn autodetect_amiga_source(install_path: &Path) -> Option<AmigaSource> {
    if let Some(p) = find_first_by_ext(install_path, "fs-uae") {
        return Some(AmigaSource::Config(p));
    }
    if let Some(p) = find_first_by_ext(install_path, "hdf") {
        return Some(AmigaSource::HardDrive(p));
    }
    if let Some(p) = find_first_by_ext(install_path, "adf") {
        return Some(AmigaSource::Floppy(p));
    }
    None
}

/// First file under `dir` (depth-first) whose extension matches `ext`
/// case-insensitively. Files at the current level take precedence over
/// those in subdirectories.
fn find_first_by_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    subdirs.sort();
    for sub in subdirs {
        if let Some(found) = find_first_by_ext(&sub, ext) {
            return Some(found);
        }
    }
    None
}

fn fs_uae_model_string(model: AmigaModel) -> &'static str {
    match model {
        AmigaModel::A500 => "A500",
        AmigaModel::A600 => "A600",
        AmigaModel::A1200 => "A1200",
        AmigaModel::A4000 => "A4000",
    }
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
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tap/catalog")
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
        let toml_src =
            std::fs::read_to_string(fixture_catalog_dir().join("dos/qfg1-ega.toml")).unwrap();
        let toml_dst = catalog_dos.join("qfg1-ega.toml");
        std::fs::write(&toml_dst, toml_src).unwrap();
        // Sibling shipped .conf — content irrelevant to composition,
        // just has to exist.
        std::fs::write(
            catalog_dos.join("qfg1-ega.conf"),
            "[sdl]\nfullscreen=false\n",
        )
        .unwrap();
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
        let toml_src =
            std::fs::read_to_string(fixture_catalog_dir().join("dos/qfg1-ega.toml")).unwrap();
        let source_path = catalog_dos.join("qfg1-ega.toml");
        std::fs::write(&source_path, toml_src).unwrap();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("qfg1-ega", "/games/qfg1-ega");

        let err =
            compose_dosbox(&entry, &source_path, &install, &UserConfig::default()).unwrap_err();
        assert!(matches!(err, LaunchError::ShippedConfigNotFound { .. }));
    }

    fn temp_tap_with_fatman() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let catalog_amiga = tmp.path().join("catalog/amiga");
        std::fs::create_dir_all(&catalog_amiga).unwrap();
        let toml_src =
            std::fs::read_to_string(fixture_catalog_dir().join("amiga/fatman.toml")).unwrap();
        let source_path = catalog_amiga.join("fatman.toml");
        std::fs::write(&source_path, toml_src).unwrap();
        (tmp, source_path)
    }

    #[test]
    fn compose_fs_uae_happy_path_against_fatman_model_template() {
        let (_tmp, source_path) = temp_tap_with_fatman();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("fatman", "/home/test/games/fatman");

        let plan = compose_fs_uae(&entry, &source_path, &install, &UserConfig::default()).unwrap();

        assert_eq!(plan.primary.program, "fs-uae");
        assert_eq!(
            plan.primary.args,
            vec![
                "--amiga_model=A500".to_string(),
                "--floppy_drive_count=1".to_string(),
                "--floppy_drive_0=/home/test/games/fatman/fatman.adf".to_string(),
                "--floppy_image_0=/home/test/games/fatman/fatman.adf".to_string(),
                "--fullscreen=0".to_string(),
                "--window_width=1920".to_string(),
                "--window_height=1440".to_string(),
            ]
        );
        assert!(plan.sidecars.is_empty());
    }

    #[test]
    fn compose_fs_uae_uses_shipped_config_when_present() {
        use crate::catalog::{
            Acquisition, CatalogEntry, Emulator, FsUaeRuntime, Game, Install as CatInstall, Meta,
            Runtime,
        };

        let tmp = tempfile::tempdir().unwrap();
        let catalog_amiga = tmp.path().join("catalog/amiga");
        std::fs::create_dir_all(&catalog_amiga).unwrap();
        let shipped = catalog_amiga.join("fatman.fs-uae");
        std::fs::write(&shipped, "amiga_model = A500\n").unwrap();
        let source_path = catalog_amiga.join("fatman.toml");
        std::fs::write(&source_path, "# placeholder").unwrap();

        let entry = CatalogEntry {
            schema_version: 1,
            game: Game {
                id: "fatman".into(),
                title: "Fatman".into(),
                platform: Platform::Amiga,
                collection: None,
            },
            meta: Meta::default(),
            acquisition: Acquisition::default(),
            install: CatInstall::default(),
            runtime: Runtime {
                emulator: Emulator::FsUae,
                sidecars: vec![],
                dosbox: None,
                fs_uae: Some(FsUaeRuntime {
                    model: AmigaModel::A500,
                    config: Some("fatman.fs-uae".into()),
                    floppies: vec!["fatman.adf".into()],
                    hard_drives: vec![],
                }),
            },
        };
        let install = synthetic_install("fatman", "/games/fatman");

        let plan = compose_fs_uae(&entry, &source_path, &install, &UserConfig::default()).unwrap();

        // First positional arg is the shipped config; no --amiga_model.
        assert_eq!(plan.primary.args[0], shipped.to_string_lossy());
        assert!(!plan
            .primary
            .args
            .iter()
            .any(|a| a.starts_with("--amiga_model=")));
        assert!(plan
            .primary
            .args
            .iter()
            .any(|a| a == "--floppy_drive_0=/games/fatman/fatman.adf"));
    }

    #[test]
    fn compose_fs_uae_handles_multiple_floppies_in_order() {
        use crate::catalog::{
            Acquisition, CatalogEntry, Emulator, FsUaeRuntime, Game, Install as CatInstall, Meta,
            Runtime,
        };
        let entry = CatalogEntry {
            schema_version: 1,
            game: Game {
                id: "multi".into(),
                title: "Multi".into(),
                platform: Platform::Amiga,
                collection: None,
            },
            meta: Meta::default(),
            acquisition: Acquisition::default(),
            install: CatInstall::default(),
            runtime: Runtime {
                emulator: Emulator::FsUae,
                sidecars: vec![],
                dosbox: None,
                fs_uae: Some(FsUaeRuntime {
                    model: AmigaModel::A1200,
                    config: None,
                    floppies: vec!["disk1.adf".into(), "disk2.adf".into(), "disk3.adf".into()],
                    hard_drives: vec![],
                }),
            },
        };
        let install = synthetic_install("multi", "/games/multi");

        let plan = compose_fs_uae(
            &entry,
            Path::new("/tap/catalog/amiga/multi.toml"),
            &install,
            &UserConfig::default(),
        )
        .unwrap();

        assert!(plan
            .primary
            .args
            .contains(&"--amiga_model=A1200".to_string()));
        // Single internal drive boots disk 1; all disks form the swap list.
        assert!(plan
            .primary
            .args
            .contains(&"--floppy_drive_count=1".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--floppy_drive_0=/games/multi/disk1.adf".to_string()));
        assert!(!plan
            .primary
            .args
            .iter()
            .any(|a| a.starts_with("--floppy_drive_1") || a.starts_with("--floppy_drive_2")));
        assert!(plan
            .primary
            .args
            .contains(&"--floppy_image_0=/games/multi/disk1.adf".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--floppy_image_1=/games/multi/disk2.adf".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--floppy_image_2=/games/multi/disk3.adf".to_string()));
    }

    fn amiga_entry_no_disks(model: AmigaModel) -> crate::catalog::CatalogEntry {
        use crate::catalog::{
            Acquisition, CatalogEntry, Emulator, FsUaeRuntime, Game, Install as CatInstall, Meta,
            Runtime,
        };
        CatalogEntry {
            schema_version: 1,
            game: Game {
                id: "g".into(),
                title: "G".into(),
                platform: Platform::Amiga,
                collection: None,
            },
            meta: Meta::default(),
            acquisition: Acquisition::default(),
            install: CatInstall::default(),
            runtime: Runtime {
                emulator: Emulator::FsUae,
                sidecars: vec![],
                dosbox: None,
                fs_uae: Some(FsUaeRuntime {
                    model,
                    config: None,
                    floppies: vec![],
                    hard_drives: vec![],
                }),
            },
        }
    }

    #[test]
    fn compose_fs_uae_maps_hard_drives_in_order() {
        let mut entry = amiga_entry_no_disks(AmigaModel::A1200);
        entry.runtime.fs_uae.as_mut().unwrap().hard_drives =
            vec!["sys.hdf".into(), "work.hdf".into()];
        let install = synthetic_install("wb", "/games/wb");

        let plan = compose_fs_uae(
            &entry,
            Path::new("/tap/catalog/amiga/wb.toml"),
            &install,
            &UserConfig::default(),
        )
        .unwrap();

        assert!(plan
            .primary
            .args
            .contains(&"--amiga_model=A1200".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--hard_drive_0=/games/wb/sys.hdf".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--hard_drive_1=/games/wb/work.hdf".to_string()));
    }

    #[test]
    fn compose_fs_uae_autodetects_adf_when_nothing_declared() {
        let install_dir = tempfile::tempdir().unwrap();
        std::fs::write(install_dir.path().join("game.adf"), b"DOS").unwrap();
        let entry = amiga_entry_no_disks(AmigaModel::A500);
        let install = synthetic_install("g", install_dir.path().to_str().unwrap());

        let plan = compose_fs_uae(
            &entry,
            Path::new("/tap/catalog/amiga/g.toml"),
            &install,
            &UserConfig::default(),
        )
        .unwrap();

        let expected_drive = format!(
            "--floppy_drive_0={}/game.adf",
            install_dir.path().to_string_lossy()
        );
        let expected_image = format!(
            "--floppy_image_0={}/game.adf",
            install_dir.path().to_string_lossy()
        );
        assert!(
            plan.primary.args.contains(&expected_drive),
            "expected {expected_drive} in {:?}",
            plan.primary.args
        );
        assert!(
            plan.primary.args.contains(&expected_image),
            "expected {expected_image} in {:?}",
            plan.primary.args
        );
    }

    #[test]
    fn compose_fs_uae_autodetects_hdf_when_nothing_declared() {
        let install_dir = tempfile::tempdir().unwrap();
        std::fs::write(install_dir.path().join("system.hdf"), b"RDSK").unwrap();
        let entry = amiga_entry_no_disks(AmigaModel::A1200);
        let install = synthetic_install("g", install_dir.path().to_str().unwrap());

        let plan = compose_fs_uae(
            &entry,
            Path::new("/tap/catalog/amiga/g.toml"),
            &install,
            &UserConfig::default(),
        )
        .unwrap();

        let expected = format!(
            "--hard_drive_0={}/system.hdf",
            install_dir.path().to_string_lossy()
        );
        assert!(
            plan.primary.args.contains(&expected),
            "expected {expected} in {:?}",
            plan.primary.args
        );
    }

    #[test]
    fn compose_fs_uae_autodetected_config_replaces_amiga_model() {
        let install_dir = tempfile::tempdir().unwrap();
        let cfg = install_dir.path().join("game.fs-uae");
        std::fs::write(&cfg, "amiga_model = A1200\n").unwrap();
        let entry = amiga_entry_no_disks(AmigaModel::A1200);
        let install = synthetic_install("g", install_dir.path().to_str().unwrap());

        let plan = compose_fs_uae(
            &entry,
            Path::new("/tap/catalog/amiga/g.toml"),
            &install,
            &UserConfig::default(),
        )
        .unwrap();

        assert_eq!(plan.primary.args[0], cfg.to_string_lossy());
        assert!(
            !plan
                .primary
                .args
                .iter()
                .any(|a| a.starts_with("--amiga_model=")),
            "autodetected .fs-uae config should replace --amiga_model: {:?}",
            plan.primary.args
        );
    }

    #[test]
    fn compose_fs_uae_includes_kickstart_dir_when_set() {
        let (_tmp, source_path) = temp_tap_with_fatman();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("fatman", "/games/fatman");
        let mut user = UserConfig::default();
        user.emulators.fs_uae.kickstart_path = Some(PathBuf::from("/opt/kickstarts"));

        let plan = compose_fs_uae(&entry, &source_path, &install, &user).unwrap();

        assert!(plan
            .primary
            .args
            .contains(&"--kickstarts_dir=/opt/kickstarts".to_string()));
    }

    #[test]
    fn compose_fs_uae_windowed_integer_scaled_by_default() {
        let (_tmp, source_path) = temp_tap_with_fatman();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("fatman", "/games/fatman");

        let plan = compose_fs_uae(&entry, &source_path, &install, &UserConfig::default()).unwrap();

        // Default scale is 3 → 640×480 × 3 = 1920×1440, windowed.
        assert!(plan.primary.args.contains(&"--fullscreen=0".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--window_width=1920".to_string()));
        assert!(plan
            .primary
            .args
            .contains(&"--window_height=1440".to_string()));
    }

    #[test]
    fn compose_fs_uae_fullscreen_when_user_opts_in() {
        let (_tmp, source_path) = temp_tap_with_fatman();
        let entry = crate::catalog::load(&source_path).unwrap();
        let install = synthetic_install("fatman", "/games/fatman");
        let mut user = UserConfig::default();
        user.emulators.fs_uae.fullscreen = true;

        let plan = compose_fs_uae(&entry, &source_path, &install, &user).unwrap();

        assert!(plan.primary.args.contains(&"--fullscreen=1".to_string()));
        assert!(
            !plan.primary.args.iter().any(|a| a.starts_with("--window_")),
            "fullscreen launch should not set a window size: {:?}",
            plan.primary.args
        );
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
        assert!(
            line.contains(r#""MOUNT C \"/home/me/games/qfg1\"""#),
            "got {line}"
        );
    }
}
