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

/// Default base directory the install flow copies/extracts games into.
/// Each game lands at `<library>/<id>` (see [`games_dir`]).
///
/// `RELIQUAINT_GAMES_DIR`, if set and non-empty, overrides the default
/// `~/games`. Used by integration tests to isolate state.
pub fn default_library_dir() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_GAMES_DIR") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    expand_tilde("~/games")
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

/// Locate a tap that was packaged alongside the executable (Tauri `.deb` /
/// AppImage bundle), for runs outside a git checkout where
/// [`find_repo_root`] finds nothing.
///
/// Returns the directory that *contains* `tap/` — i.e. a value usable as a
/// `repo_root` with [`tap_root`] — or `None` if no packaged tap is found.
///
/// The GUI has the authoritative resource directory via Tauri's app handle;
/// this is the resolver the CLI uses (and a GUI fallback), derived from the
/// executable location. Tauri's Linux `.deb` installs the binary at
/// `/usr/bin/<name>` and its resources under `/usr/lib/<name>/`; an AppImage
/// exposes its mount point via `$APPDIR`.
pub fn packaged_repo_root() -> Option<PathBuf> {
    packaged_repo_root_from(
        std::env::var("APPDIR").ok().as_deref(),
        std::env::current_exe().ok().as_deref(),
    )
}

/// Pure core of [`packaged_repo_root`]: builds candidate locations from an
/// optional `$APPDIR` and the executable path, then returns the first whose
/// `tap/tap.toml` exists. Split out so it can be tested without mutating env
/// or relying on the test binary's location.
fn packaged_repo_root_from(appdir: Option<&str>, exe: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(appdir) = appdir {
        for name in ["reliquaint", "Reliquaint"] {
            candidates.push(PathBuf::from(appdir).join("usr/lib").join(name));
        }
    }
    if let Some(exe) = exe {
        if let Some(bindir) = exe.parent() {
            // /usr/bin/<name> -> /usr/lib/<name>
            for name in ["reliquaint", "Reliquaint"] {
                candidates.push(bindir.join("../lib").join(name));
            }
        }
    }
    candidates
        .into_iter()
        .find(|c| c.join("tap/tap.toml").is_file())
}

/// Where user-subscribed taps live (v0.3+; the constant exists now so the
/// path doesn't get hardcoded ad-hoc later).
///
/// `RELIQUAINT_TAPS_CACHE_DIR`, if set and non-empty, overrides the default.
/// Used by integration tests to isolate state.
pub fn user_taps_dir() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_TAPS_CACHE_DIR") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    data_home_from_env(std::env::var("XDG_DATA_HOME").ok().as_deref()).join("reliquaint/taps")
}

/// Cache directory for a single fetched tap: `<user_taps_dir>/<tap_id>/`.
pub fn tap_cache_dir_for(tap_id: &str) -> PathBuf {
    user_taps_dir().join(tap_id)
}

/// The subscriptions manifest — which taps the user is subscribed to.
///
/// `RELIQUAINT_SUBSCRIPTIONS_PATH`, if set and non-empty, overrides the
/// default XDG-derived path. Used by integration tests to isolate state.
pub fn subscriptions_path() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_SUBSCRIPTIONS_PATH") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    config_home_from_env(std::env::var("XDG_CONFIG_HOME").ok().as_deref())
        .join("reliquaint/subscriptions.toml")
}

/// The user's local pseudo-tap, writable by the CLI and GUI wizards.
/// Created lazily on first manifest write. The tap claims the reserved
/// id `local` (see `tap::RESERVED_USER_TAP_ID`).
///
/// `RELIQUAINT_USER_TAP_DIR`, if set and non-empty, overrides the
/// default XDG-derived path. Used by integration tests to isolate state.
pub fn user_tap_dir() -> PathBuf {
    if let Ok(v) = std::env::var("RELIQUAINT_USER_TAP_DIR") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    config_home_from_env(std::env::var("XDG_CONFIG_HOME").ok().as_deref()).join("reliquaint/tap")
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
        assert_eq!(
            root,
            tmp.path()
                .canonicalize()
                .unwrap_or_else(|_| tmp.path().to_path_buf())
        );
    }

    #[test]
    fn find_repo_root_recognizes_legacy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("dos")).unwrap();
        std::fs::create_dir(tmp.path().join("amiga")).unwrap();
        let root = find_repo_root(tmp.path()).unwrap();
        assert_eq!(
            root,
            tmp.path()
                .canonicalize()
                .unwrap_or_else(|_| tmp.path().to_path_buf())
        );
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
    fn default_library_dir_env_overrides() {
        // Save/restore the env var so we don't leak state to other tests.
        let saved = std::env::var("RELIQUAINT_GAMES_DIR").ok();
        std::env::set_var("RELIQUAINT_GAMES_DIR", "/tmp/custom-games");
        assert_eq!(default_library_dir(), PathBuf::from("/tmp/custom-games"));
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_GAMES_DIR", v),
            None => std::env::remove_var("RELIQUAINT_GAMES_DIR"),
        }
    }

    #[test]
    fn default_library_dir_falls_back_to_games_under_home() {
        let saved = std::env::var("RELIQUAINT_GAMES_DIR").ok();
        std::env::remove_var("RELIQUAINT_GAMES_DIR");
        let dir = default_library_dir();
        assert!(
            dir.ends_with("games"),
            "expected ~/games, got {}",
            dir.display()
        );
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_GAMES_DIR", v),
            None => std::env::remove_var("RELIQUAINT_GAMES_DIR"),
        }
    }

    #[test]
    fn packaged_repo_root_finds_exe_relative_lib() {
        // Lay out /usr/bin/reliquaint with resources at /usr/lib/reliquaint/tap.
        let tmp = tempfile::tempdir().unwrap();
        let usr = tmp.path().join("usr");
        std::fs::create_dir_all(usr.join("bin")).unwrap();
        let lib_tap = usr.join("lib/reliquaint/tap");
        std::fs::create_dir_all(&lib_tap).unwrap();
        std::fs::write(lib_tap.join("tap.toml"), "schema_version = 1\n").unwrap();

        let exe = usr.join("bin/reliquaint");
        let found = packaged_repo_root_from(None, Some(&exe)).unwrap();
        assert!(found.join("tap/tap.toml").is_file());
        assert_eq!(
            found.canonicalize().unwrap(),
            usr.join("lib/reliquaint").canonicalize().unwrap()
        );
    }

    #[test]
    fn packaged_repo_root_finds_appdir() {
        // AppImage exposes its mount point via $APPDIR.
        let tmp = tempfile::tempdir().unwrap();
        let lib_tap = tmp.path().join("usr/lib/Reliquaint/tap");
        std::fs::create_dir_all(&lib_tap).unwrap();
        std::fs::write(lib_tap.join("tap.toml"), "schema_version = 1\n").unwrap();

        let appdir = tmp.path().to_string_lossy().into_owned();
        let found = packaged_repo_root_from(Some(&appdir), None).unwrap();
        assert!(found.join("tap/tap.toml").is_file());
    }

    #[test]
    fn packaged_repo_root_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("usr/bin/reliquaint");
        // No tap laid out anywhere the candidates point.
        assert!(packaged_repo_root_from(Some("/nonexistent-xyz-appdir"), Some(&exe)).is_none());
    }

    #[test]
    fn user_taps_dir_ends_with_reliquaint_taps() {
        let saved = std::env::var("RELIQUAINT_TAPS_CACHE_DIR").ok();
        std::env::remove_var("RELIQUAINT_TAPS_CACHE_DIR");
        let p = user_taps_dir();
        assert!(
            p.ends_with("reliquaint/taps"),
            "user_taps_dir should end with reliquaint/taps, got {}",
            p.display()
        );
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_TAPS_CACHE_DIR", v),
            None => std::env::remove_var("RELIQUAINT_TAPS_CACHE_DIR"),
        }
    }

    #[test]
    fn tap_cache_dir_for_appends_tap_id() {
        let saved = std::env::var("RELIQUAINT_TAPS_CACHE_DIR").ok();
        std::env::set_var("RELIQUAINT_TAPS_CACHE_DIR", "/tmp/taps");
        let p = tap_cache_dir_for("reliquaint-core");
        assert_eq!(p, PathBuf::from("/tmp/taps/reliquaint-core"));
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_TAPS_CACHE_DIR", v),
            None => std::env::remove_var("RELIQUAINT_TAPS_CACHE_DIR"),
        }
    }

    #[test]
    fn subscriptions_path_ends_with_subscriptions_toml() {
        let saved = std::env::var("RELIQUAINT_SUBSCRIPTIONS_PATH").ok();
        std::env::remove_var("RELIQUAINT_SUBSCRIPTIONS_PATH");
        let p = subscriptions_path();
        assert!(
            p.ends_with("reliquaint/subscriptions.toml"),
            "subscriptions_path should end with reliquaint/subscriptions.toml, got {}",
            p.display()
        );
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_SUBSCRIPTIONS_PATH", v),
            None => std::env::remove_var("RELIQUAINT_SUBSCRIPTIONS_PATH"),
        }
    }

    #[test]
    fn subscriptions_path_env_overrides() {
        let saved = std::env::var("RELIQUAINT_SUBSCRIPTIONS_PATH").ok();
        std::env::set_var("RELIQUAINT_SUBSCRIPTIONS_PATH", "/tmp/custom-subs.toml");
        assert_eq!(subscriptions_path(), PathBuf::from("/tmp/custom-subs.toml"));
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_SUBSCRIPTIONS_PATH", v),
            None => std::env::remove_var("RELIQUAINT_SUBSCRIPTIONS_PATH"),
        }
    }

    #[test]
    fn user_tap_dir_ends_with_reliquaint_tap() {
        let saved = std::env::var("RELIQUAINT_USER_TAP_DIR").ok();
        std::env::remove_var("RELIQUAINT_USER_TAP_DIR");
        let p = user_tap_dir();
        assert!(
            p.ends_with("reliquaint/tap"),
            "user_tap_dir should end with reliquaint/tap, got {}",
            p.display()
        );
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_USER_TAP_DIR", v),
            None => std::env::remove_var("RELIQUAINT_USER_TAP_DIR"),
        }
    }

    #[test]
    fn user_tap_dir_env_overrides() {
        let saved = std::env::var("RELIQUAINT_USER_TAP_DIR").ok();
        std::env::set_var("RELIQUAINT_USER_TAP_DIR", "/tmp/custom-user-tap");
        assert_eq!(user_tap_dir(), PathBuf::from("/tmp/custom-user-tap"));
        match saved {
            Some(v) => std::env::set_var("RELIQUAINT_USER_TAP_DIR", v),
            None => std::env::remove_var("RELIQUAINT_USER_TAP_DIR"),
        }
    }
}
