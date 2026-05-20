use crate::discovery::discover;
use crate::manifest::Platform;
use crate::paths::expand_tilde;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Ok,
    Missing,
    Unknown,
}

pub struct ProbeResult {
    pub name: String,
    pub status: ProbeStatus,
    pub detail: Option<String>,
}

pub fn run_all(repo_root: &Path) -> Vec<ProbeResult> {
    let mut results = Vec::new();

    results.push(probe_flatpak_dosbox());
    results.push(probe_which("fluidsynth"));
    results.push(probe_soundfont());
    results.push(probe_which("innoextract"));
    results.push(probe_which("fs-uae"));
    results.push(probe_which("unzip"));

    for (_, manifest) in discover(repo_root) {
        if manifest.platform != Platform::Dos {
            continue;
        }
        if let Some(install) = &manifest.install {
            let dir = expand_tilde(&install.expects_dir);
            let status = if dir.exists() {
                ProbeStatus::Ok
            } else {
                ProbeStatus::Missing
            };
            results.push(ProbeResult {
                name: format!("{} install dir", manifest.id),
                status,
                detail: Some(dir.display().to_string()),
            });
        }
    }

    results
}

fn probe_flatpak_dosbox() -> ProbeResult {
    let status = Command::new("flatpak")
        .args(["info", "io.github.dosbox-staging"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    ProbeResult {
        name: "dosbox-staging (flatpak)".to_string(),
        status: match status {
            Ok(s) if s.success() => ProbeStatus::Ok,
            Ok(_) => ProbeStatus::Missing,
            Err(_) => ProbeStatus::Unknown,
        },
        detail: None,
    }
}

fn probe_which(cmd: &str) -> ProbeResult {
    let status = Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    ProbeResult {
        name: cmd.to_string(),
        status: match status {
            Ok(s) if s.success() => ProbeStatus::Ok,
            Ok(_) => ProbeStatus::Missing,
            Err(_) => ProbeStatus::Unknown,
        },
        detail: None,
    }
}

fn probe_soundfont() -> ProbeResult {
    let path = "/usr/share/sounds/sf2/FluidR3_GM.sf2";
    ProbeResult {
        name: "soundfont".to_string(),
        status: if Path::new(path).exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        },
        detail: Some(path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn install_dir_ok_when_exists() {
        let temp = tempfile::tempdir().unwrap();
        let game_dir = temp.path().join("mygame");
        std::fs::create_dir(&game_dir).unwrap();

        let dir_str = game_dir.to_str().unwrap().to_string();
        let dir = expand_tilde(&dir_str);
        let status = if dir.exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        };
        assert_eq!(status, ProbeStatus::Ok);
    }

    #[test]
    fn install_dir_missing_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let game_dir = temp.path().join("nonexistent");
        let dir_str = game_dir.to_str().unwrap().to_string();
        let dir = expand_tilde(&dir_str);
        let status = if dir.exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        };
        assert_eq!(status, ProbeStatus::Missing);
    }

    #[test]
    fn run_all_includes_host_probes() {
        let root = fixture_root();
        let results = run_all(&root);
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"dosbox-staging (flatpak)"));
        assert!(names.contains(&"fluidsynth"));
        assert!(names.contains(&"soundfont"));
        assert!(names.contains(&"innoextract"));
        assert!(names.contains(&"fs-uae"));
        assert!(names.contains(&"unzip"));
    }

    #[test]
    fn run_all_includes_manifest_dir_checks() {
        let root = fixture_root();
        let results = run_all(&root);
        let has_install_check = results
            .iter()
            .any(|r| r.name.contains("install dir"));
        assert!(has_install_check);
    }
}
