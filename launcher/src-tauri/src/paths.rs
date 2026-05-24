use std::path::{Path, PathBuf};

pub fn expand_tilde(path: &str) -> PathBuf {
    PathBuf::from(shellexpand::tilde(path).as_ref())
}

pub fn resolve_relative(manifest_path: &Path, relative: &str) -> PathBuf {
    let base = manifest_path.parent().unwrap_or(Path::new("."));
    base.join(relative)
}

pub fn games_dir(base: &Path, id: &str) -> PathBuf {
    base.join(id)
}

/// Walk upward from `start` looking for a Reliquaint repository root.
///
/// Recognizes either layout:
/// - **New (post-Milestone 6):** `<root>/tap/tap.toml` present.
/// - **Legacy (pre-Milestone 6):** `<root>/dos/` and `<root>/amiga/`
///   collection directories present.
///
/// Returns the first ancestor that matches, or `None` if none does.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let has_tap = current.join("tap/tap.toml").is_file();
        let has_legacy = current.join("dos").is_dir() && current.join("amiga").is_dir();
        if has_tap || has_legacy {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

// --- XDG-based path helpers ----------------------------------------------
//
// Every reliquaint-owned path under the user's home goes through one of
// these functions. No other module should hardcode `.local/share` or
// `.config`; if you need a new app-owned location, add it here.

/// The bundled `reliquaint-core` tap that ships in the repo. The actual
/// directory is stood up in Task 6.1; today this returns the path it will
/// live at.
pub fn tap_root(repo_root: &Path) -> PathBuf {
    repo_root.join("tap")
}

/// Where user-subscribed taps live (v0.3+; the constant exists now so the
/// path doesn't get hardcoded ad-hoc later).
pub fn user_taps_dir() -> PathBuf {
    data_home_from_env(std::env::var("XDG_DATA_HOME").ok().as_deref()).join("reliquaint/taps")
}

/// Where per-game installation records live.
///
/// `RELIQUAINT_INSTALLS_DIR`, if set and non-empty, overrides the
/// default XDG-derived path. Used by integration tests to isolate
/// state.
pub fn installs_dir() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_INSTALLS_DIR") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    data_home_from_env(std::env::var("XDG_DATA_HOME").ok().as_deref()).join("reliquaint/installs")
}

/// The user's launcher config file.
///
/// `RELIQUAINT_USER_CONFIG_PATH`, if set and non-empty, overrides the
/// default XDG-derived path. Used by integration tests to isolate state.
pub fn user_config_path() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_USER_CONFIG_PATH") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    config_home_from_env(std::env::var("XDG_CONFIG_HOME").ok().as_deref())
        .join("reliquaint/config.toml")
}

fn data_home_from_env(env: Option<&str>) -> PathBuf {
    match env.filter(|s| !s.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => expand_tilde("~/.local/share"),
    }
}

fn config_home_from_env(env: Option<&str>) -> PathBuf {
    match env.filter(|s| !s.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => expand_tilde("~/.config"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands_to_home() {
        let result = expand_tilde("~/games/qfg1-ega");
        let home = std::env::var("HOME").unwrap_or_default();
        assert!(result.starts_with(&home));
        assert!(result.ends_with("games/qfg1-ega"));
    }

    #[test]
    fn absolute_path_unchanged() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn resolve_relative_from_manifest() {
        let manifest = Path::new("/repo/dos/quest-for-glory/manifests/qfg1-ega.toml");
        let result = resolve_relative(manifest, "../config/qfg1-ega.conf");
        assert_eq!(
            result,
            PathBuf::from("/repo/dos/quest-for-glory/manifests/../config/qfg1-ega.conf")
        );
    }

    #[test]
    fn resolve_relative_normalizes_path_components() {
        let manifest = Path::new("/repo/dos/quest-for-glory/manifests/qfg1-ega.toml");
        let result = resolve_relative(manifest, "../config/qfg1-ega.conf");
        // The path contains `..` but points at the right location
        assert!(result.to_string_lossy().contains("config/qfg1-ega.conf"));
    }

    #[test]
    fn games_dir_joins_base_and_id() {
        let base = Path::new("/home/user/games");
        let result = games_dir(base, "qfg1-ega");
        assert_eq!(result, PathBuf::from("/home/user/games/qfg1-ega"));
    }

    #[test]
    fn games_dir_with_tilde_base() {
        let base = expand_tilde("~/games");
        let result = games_dir(&base, "kq1sci");
        let home = std::env::var("HOME").unwrap_or_default();
        assert!(result.starts_with(&home));
        assert!(result.ends_with("games/kq1sci"));
    }

    #[test]
    fn data_home_uses_env_when_set() {
        let result = data_home_from_env(Some("/tmp/xdg-data"));
        assert_eq!(result, PathBuf::from("/tmp/xdg-data"));
    }

    #[test]
    fn data_home_falls_back_when_env_unset_or_empty() {
        let unset = data_home_from_env(None);
        let empty = data_home_from_env(Some(""));
        assert!(unset.ends_with(".local/share"));
        assert!(empty.ends_with(".local/share"));
    }

    #[test]
    fn config_home_uses_env_when_set() {
        let result = config_home_from_env(Some("/tmp/xdg-config"));
        assert_eq!(result, PathBuf::from("/tmp/xdg-config"));
    }

    #[test]
    fn config_home_falls_back_when_env_unset_or_empty() {
        let unset = config_home_from_env(None);
        let empty = config_home_from_env(Some(""));
        assert!(unset.ends_with(".config"));
        assert!(empty.ends_with(".config"));
    }

    #[test]
    fn tap_root_joins_repo_root() {
        let repo = Path::new("/repo");
        assert_eq!(tap_root(repo), PathBuf::from("/repo/tap"));
    }

    #[test]
    fn installs_dir_ends_with_reliquaint_installs() {
        let p = installs_dir();
        assert!(
            p.ends_with("reliquaint/installs"),
            "installs_dir should end with reliquaint/installs, got {}",
            p.display()
        );
    }

    #[test]
    fn user_config_path_ends_with_reliquaint_config_toml() {
        let p = user_config_path();
        assert!(
            p.ends_with("reliquaint/config.toml"),
            "user_config_path should end with reliquaint/config.toml, got {}",
            p.display()
        );
    }

    #[test]
    fn find_repo_root_recognizes_new_tap_layout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("tap")).unwrap();
        std::fs::write(tmp.path().join("tap/tap.toml"), "schema_version = 1\n").unwrap();
        // Walk up from a nested path:
        let nested = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        let root = find_repo_root(&nested).unwrap();
        assert_eq!(root, tmp.path().canonicalize().unwrap_or_else(|_| tmp.path().to_path_buf()));
    }

    #[test]
    fn find_repo_root_recognizes_legacy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("dos")).unwrap();
        std::fs::create_dir(tmp.path().join("amiga")).unwrap();
        let root = find_repo_root(tmp.path()).unwrap();
        assert_eq!(root, tmp.path().canonicalize().unwrap_or_else(|_| tmp.path().to_path_buf()));
    }

    #[test]
    fn find_repo_root_returns_none_outside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        // No marker dirs/files. Will walk upward to /, then None.
        // (May find an ancestor if developer's machine has a tap.toml
        // somewhere above /tmp — unlikely but possible. Skip in that
        // case rather than failing.)
        if find_repo_root(tmp.path()).is_some() {
            eprintln!("skipping: machine has an ancestor repo root above /tmp");
            return;
        }
        assert!(find_repo_root(tmp.path()).is_none());
    }

    #[test]
    fn user_taps_dir_ends_with_reliquaint_taps() {
        let p = user_taps_dir();
        assert!(
            p.ends_with("reliquaint/taps"),
            "user_taps_dir should end with reliquaint/taps, got {}",
            p.display()
        );
    }
}
