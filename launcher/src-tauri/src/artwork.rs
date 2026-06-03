//! Resolving a single display icon for a game.
//!
//! Two sources, in precedence order (see the library-rework design doc,
//! `docs/superpowers/specs/2026-05-21-library-rework-design.md`):
//!
//! 1. **Install directory** — auto-detected from `~/games/<id>/` for an
//!    installed game ([`detect_in_dir`]).
//! 2. **Tap-provided art** — the optional `[meta] artwork` path shipped by the
//!    tap, resolved within the tap root ([`resolve_tap_art`]).
//!
//! Both sources live under `$HOME`, so the resolved absolute path is served to
//! the webview via Tauri's asset protocol (`convertFileSrc`) — no custom URI
//! scheme is needed.

use std::path::{Path, PathBuf};

/// Named artwork files checked first, in priority order, when scanning a game's
/// install directory.
const NAMED: [&str; 6] = [
    "cover.png",
    "cover.jpg",
    "icon.png",
    "icon.jpg",
    "box.png",
    "box.jpg",
];

/// Does this path name an image we'll treat as loose cover art?
fn is_loose_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "bmp")
    )
}

/// Find a display image inside an installed game's directory.
///
/// Priority: a named file (`cover.png` → `cover.jpg` → `icon.png` → `icon.jpg`
/// → `box.png` → `box.jpg`), else the alphabetically-first loose
/// `.png`/`.jpg`/`.bmp` in the directory root. Non-recursive. Returns the
/// absolute path of the first match, or `None`.
pub fn detect_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in NAMED {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let mut loose: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_loose_image(p))
        .collect();
    loose.sort();
    loose.into_iter().next()
}

/// Resolve a tap-relative artwork path within `base`, enforcing containment
/// (no `..`, no absolute path, no symlink escape) by reusing the companion
/// protocol's boundary check. Returns the canonical path if it names an
/// existing file, else `None`.
pub fn resolve_art_in(base: &Path, rel_path: &str) -> Option<PathBuf> {
    let canon = crate::companion_protocol::resolve_within(base, rel_path).ok()?;
    canon.is_file().then_some(canon)
}

/// Resolve the tap-provided `[meta] artwork` path for `tap_id` against that
/// tap's on-disk root. Thin wrapper over [`resolve_art_in`].
pub fn resolve_tap_art(tap_id: &str, rel_path: &str) -> Option<PathBuf> {
    resolve_art_in(&crate::paths::tap_root_dir(tap_id), rel_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn detect_prefers_named_cover_over_loose_and_box() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "box.png", PNG);
        write(dir.path(), "screenshot.png", PNG);
        write(dir.path(), "cover.png", PNG);
        assert_eq!(
            detect_in_dir(dir.path()),
            Some(dir.path().join("cover.png"))
        );
    }

    #[test]
    fn detect_named_priority_icon_before_box() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "box.png", PNG);
        write(dir.path(), "icon.png", PNG);
        assert_eq!(detect_in_dir(dir.path()), Some(dir.path().join("icon.png")));
    }

    #[test]
    fn detect_falls_back_to_lone_loose_image() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "RESOURCE.001", b"not an image");
        write(dir.path(), "screenshot.jpg", PNG);
        assert_eq!(
            detect_in_dir(dir.path()),
            Some(dir.path().join("screenshot.jpg"))
        );
    }

    #[test]
    fn detect_loose_fallback_is_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "b.png", PNG);
        write(dir.path(), "a.png", PNG);
        assert_eq!(detect_in_dir(dir.path()), Some(dir.path().join("a.png")));
    }

    #[test]
    fn detect_returns_none_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_in_dir(dir.path()), None);
    }

    #[test]
    fn detect_returns_none_for_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_in_dir(&dir.path().join("nope")), None);
    }

    #[test]
    fn resolve_art_in_returns_contained_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "art/cover.png", PNG);
        let got = resolve_art_in(dir.path(), "art/cover.png");
        assert!(got.is_some());
        assert!(got.unwrap().ends_with("art/cover.png"));
    }

    #[test]
    fn resolve_art_in_rejects_traversal_and_absolute() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "ok.png", PNG);
        assert_eq!(resolve_art_in(dir.path(), "../escape.png"), None);
        assert_eq!(resolve_art_in(dir.path(), "/etc/passwd"), None);
    }

    #[test]
    fn resolve_art_in_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve_art_in(dir.path(), "nope.png"), None);
    }
}
