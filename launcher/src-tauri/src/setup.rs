use crate::doctor::ProbeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
    DebianLike,
    Unknown,
}

pub enum InstallAction {
    Apt(Vec<&'static str>),
    FlatpakUserApp {
        remote: &'static str,
        remote_url: &'static str,
        app_id: &'static str,
    },
    Informational(&'static str),
}

pub fn detect_distro() -> Distro {
    match std::fs::read_to_string("/etc/os-release") {
        Ok(s) => detect_distro_from(&s),
        Err(_) => Distro::Unknown,
    }
}

pub fn detect_distro_from(os_release: &str) -> Distro {
    let mut id: Option<String> = None;
    let mut id_like: Option<String> = None;
    for line in os_release.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("ID=") {
            id = Some(strip_quotes(rest).to_ascii_lowercase());
        } else if let Some(rest) = line.strip_prefix("ID_LIKE=") {
            id_like = Some(strip_quotes(rest).to_ascii_lowercase());
        }
    }
    let matches_debian = |s: &str| {
        s.split_whitespace()
            .any(|tok| tok == "debian" || tok == "ubuntu")
    };
    if id.as_deref().map(matches_debian).unwrap_or(false)
        || id_like.as_deref().map(matches_debian).unwrap_or(false)
    {
        Distro::DebianLike
    } else {
        Distro::Unknown
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    s.trim_matches('"').trim_matches('\'')
}

pub fn action_for(kind: &ProbeKind, distro: Distro) -> Option<InstallAction> {
    match kind {
        ProbeKind::DosboxFlatpak => Some(InstallAction::FlatpakUserApp {
            remote: "flathub",
            remote_url: "https://flathub.org/repo/flathub.flatpakrepo",
            app_id: "io.github.dosbox-staging",
        }),
        ProbeKind::GameInstallDir(_) => Some(InstallAction::Informational(
            "Acquire and extract the game per the collection guide; the launcher cannot install game files.",
        )),
        ProbeKind::Fluidsynth => apt_if_debian(distro, vec!["fluidsynth"]),
        ProbeKind::Soundfont => apt_if_debian(distro, vec!["fluid-soundfont-gm"]),
        ProbeKind::Innoextract => apt_if_debian(distro, vec!["innoextract"]),
        ProbeKind::FsUae => apt_if_debian(distro, vec!["fs-uae"]),
        ProbeKind::Unzip => apt_if_debian(distro, vec!["unzip"]),
    }
}

fn apt_if_debian(distro: Distro, packages: Vec<&'static str>) -> Option<InstallAction> {
    match distro {
        Distro::DebianLike => Some(InstallAction::Apt(packages)),
        Distro::Unknown => None,
    }
}

pub fn build_commands(action: &InstallAction) -> Vec<Vec<String>> {
    match action {
        InstallAction::Apt(packages) => {
            let mut cmd = vec![
                "pkexec".to_string(),
                "apt".to_string(),
                "install".to_string(),
                "-y".to_string(),
            ];
            for p in packages {
                cmd.push((*p).to_string());
            }
            vec![cmd]
        }
        InstallAction::FlatpakUserApp {
            remote,
            remote_url,
            app_id,
        } => vec![
            vec![
                "flatpak".to_string(),
                "remote-add".to_string(),
                "--user".to_string(),
                "--if-not-exists".to_string(),
                (*remote).to_string(),
                (*remote_url).to_string(),
            ],
            vec![
                "flatpak".to_string(),
                "install".to_string(),
                "--user".to_string(),
                "-y".to_string(),
                (*remote).to_string(),
                (*app_id).to_string(),
            ],
        ],
        InstallAction::Informational(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debian_id_detected() {
        let input = "PRETTY_NAME=\"Debian GNU/Linux 12\"\nID=debian\n";
        assert_eq!(detect_distro_from(input), Distro::DebianLike);
    }

    #[test]
    fn ubuntu_id_like_detected() {
        let input = "ID=ubuntu\nID_LIKE=debian\n";
        assert_eq!(detect_distro_from(input), Distro::DebianLike);
    }

    #[test]
    fn pop_os_id_like_detected() {
        let input = "ID=pop\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(detect_distro_from(input), Distro::DebianLike);
    }

    #[test]
    fn fedora_not_detected_as_debian() {
        let input = "ID=fedora\n";
        assert_eq!(detect_distro_from(input), Distro::Unknown);
    }

    #[test]
    fn empty_os_release_unknown() {
        assert_eq!(detect_distro_from(""), Distro::Unknown);
    }

    #[test]
    fn action_for_dosbox_is_flatpak_on_any_distro() {
        for d in [Distro::DebianLike, Distro::Unknown] {
            let a = action_for(&ProbeKind::DosboxFlatpak, d).expect("dosbox always has action");
            assert!(matches!(a, InstallAction::FlatpakUserApp { .. }));
        }
    }

    #[test]
    fn action_for_apt_kinds_on_debian() {
        let kinds = [
            ProbeKind::Fluidsynth,
            ProbeKind::Soundfont,
            ProbeKind::Innoextract,
            ProbeKind::FsUae,
            ProbeKind::Unzip,
        ];
        for k in kinds {
            let a = action_for(&k, Distro::DebianLike)
                .unwrap_or_else(|| panic!("expected apt action for {k:?}"));
            assert!(matches!(a, InstallAction::Apt(_)));
        }
    }

    #[test]
    fn action_for_apt_kinds_none_on_unknown_distro() {
        let kinds = [
            ProbeKind::Fluidsynth,
            ProbeKind::Soundfont,
            ProbeKind::Innoextract,
            ProbeKind::FsUae,
            ProbeKind::Unzip,
        ];
        for k in kinds {
            assert!(action_for(&k, Distro::Unknown).is_none());
        }
    }

    #[test]
    fn action_for_game_install_dir_is_informational() {
        let a = action_for(
            &ProbeKind::GameInstallDir("qfg1-ega".into()),
            Distro::DebianLike,
        )
        .expect("informational");
        assert!(matches!(a, InstallAction::Informational(_)));
    }

    #[test]
    fn build_commands_apt_uses_pkexec() {
        let cmds = build_commands(&InstallAction::Apt(vec!["fluidsynth"]));
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0][0], "pkexec");
        assert_eq!(cmds[0][1], "apt");
        assert_eq!(cmds[0][2], "install");
        assert!(cmds[0].contains(&"fluidsynth".to_string()));
    }

    #[test]
    fn build_commands_flatpak_includes_remote_add_then_install() {
        let cmds = build_commands(&InstallAction::FlatpakUserApp {
            remote: "flathub",
            remote_url: "https://flathub.org/repo/flathub.flatpakrepo",
            app_id: "io.github.dosbox-staging",
        });
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0][0], "flatpak");
        assert_eq!(cmds[0][1], "remote-add");
        assert!(cmds[0].contains(&"--if-not-exists".to_string()));
        assert_eq!(cmds[1][0], "flatpak");
        assert_eq!(cmds[1][1], "install");
        assert!(cmds[1].contains(&"io.github.dosbox-staging".to_string()));
    }

    #[test]
    fn build_commands_informational_is_empty() {
        let cmds = build_commands(&InstallAction::Informational("nope"));
        assert!(cmds.is_empty());
    }
}
