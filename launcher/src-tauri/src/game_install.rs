use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Collection {
    QuestForGlory,
    KingsQuest,
}

impl Collection {
    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "quest-for-glory" => Some(Self::QuestForGlory),
            "kings-quest" => Some(Self::KingsQuest),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallerEntry {
    pub game_id: String,
    pub label: String,
    pub source: PathBuf,
    pub target: PathBuf,
}

const QFG_GAMES: &[(&str, &str, &str)] = &[
    ("qfg1-vga", "qfg1.exe", "Quest for Glory 1 (VGA)"),
    ("qfg1-ega", "qfg1.exe", "Quest for Glory 1 (EGA)"),
    ("qfg2", "qfg2.exe", "Quest for Glory 2"),
    ("qfg3", "qfg3.exe", "Quest for Glory 3"),
    ("qfg4", "qfg4.exe", "Quest for Glory 4"),
];

const KQ_GAME_IDS: &[(&str, &str)] = &[
    ("kq1sci", "King's Quest 1 (SCI)"),
    ("kq2", "King's Quest 2"),
    ("kq3", "King's Quest 3"),
    ("kq4", "King's Quest 4 (SCI)"),
    ("kq5", "King's Quest 5"),
    ("kq6", "King's Quest 6"),
];

pub fn discover_qfg(installers_dir: &Path, games_base: &Path) -> Vec<InstallerEntry> {
    QFG_GAMES
        .iter()
        .filter_map(|(game_id, exe_name, label)| {
            let installer = installers_dir.join(exe_name);
            if installer.is_file() {
                Some(InstallerEntry {
                    game_id: (*game_id).to_string(),
                    label: (*label).to_string(),
                    source: installer,
                    target: games_base.join(game_id),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn make_kq_entry(game_id: &str, source: &Path, games_base: &Path) -> Result<InstallerEntry, String> {
    let label = KQ_GAME_IDS
        .iter()
        .find(|(id, _)| *id == game_id)
        .map(|(_, l)| *l)
        .ok_or_else(|| format!("unknown King's Quest game id: {game_id}"))?;
    if !source.is_dir() {
        return Err(format!(
            "source folder does not exist or is not a directory: {}",
            source.display()
        ));
    }
    Ok(InstallerEntry {
        game_id: game_id.to_string(),
        label: label.to_string(),
        source: source.to_path_buf(),
        target: games_base.join(game_id),
    })
}

pub fn build_qfg_commands(entry: &InstallerEntry, games_base: &Path) -> Vec<Vec<String>> {
    let target = entry.target.to_string_lossy().to_string();
    let source = entry.source.to_string_lossy().to_string();

    if entry.game_id == "qfg1-ega" {
        let vga_target = games_base.join("qfg1-vga");
        let ega_source = vga_target.join("EGA").to_string_lossy().to_string();
        return vec![
            vec!["rm".into(), "-rf".into(), target.clone()],
            vec!["mv".into(), ega_source, target],
        ];
    }

    vec![
        vec!["mkdir".into(), "-p".into(), target.clone()],
        vec![
            "innoextract".into(),
            "--exclude-temp".into(),
            "--silent".into(),
            "--output-dir".into(),
            target,
            source,
        ],
    ]
}

pub fn build_kq_commands(entry: &InstallerEntry, _games_base: &Path) -> Vec<Vec<String>> {
    let target = entry.target.to_string_lossy().to_string();
    let source_dot = format!("{}/.", entry.source.to_string_lossy());
    vec![
        vec!["rm".into(), "-rf".into(), target.clone()],
        vec!["mkdir".into(), "-p".into(), target.clone()],
        vec!["cp".into(), "-r".into(), "--".into(), source_dot, target],
    ]
}

pub fn build_amiga_copy_commands(source: &Path, target_dir: &Path) -> Vec<Vec<String>> {
    let target = target_dir.to_string_lossy().to_string();
    vec![
        vec!["mkdir".into(), "-p".into(), target.clone()],
        vec!["cp".into(), "--".into(), source.to_string_lossy().to_string(), target],
    ]
}

pub fn install_order(entries: Vec<InstallerEntry>) -> Vec<InstallerEntry> {
    let mut sorted = entries;
    sorted.sort_by_key(|e| match e.game_id.as_str() {
        "qfg1-vga" => 0,
        "qfg1-ega" => 1,
        _ => 2,
    });
    sorted
}

pub fn validate_collection_membership(
    collection: Collection,
    entries: &[InstallerEntry],
) -> Result<(), String> {
    for e in entries {
        let belongs = match collection {
            Collection::QuestForGlory => QFG_GAMES.iter().any(|(id, _, _)| *id == e.game_id),
            Collection::KingsQuest => KQ_GAME_IDS.iter().any(|(id, _)| *id == e.game_id),
        };
        if !belongs {
            return Err(format!(
                "game id '{}' does not belong to collection {:?}",
                e.game_id, collection
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_qfg_finds_known_exes() {
        let temp = tempfile::tempdir().unwrap();
        let games_base = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("qfg1.exe"), b"").unwrap();
        std::fs::write(temp.path().join("qfg2.exe"), b"").unwrap();

        let entries = discover_qfg(temp.path(), games_base.path());
        let ids: Vec<&str> = entries.iter().map(|e| e.game_id.as_str()).collect();
        assert!(ids.contains(&"qfg1-vga"));
        assert!(ids.contains(&"qfg1-ega"));
        assert!(ids.contains(&"qfg2"));
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn discover_qfg_ignores_qfg5() {
        let temp = tempfile::tempdir().unwrap();
        let games_base = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("qfg5.exe"), b"").unwrap();
        let entries = discover_qfg(temp.path(), games_base.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn discover_qfg_empty_when_no_installers() {
        let temp = tempfile::tempdir().unwrap();
        let games_base = tempfile::tempdir().unwrap();
        let entries = discover_qfg(temp.path(), games_base.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn make_kq_entry_accepts_known_id_and_existing_dir() {
        let temp = tempfile::tempdir().unwrap();
        let games_base = Path::new("/home/user/games");
        let entry = make_kq_entry("kq1sci", temp.path(), games_base).unwrap();
        assert_eq!(entry.game_id, "kq1sci");
        assert!(entry.target.to_string_lossy().ends_with("games/kq1sci"));
    }

    #[test]
    fn make_kq_entry_rejects_unknown_id() {
        let temp = tempfile::tempdir().unwrap();
        let games_base = tempfile::tempdir().unwrap();
        let result = make_kq_entry("kq99", temp.path(), games_base.path());
        assert!(result.is_err());
    }

    #[test]
    fn make_kq_entry_rejects_missing_dir() {
        let games_base = tempfile::tempdir().unwrap();
        let result = make_kq_entry("kq1sci", Path::new("/this/path/should/not/exist/abc123"), games_base.path());
        assert!(result.is_err());
    }

    #[test]
    fn build_qfg_commands_for_qfg1_vga_uses_innoextract() {
        let games_base = Path::new("/home/user/games");
        let entry = InstallerEntry {
            game_id: "qfg1-vga".into(),
            label: "QFG1 VGA".into(),
            source: PathBuf::from("/installers/qfg1.exe"),
            target: PathBuf::from("/home/user/games/qfg1-vga"),
        };
        let cmds = build_qfg_commands(&entry, games_base);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0][0], "mkdir");
        assert_eq!(cmds[1][0], "innoextract");
        assert!(cmds[1].contains(&"--silent".to_string()));
        assert!(cmds[1].contains(&"/installers/qfg1.exe".to_string()));
    }

    #[test]
    fn build_qfg_commands_for_qfg1_ega_moves_from_vga_dir() {
        let games_base = Path::new("/home/user/games");
        let entry = InstallerEntry {
            game_id: "qfg1-ega".into(),
            label: "QFG1 EGA".into(),
            source: PathBuf::from("/installers/qfg1.exe"),
            target: PathBuf::from("/home/user/games/qfg1-ega"),
        };
        let cmds = build_qfg_commands(&entry, games_base);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0][0], "rm");
        assert_eq!(cmds[1][0], "mv");
        let mv_source = &cmds[1][1];
        assert!(mv_source.ends_with("games/qfg1-vga/EGA"));
        assert_eq!(cmds[1][2], entry.target.to_string_lossy());
    }

    #[test]
    fn build_qfg_commands_for_qfg2_uses_innoextract() {
        let games_base = Path::new("/home/user/games");
        let entry = InstallerEntry {
            game_id: "qfg2".into(),
            label: "QFG2".into(),
            source: PathBuf::from("/installers/qfg2.exe"),
            target: PathBuf::from("/home/user/games/qfg2"),
        };
        let cmds = build_qfg_commands(&entry, games_base);
        assert_eq!(cmds[1][0], "innoextract");
    }

    #[test]
    fn build_kq_commands_uses_cp_r_with_trailing_dot() {
        let games_base = Path::new("/home/user/games");
        let entry = InstallerEntry {
            game_id: "kq1sci".into(),
            label: "KQ1 SCI".into(),
            source: PathBuf::from("/steam/Kings Quest 1+2+3/kq1sci"),
            target: PathBuf::from("/home/user/games/kq1sci"),
        };
        let cmds = build_kq_commands(&entry, games_base);
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0][0], "rm");
        assert_eq!(cmds[1][0], "mkdir");
        assert_eq!(cmds[2][0], "cp");
        assert_eq!(cmds[2][1], "-r");
        assert_eq!(cmds[2][2], "--");
        assert!(cmds[2][3].ends_with("kq1sci/."));
        assert_eq!(cmds[2][4], "/home/user/games/kq1sci");
    }

    #[test]
    fn install_order_puts_vga_before_ega() {
        let entries = vec![
            InstallerEntry {
                game_id: "qfg1-ega".into(),
                label: "EGA".into(),
                source: PathBuf::from("/a.exe"),
                target: PathBuf::from("/t/qfg1-ega"),
            },
            InstallerEntry {
                game_id: "qfg2".into(),
                label: "QFG2".into(),
                source: PathBuf::from("/b.exe"),
                target: PathBuf::from("/t/qfg2"),
            },
            InstallerEntry {
                game_id: "qfg1-vga".into(),
                label: "VGA".into(),
                source: PathBuf::from("/c.exe"),
                target: PathBuf::from("/t/qfg1-vga"),
            },
        ];
        let ordered = install_order(entries);
        let ids: Vec<&str> = ordered.iter().map(|e| e.game_id.as_str()).collect();
        let vga_pos = ids.iter().position(|i| *i == "qfg1-vga").unwrap();
        let ega_pos = ids.iter().position(|i| *i == "qfg1-ega").unwrap();
        assert!(vga_pos < ega_pos);
    }

    #[test]
    fn validate_collection_rejects_mismatch() {
        let entry = InstallerEntry {
            game_id: "kq1sci".into(),
            label: "KQ1".into(),
            source: PathBuf::from("/x"),
            target: PathBuf::from("/y"),
        };
        let result = validate_collection_membership(Collection::QuestForGlory, &[entry]);
        assert!(result.is_err());
    }

    #[test]
    fn validate_collection_accepts_match() {
        let entry = InstallerEntry {
            game_id: "qfg2".into(),
            label: "QFG2".into(),
            source: PathBuf::from("/x"),
            target: PathBuf::from("/y"),
        };
        let result = validate_collection_membership(Collection::QuestForGlory, &[entry]);
        assert!(result.is_ok());
    }

    #[test]
    fn build_amiga_copy_commands_creates_dir_and_copies() {
        let source = Path::new("/downloads/lemmings.adf");
        let target = Path::new("/home/user/games/lemmings");
        let cmds = build_amiga_copy_commands(source, target);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0][0], "mkdir");
        assert!(cmds[0].contains(&"-p".to_string()));
        assert_eq!(cmds[0].last().unwrap(), "/home/user/games/lemmings");
        assert_eq!(cmds[1][0], "cp");
        assert!(cmds[1].contains(&"/downloads/lemmings.adf".to_string()));
        assert_eq!(cmds[1].last().unwrap(), "/home/user/games/lemmings");
    }
}
