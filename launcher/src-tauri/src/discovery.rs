use crate::manifest::{self, Manifest};
use std::path::{Path, PathBuf};

pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join("dos").is_dir() && current.join("amiga").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn discover(repo_root: &Path) -> Vec<(PathBuf, Manifest)> {
    let scan_dirs = [
        repo_root.join("dos/quest-for-glory/manifests"),
        repo_root.join("dos/kings-quest/manifests"),
        repo_root.join("amiga/manifests"),
    ];

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
    discover(repo_root).into_iter().find(|(_, m)| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn discover_finds_fixture_manifests() {
        let root = fixture_root();
        let mut manifests = discover(&root);
        manifests.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
        let ids: Vec<&str> = manifests.iter().map(|(_, m)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["kq1sci", "qfg1-ega"]);
    }

    #[test]
    fn discover_skips_disabled_and_gitkeep() {
        let root = fixture_root();
        let manifests = discover(&root);
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
    fn find_repo_root_walks_upward() {
        let deep = fixture_root().join("dos/quest-for-glory/manifests");
        let root = find_repo_root(&deep);
        assert!(root.is_some());
        assert_eq!(root.unwrap(), fixture_root());
    }
}
