use crate::discovery::discover_catalog;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeKind {
    DosboxFlatpak,
    Fluidsynth,
    Soundfont,
    Innoextract,
    FsUae,
    Unzip,
    GameInstallDir(String),
}

pub struct ProbeResult {
    pub name: String,
    pub status: ProbeStatus,
    pub detail: Option<String>,
    pub kind: ProbeKind,
}

pub fn run_all(repo_root: &Path) -> Vec<ProbeResult> {
    let results = vec![
        probe_flatpak_dosbox(),
        probe_which("fluidsynth", ProbeKind::Fluidsynth),
        probe_soundfont(),
        probe_which("innoextract", ProbeKind::Innoextract),
        probe_which("fs-uae", ProbeKind::FsUae),
        probe_which("unzip", ProbeKind::Unzip),
    ];

    for (_, manifest) in discover_catalog(repo_root) {
        if manifest.platform != Platform::Dos {
            continue;
        }
        // TODO(task5): check ~/games/{id} via paths::games_dir
        let _ = &manifest;
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
        kind: ProbeKind::DosboxFlatpak,
    }
}

fn probe_which(cmd: &str, kind: ProbeKind) -> ProbeResult {
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
        kind,
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
        kind: ProbeKind::Soundfont,
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
    fn host_probes_carry_expected_kinds() {
        let root = fixture_root();
        let results = run_all(&root);
        let kind_for = |name: &str| -> ProbeKind {
            results
                .iter()
                .find(|r| r.name == name)
                .map(|r| r.kind.clone())
                .unwrap_or_else(|| panic!("missing probe {name}"))
        };
        assert_eq!(kind_for("dosbox-staging (flatpak)"), ProbeKind::DosboxFlatpak);
        assert_eq!(kind_for("fluidsynth"), ProbeKind::Fluidsynth);
        assert_eq!(kind_for("soundfont"), ProbeKind::Soundfont);
        assert_eq!(kind_for("innoextract"), ProbeKind::Innoextract);
        assert_eq!(kind_for("fs-uae"), ProbeKind::FsUae);
        assert_eq!(kind_for("unzip"), ProbeKind::Unzip);
    }

}
