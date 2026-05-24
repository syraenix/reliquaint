use crate::manifest::{self, Manifest};
use std::path::{Path, PathBuf};

pub fn discover_catalog(repo_root: &Path) -> Vec<(PathBuf, Manifest)> {
    let mut scan_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(repo_root.join("dos")) {
        for entry in entries.flatten() {
            let manifests = entry.path().join("manifests");
            if manifests.is_dir() {
                scan_dirs.push(manifests);
            }
        }
    }
    scan_dirs.push(repo_root.join("amiga/manifests"));

    let mut results = Vec::new();
    for dir in &scan_dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == ".gitkeep" || name.ends_with(".disabled") {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match manifest::parse_file(&path) {
                Ok(m) => results.push((path, m)),
                Err(e) => eprintln!("warning: skipping {}: {e}", path.display()),
            }
        }
    }
    results
}

pub fn find_by_id(repo_root: &Path, id: &str) -> Option<(PathBuf, Manifest)> {
    discover_catalog(repo_root).into_iter().find(|(_, m)| m.id == id)
}

pub fn find_artwork(game_dir: &Path) -> Option<PathBuf> {
    let priority_stems = ["cover", "icon", "box"];
    let image_exts = ["png", "jpg", "jpeg", "bmp"];
    for stem in &priority_stems {
        for ext in &image_exts {
            let p = game_dir.join(format!("{stem}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg" | "bmp") {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub struct InstalledEntry {
    pub manifest_path: PathBuf,
    pub manifest: Manifest,
    pub install_dir: PathBuf,
    pub artwork_path: Option<PathBuf>,
}

pub fn discover_installed(repo_root: &Path, games_base: &Path) -> Vec<InstalledEntry> {
    let catalog: std::collections::HashMap<String, (PathBuf, Manifest)> =
        discover_catalog(repo_root)
            .into_iter()
            .map(|(p, m)| (m.id.clone(), (p, m)))
            .collect();

    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(games_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let Some((manifest_path, manifest)) = catalog.get(&dir_name).cloned() else {
                continue;
            };
            let non_empty = std::fs::read_dir(&path)
                .ok()
                .and_then(|mut d| d.next())
                .is_some();
            if !non_empty {
                continue;
            }
            let artwork_path = find_artwork(&path);
            results.push(InstalledEntry { manifest_path, manifest, install_dir: path, artwork_path });
        }
    }
    results.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn discover_catalog_finds_fixture_manifests() {
        let root = fixture_root();
        let mut manifests = discover_catalog(&root);
        manifests.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
        let ids: Vec<&str> = manifests.iter().map(|(_, m)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["kq1sci", "qfg1-ega"]);
    }

    #[test]
    fn discover_catalog_skips_disabled_and_gitkeep() {
        let root = fixture_root();
        let manifests = discover_catalog(&root);
        for (path, _) in &manifests {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(!name.ends_with(".disabled"), "should skip .disabled: {name}");
            assert_ne!(name, ".gitkeep", "should skip .gitkeep");
        }
    }

    #[test]
    fn find_by_id_returns_correct_manifest() {
        let root = fixture_root();
        let result = find_by_id(&root, "qfg1-ega");
        assert!(result.is_some());
        let (_, m) = result.unwrap();
        assert_eq!(m.id, "qfg1-ega");
    }

    #[test]
    fn find_by_id_returns_none_for_unknown() {
        let root = fixture_root();
        assert!(find_by_id(&root, "nonexistent").is_none());
    }

    #[test]
    fn discover_installed_returns_games_in_games_base() {
        let root = fixture_root();
        let temp = tempfile::tempdir().unwrap();
        // qfg1-ega dir exists and is non-empty
        let qfg_dir = temp.path().join("qfg1-ega");
        std::fs::create_dir(&qfg_dir).unwrap();
        std::fs::write(qfg_dir.join("QUEST.EXE"), b"").unwrap();

        let installed = discover_installed(&root, temp.path());
        let ids: Vec<&str> = installed.iter().map(|e| e.manifest.id.as_str()).collect();
        assert!(ids.contains(&"qfg1-ega"));
    }

    #[test]
    fn discover_installed_ignores_empty_dirs() {
        let root = fixture_root();
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("qfg1-ega")).unwrap(); // empty

        let installed = discover_installed(&root, temp.path());
        assert!(installed.is_empty());
    }

    #[test]
    fn discover_installed_ignores_unknown_dirs() {
        let root = fixture_root();
        let temp = tempfile::tempdir().unwrap();
        let unknown = temp.path().join("mystery-game");
        std::fs::create_dir(&unknown).unwrap();
        std::fs::write(unknown.join("game.exe"), b"").unwrap();

        let installed = discover_installed(&root, temp.path());
        assert!(installed.is_empty());
    }

    #[test]
    fn find_artwork_prefers_named_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("random.png"), b"").unwrap();
        std::fs::write(temp.path().join("cover.jpg"), b"").unwrap();
        let result = find_artwork(temp.path());
        assert!(result.unwrap().ends_with("cover.jpg"));
    }

    #[test]
    fn find_artwork_falls_back_to_any_image() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("splash.bmp"), b"").unwrap();
        let result = find_artwork(temp.path());
        assert!(result.is_some());
    }

    #[test]
    fn find_artwork_returns_none_when_no_images() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("game.exe"), b"").unwrap();
        assert!(find_artwork(temp.path()).is_none());
    }
}
