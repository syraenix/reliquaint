use crate::manifest::{Manifest, Platform};
use crate::paths::resolve_relative;
use anyhow::anyhow;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

pub struct RunOpts {
    pub dry_run: bool,
    pub windowed: bool,
}

const SOUNDFONT_PATH: &str = "/usr/share/sounds/sf2/FluidR3_GM.sf2";

pub fn build_fluidsynth_command() -> Command {
    let mut cmd = Command::new("fluidsynth");
    cmd.arg("-i").arg(SOUNDFONT_PATH);
    cmd
}

pub fn build_dos_command(manifest_path: &Path, manifest: &Manifest) -> anyhow::Result<Command> {
    let config_rel = manifest
        .runtime
        .config
        .as_deref()
        .ok_or_else(|| anyhow!("DOS manifest missing runtime.config"))?;
    let abs_config = resolve_relative(manifest_path, config_rel);
    let mut cmd = Command::new("flatpak");
    cmd.args(["run", "io.github.dosbox-staging", "-conf"])
        .arg(abs_config);
    Ok(cmd)
}

pub fn build_amiga_adf_command(
    adf_path: &Path,
    model_config: &Path,
    windowed: bool,
) -> Command {
    let mut cmd = Command::new("fs-uae");
    cmd.arg(model_config)
        .arg(format!("--floppy_drive_0={}", adf_path.display()));
    if windowed {
        cmd.arg("--fullscreen=0");
    }
    cmd
}

pub fn build_amiga_rp9_command(
    rp9_path: &Path,
    model_config: &Path,
    windowed: bool,
) -> anyhow::Result<(Command, tempfile::TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let file = std::fs::File::open(rp9_path)
        .map_err(|e| anyhow!("cannot open rp9 {}: {e}", rp9_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| anyhow!("cannot read rp9 zip {}: {e}", rp9_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let out_path = temp_dir.path().join(&name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }

    let inner_fsuae = find_first_match(temp_dir.path(), "fs-uae");
    let inner_adf = find_first_match(temp_dir.path(), "adf");

    let cmd = if let Some(cfg_path) = inner_fsuae {
        let mut c = Command::new("fs-uae");
        c.arg(&cfg_path);
        if windowed {
            c.arg("--fullscreen=0");
        }
        c
    } else if let Some(adf_path) = inner_adf {
        build_amiga_adf_command(&adf_path, model_config, windowed)
    } else {
        return Err(anyhow!(
            "rp9 bundle {} has no .fs-uae config or .adf file",
            rp9_path.display()
        ));
    };

    Ok((cmd, temp_dir))
}

fn find_first_match(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_first_match(&path, ext) {
                return Some(found);
            }
        }
    }
    None
}

fn resolve_amiga_model_config(repo_root: &Path, model: &str) -> anyhow::Result<PathBuf> {
    let config_path = repo_root
        .join("amiga/config")
        .join(format!("{model}.fs-uae"));
    if !config_path.exists() {
        return Err(anyhow!("model config not found: {}", config_path.display()));
    }
    Ok(config_path)
}

pub fn format_command(cmd: &Command) -> String {
    let prog = cmd.get_program().to_string_lossy();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    if args.is_empty() {
        prog.into_owned()
    } else {
        format!("{prog} {}", args.join(" "))
    }
}

pub fn run(
    manifest_path: &Path,
    manifest: &Manifest,
    repo_root: &Path,
    opts: &RunOpts,
) -> anyhow::Result<ExitCode> {
    match manifest.platform {
        Platform::Dos => run_dos(manifest_path, manifest, opts),
        Platform::Amiga => run_amiga(manifest_path, manifest, repo_root, opts),
    }
}

fn run_dos(
    manifest_path: &Path,
    manifest: &Manifest,
    opts: &RunOpts,
) -> anyhow::Result<ExitCode> {
    let mut primary_cmd = build_dos_command(manifest_path, manifest)?;

    if opts.dry_run {
        for sidecar in &manifest.runtime.sidecars {
            if sidecar == "fluidsynth" {
                println!("[sidecar] {}", format_command(&build_fluidsynth_command()));
            }
        }
        println!("[primary] {}", format_command(&primary_cmd));
        return Ok(ExitCode::SUCCESS);
    }

    for sidecar in &manifest.runtime.sidecars {
        if sidecar == "fluidsynth" {
            build_fluidsynth_command().spawn()?;
            wait_for_fluidsynth();
        }
    }

    let status = primary_cmd
        .status()
        .map_err(|e| anyhow!("failed to spawn dosbox-staging: {e}"))?;

    Ok(exit_code_from_status(status))
}

fn run_amiga(
    manifest_path: &Path,
    manifest: &Manifest,
    repo_root: &Path,
    opts: &RunOpts,
) -> anyhow::Result<ExitCode> {
    let file_rel = manifest
        .runtime
        .file
        .as_deref()
        .ok_or_else(|| anyhow!("Amiga manifest missing runtime.file"))?;
    let abs_file = resolve_relative(manifest_path, file_rel);
    let model = manifest.runtime.model.as_deref().unwrap_or("a500");
    let model_config = resolve_amiga_model_config(repo_root, model)?;

    let ext = abs_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "adf" => {
            let mut cmd = build_amiga_adf_command(&abs_file, &model_config, opts.windowed);
            if opts.dry_run {
                println!("[primary] {}", format_command(&cmd));
                return Ok(ExitCode::SUCCESS);
            }
            let status = cmd
                .status()
                .map_err(|e| anyhow!("failed to spawn fs-uae: {e}"))?;
            Ok(exit_code_from_status(status))
        }
        "rp9" => {
            let (mut cmd, _keep) =
                build_amiga_rp9_command(&abs_file, &model_config, opts.windowed)?;
            if opts.dry_run {
                println!("[primary] {}", format_command(&cmd));
                return Ok(ExitCode::SUCCESS);
            }
            let status = cmd
                .status()
                .map_err(|e| anyhow!("failed to spawn fs-uae: {e}"))?;
            Ok(exit_code_from_status(status))
        }
        other => Err(anyhow!("unsupported Amiga file extension: .{other}")),
    }
}

fn wait_for_fluidsynth() {
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let found = Command::new("pgrep")
            .args(["-x", "fluidsynth"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if found {
            return;
        }
    }
    eprintln!("warning: fluidsynth did not start after 10s, proceeding anyway");
}

fn exit_code_from_status(status: std::process::ExitStatus) -> ExitCode {
    match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(n) => ExitCode::from(n as u8),
        None => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn qfg1_manifest_path() -> PathBuf {
        fixture_root().join("dos/quest-for-glory/manifests/qfg1-ega.toml")
    }

    fn qfg1_manifest() -> Manifest {
        crate::manifest::parse_file(&qfg1_manifest_path()).unwrap()
    }

    #[test]
    fn fluidsynth_command_has_correct_args() {
        let cmd = build_fluidsynth_command();
        assert_eq!(cmd.get_program(), "fluidsynth");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, ["-i", SOUNDFONT_PATH]);
    }

    #[test]
    fn dos_command_uses_flatpak() {
        let path = qfg1_manifest_path();
        let m = qfg1_manifest();
        let cmd = build_dos_command(&path, &m).unwrap();
        assert_eq!(cmd.get_program(), "flatpak");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "io.github.dosbox-staging");
        assert_eq!(args[2], "-conf");
        assert!(args[3].contains("config/qfg1-ega.conf"));
    }

    #[test]
    fn amiga_adf_command_structure() {
        let adf = Path::new("/games/lemmings.adf");
        let model = Path::new("/repo/amiga/config/a500.fs-uae");
        let cmd = build_amiga_adf_command(adf, model, false);
        assert_eq!(cmd.get_program(), "fs-uae");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "/repo/amiga/config/a500.fs-uae");
        assert_eq!(args[1], "--floppy_drive_0=/games/lemmings.adf");
        assert!(!args.contains(&"--fullscreen=0".to_string()));
    }

    #[test]
    fn amiga_adf_command_windowed() {
        let adf = Path::new("/games/lemmings.adf");
        let model = Path::new("/repo/amiga/config/a500.fs-uae");
        let cmd = build_amiga_adf_command(adf, model, true);
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--fullscreen=0".to_string()));
    }

    #[test]
    fn rp9_with_inner_adf() {
        let temp = tempfile::tempdir().unwrap();
        let model_config = temp.path().join("a500.fs-uae");
        std::fs::write(&model_config, "").unwrap();
        let rp9_path = make_test_rp9(temp.path(), &[("game.adf", b"dummy adf content")]);
        let (cmd, _keep) =
            build_amiga_rp9_command(&rp9_path, &model_config, false).unwrap();
        assert_eq!(cmd.get_program(), "fs-uae");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a.contains("a500.fs-uae")));
        assert!(args.iter().any(|a| a.contains("game.adf")));
    }

    #[test]
    fn rp9_with_inner_fsuae_config() {
        let temp = tempfile::tempdir().unwrap();
        let model_config = temp.path().join("a500.fs-uae");
        std::fs::write(&model_config, "").unwrap();
        let rp9_path = make_test_rp9(
            temp.path(),
            &[("Config.fs-uae", b"[config]\n"), ("game.adf", b"dummy")],
        );
        let (cmd, _keep) =
            build_amiga_rp9_command(&rp9_path, &model_config, false).unwrap();
        assert_eq!(cmd.get_program(), "fs-uae");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a.contains("Config.fs-uae")));
        assert!(!args.iter().any(|a| a.contains("--floppy_drive_0")));
    }

    #[test]
    fn rp9_with_neither_errors() {
        let temp = tempfile::tempdir().unwrap();
        let model_config = temp.path().join("a500.fs-uae");
        std::fs::write(&model_config, "").unwrap();
        let rp9_path = make_test_rp9(temp.path(), &[("readme.txt", b"nothing useful")]);
        assert!(build_amiga_rp9_command(&rp9_path, &model_config, false).is_err());
    }

    #[test]
    fn rp9_windowed_flag_propagates() {
        let temp = tempfile::tempdir().unwrap();
        let model_config = temp.path().join("a500.fs-uae");
        std::fs::write(&model_config, "").unwrap();
        let rp9_path = make_test_rp9(temp.path(), &[("game.adf", b"dummy")]);
        let (cmd, _keep) =
            build_amiga_rp9_command(&rp9_path, &model_config, true).unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--fullscreen=0".to_string()));
    }

    fn make_test_rp9(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        use std::io::Write;
        let rp9_path = dir.join("test.rp9");
        let file = std::fs::File::create(&rp9_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::FileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
        rp9_path
    }
}
