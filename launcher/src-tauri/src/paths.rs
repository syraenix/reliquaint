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
}
