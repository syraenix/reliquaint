use crate::discovery::discover_catalog;
use crate::paths::games_dir;
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

pub fn run_all(repo_root: &Path, games_base: &Path) -> Vec<ProbeResult> {
    let mut results = vec![
        probe_flatpak_dosbox(),
        probe_which("fluidsynth", ProbeKind::Fluidsynth),
        probe_soundfont(),
        probe_which("innoextract", ProbeKind::Innoextract),
        probe_which("fs-uae", ProbeKind::FsUae),
        probe_which("unzip", ProbeKind::Unzip),
    ];

    for (_, manifest) in discover_catalog(repo_root) {
        let dir = games_dir(games_base, &manifest.id);
        let status = if dir.exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        };
        results.push(ProbeResult {
            name: format!("{} install dir", manifest.id),
            status,
            detail: Some(dir.display().to_string()),
            kind: ProbeKind::GameInstallDir(manifest.id.clone()),
        });
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

        let status = if game_dir.exists() {
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
        let status = if game_dir.exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        };
        assert_eq!(status, ProbeStatus::Missing);
    }

    #[test]
    fn run_all_includes_host_probes() {
        let root = fixture_root();
        let games_base = tempfile::tempdir().unwrap();
        let results = run_all(&root, games_base.path());
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
        let games_base = tempfile::tempdir().unwrap();
        let results = run_all(&root, games_base.path());
        assert!(results.iter().any(|r| r.name.contains("install dir")));
    }

    #[test]
    fn install_dir_probe_kind_carries_id() {
        let root = fixture_root();
        let games_base = tempfile::tempdir().unwrap();
        let results = run_all(&root, games_base.path());
        let ids: Vec<String> = results
            .iter()
            .filter_map(|r| {
                if let ProbeKind::GameInstallDir(id) = &r.kind {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(ids.contains(&"qfg1-ega".to_string()));
        assert!(ids.contains(&"kq1sci".to_string()));
    }

    #[test]
    fn host_probes_carry_expected_kinds() {
        let root = fixture_root();
        let games_base = tempfile::tempdir().unwrap();
        let results = run_all(&root, games_base.path());
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
