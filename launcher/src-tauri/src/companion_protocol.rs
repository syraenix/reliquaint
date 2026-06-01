//! Tap-local companion image resolution for the `reliquaint-content://`
//! protocol (v0.4 Milestone 2, Task 2.3).
//!
//! The webview references companion images as
//! `reliquaint-content://<tap-id>/<game-id>/<relative-path>`. The Tauri
//! protocol handler (registered in `gui.rs`) parses that URL and calls
//! [`resolve_image`], which serves a file **only** from within the requested
//! game's companion directory.
//!
//! Two defenses, per ADR-0006:
//! - **Path-traversal / symlink-escape protection** — the resolved, symlink-
//!   followed path must stay within `<tap-root>/companion/<game-id>/`.
//! - **Format check** — the extension must be one of PNG/JPEG/GIF/WebP *and*
//!   the file's magic bytes must match it (SVG and disguised files are
//!   rejected).
//!
//! The disk-facing logic lives in [`resolve_image_in`], which takes the base
//! directory explicitly so it is unit-testable without touching XDG paths.

use std::path::{Component, Path, PathBuf};

/// Why a companion image request was refused. The protocol handler maps these
/// to HTTP-equivalent status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// The file does not exist.
    NotFound,
    /// The request resolved outside the game's companion directory (traversal,
    /// absolute path, or symlink escape).
    OutsideBoundary,
    /// The extension is not an allowed image type, or the bytes don't match it.
    BadFormat,
    /// An unexpected I/O error reading the file.
    Io,
}

/// The four image formats allowed for companion content (SVG excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Format {
    fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// Do the leading bytes match this format's signature?
    fn matches_magic(self, b: &[u8]) -> bool {
        match self {
            Self::Png => b.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
            Self::Jpeg => b.starts_with(&[0xFF, 0xD8, 0xFF]),
            Self::Gif => b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"),
            Self::Webp => b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP",
        }
    }
}

/// Resolve and read a companion image for `(tap_id, game_id)`.
///
/// Returns the file bytes plus the MIME type on success. The base directory is
/// `paths::companion_dir(tap_id, game_id)`.
pub fn resolve_image(
    tap_id: &str,
    game_id: &str,
    rel_path: &str,
) -> Result<(Vec<u8>, &'static str), ImageError> {
    let base = crate::paths::companion_dir(tap_id, game_id);
    resolve_image_in(&base, rel_path)
}

/// Resolve `rel_path` to a real file inside `base`, enforcing the boundary:
/// the requested path may contain only plain segments (no `..`, no absolute
/// root), and after symlink resolution the file must still live within `base`.
/// Returns the canonical path on success.
///
/// Shared by image serving and Markdown reading so both honor identical
/// traversal/symlink-escape protection.
pub fn resolve_within(base: &Path, rel_path: &str) -> Result<PathBuf, ImageError> {
    let rel = Path::new(rel_path);

    // Reject traversal in the *requested* path before touching disk: only
    // plain path segments (and `.`) are allowed — no `..`, no absolute root.
    for component in rel.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                tracing::warn!(
                    base = %base.display(),
                    rel = %rel_path,
                    "rejected companion request with traversal/absolute path"
                );
                return Err(ImageError::OutsideBoundary);
            }
        }
    }

    // Canonicalize (following symlinks) and require containment within the
    // canonical base — defeats symlinks that point outside the boundary.
    let canon = std::fs::canonicalize(base.join(rel)).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ImageError::NotFound,
        _ => ImageError::Io,
    })?;
    let canon_base = std::fs::canonicalize(base).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ImageError::NotFound,
        _ => ImageError::Io,
    })?;
    if !canon.starts_with(&canon_base) {
        tracing::warn!(
            base = %base.display(),
            rel = %rel_path,
            resolved = %canon.display(),
            "companion request escaped its boundary"
        );
        return Err(ImageError::OutsideBoundary);
    }

    Ok(canon)
}

/// Disk-facing core of [`resolve_image`], with the companion base directory
/// passed explicitly so it can be tested in isolation.
pub fn resolve_image_in(
    base: &Path,
    rel_path: &str,
) -> Result<(Vec<u8>, &'static str), ImageError> {
    // Span for profiling image serving (ADR-0004); zero-cost without a
    // subscriber. `RUST_LOG=reliquaint=debug` surfaces timings.
    let _span = tracing::info_span!("companion_image", rel = rel_path).entered();

    // The extension must name an allowed image format (rejects `.svg`, etc.).
    let format = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Format::from_ext)
        .ok_or(ImageError::BadFormat)?;

    let canon = resolve_within(base, rel_path)?;

    // Read the file and require its magic bytes to match the extension.
    let bytes = std::fs::read(&canon).map_err(|_| ImageError::Io)?;
    if !format.matches_magic(&bytes) {
        tracing::warn!(
            base = %base.display(),
            rel = %rel_path,
            "companion image magic bytes do not match its extension"
        );
        return Err(ImageError::BadFormat);
    }

    Ok((bytes, format.mime()))
}

/// Read a companion Markdown document for `(tap_id, game_id)`. The path must
/// have a `.md` extension and resolve within the game's companion directory
/// (same boundary protection as image serving). Returns the file's text.
pub fn read_markdown(tap_id: &str, game_id: &str, rel_path: &str) -> Result<String, ImageError> {
    let base = crate::paths::companion_dir(tap_id, game_id);
    read_markdown_in(&base, rel_path)
}

/// Disk-facing core of [`read_markdown`], with the companion base directory
/// passed explicitly so it can be tested without touching XDG paths.
pub fn read_markdown_in(base: &Path, rel_path: &str) -> Result<String, ImageError> {
    let is_md = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_md {
        return Err(ImageError::BadFormat);
    }
    let canon = resolve_within(base, rel_path)?;
    std::fs::read_to_string(&canon).map_err(|_| ImageError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

    fn write(base: &Path, rel: &str, bytes: &[u8]) {
        let path = base.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn serves_legitimate_image_with_mime() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "maps/spielburg.png", PNG_MAGIC);

        let (bytes, mime) = resolve_image_in(dir.path(), "maps/spielburg.png").unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, PNG_MAGIC);
    }

    #[test]
    fn serves_root_level_image() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "cover.jpg", JPEG_MAGIC);
        let (_, mime) = resolve_image_in(dir.path(), "cover.jpg").unwrap();
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "ok.png", PNG_MAGIC);
        assert_eq!(
            resolve_image_in(dir.path(), "../../etc/passwd.png"),
            Err(ImageError::OutsideBoundary)
        );
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_image_in(dir.path(), "/etc/passwd.png"),
            Err(ImageError::OutsideBoundary)
        );
    }

    #[test]
    fn rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.png", PNG_MAGIC);
        std::fs::create_dir_all(dir.path()).unwrap();
        // A symlink inside the companion dir pointing at a file outside it.
        std::os::unix::fs::symlink(
            outside.path().join("secret.png"),
            dir.path().join("link.png"),
        )
        .unwrap();

        assert_eq!(
            resolve_image_in(dir.path(), "link.png"),
            Err(ImageError::OutsideBoundary)
        );
    }

    #[test]
    fn rejects_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "diagram.svg", b"<svg/>");
        assert_eq!(
            resolve_image_in(dir.path(), "diagram.svg"),
            Err(ImageError::BadFormat)
        );
    }

    #[test]
    fn rejects_magic_byte_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        // A .png file actually carrying JPEG bytes.
        write(dir.path(), "fake.png", JPEG_MAGIC);
        assert_eq!(
            resolve_image_in(dir.path(), "fake.png"),
            Err(ImageError::BadFormat)
        );
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_image_in(dir.path(), "nope.png"),
            Err(ImageError::NotFound)
        );
    }

    // ---- resolve_within / read_markdown ----------------------------------

    #[test]
    fn resolve_within_accepts_contained_path_and_rejects_escape() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "sub/guide.md", b"# hi");
        assert!(resolve_within(dir.path(), "sub/guide.md").is_ok());
        assert_eq!(
            resolve_within(dir.path(), "../escape.md"),
            Err(ImageError::OutsideBoundary)
        );
        assert_eq!(
            resolve_within(dir.path(), "/etc/passwd"),
            Err(ImageError::OutsideBoundary)
        );
    }

    #[test]
    fn read_markdown_reads_contained_md() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "01-overview.md", b"# Overview\n");
        assert_eq!(
            read_markdown_in(dir.path(), "01-overview.md").as_deref(),
            Ok("# Overview\n")
        );
    }

    #[test]
    fn read_markdown_rejects_non_md_extension() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "maps/spielburg.png", PNG_MAGIC);
        assert_eq!(
            read_markdown_in(dir.path(), "maps/spielburg.png"),
            Err(ImageError::BadFormat)
        );
    }

    #[test]
    fn read_markdown_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_markdown_in(dir.path(), "../escape.md"),
            Err(ImageError::OutsideBoundary)
        );
    }
}
