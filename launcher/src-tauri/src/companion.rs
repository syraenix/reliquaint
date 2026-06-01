//! Companion content discovery and indexing (v0.4).
//!
//! Taps carry per-game companion content — walkthroughs, maps, hints,
//! install notes — under `<tap-root>/companion/<game-id>/`. See
//! `docs/v0.4-tasks.md` Milestone 1 and `docs/adr-0006-companion-content-rendering.md`.
//!
//! This module is pure discovery: it walks the on-disk layout and builds an
//! in-memory index. Rendering (Markdown → sanitized HTML) and image serving
//! live elsewhere (`render`, the `reliquaint-content://` protocol).
//!
//! Conventions (Task 1.1):
//! - Recognized files: `.md` (Markdown) and the image set
//!   `.png/.jpg/.jpeg/.gif/.webp`. **SVG is excluded** per ADR-0006.
//! - Numeric-prefixed filenames sort first by their numeric value;
//!   non-prefixed names sort alphabetically after them.
//! - Title derives from the filename: numeric prefix stripped,
//!   hyphens/underscores become spaces, words capitalized.
//! - One level of subdirectory (e.g. `hints/`, `maps/`) is a first-class
//!   section grouping; the subdirectory name is the section label.

use std::path::Path;

/// What kind of companion file this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionKind {
    Markdown,
    Image,
}

/// A single piece of companion content discovered within a tap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionItem {
    /// Id of the tap this content came from (for attribution).
    pub tap_id: String,
    /// Game id this content belongs to.
    pub game_id: String,
    /// Path relative to `companion/<game-id>/`, forward-slashed
    /// (e.g. `01-overview.md` or `maps/spielburg.png`).
    pub rel_path: String,
    /// Human-readable title derived from the filename.
    pub title: String,
    pub kind: CompanionKind,
    /// Subdirectory grouping, or `None` for files at the game-dir root.
    pub section: Option<String>,
}

/// Recognized image extensions (lowercase, no dot). SVG is deliberately absent.
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Classify a filename by extension. Returns `None` for unrecognized
/// extensions (including `.svg`, which is excluded per ADR-0006).
pub fn classify(filename: &str) -> Option<CompanionKind> {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())?;
    if ext == "md" {
        Some(CompanionKind::Markdown)
    } else if IMAGE_EXTS.contains(&ext.as_str()) {
        Some(CompanionKind::Image)
    } else {
        None
    }
}

/// Derive a display title from a filename: drop the extension, strip any
/// leading numeric prefix (`NN-` / `NN_`), turn `-`/`_` into spaces, and
/// capitalize each word.
///
/// `01-walkthrough.md` → "Walkthrough"; `boss-fight-tips.md` → "Boss Fight Tips".
pub fn derive_title(filename: &str) -> String {
    // Drop extension.
    let stem = filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(filename);
    // Strip a leading numeric prefix followed by a separator.
    let without_prefix = strip_numeric_prefix(stem).0;
    without_prefix
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Split a leading run of digits + a single `-`/`_` separator off a stem.
/// Returns `(remainder, Some(numeric_value))` when a prefix is present,
/// otherwise `(stem, None)`.
fn strip_numeric_prefix(stem: &str) -> (&str, Option<u32>) {
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return (stem, None);
    }
    let rest = &stem[digits.len()..];
    match rest.strip_prefix(['-', '_']) {
        Some(after) => (after, digits.parse().ok()),
        // Digits not followed by a separator → not a sort/title prefix.
        None => (stem, None),
    }
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Sort key for a filename within a directory: numeric-prefixed files first
/// (ordered by numeric value), then non-prefixed files alphabetically.
fn file_sort_key(filename: &str) -> (bool, u32, String) {
    let (_, prefix) = strip_numeric_prefix(filename);
    match prefix {
        Some(n) => (false, n, filename.to_ascii_lowercase()),
        None => (true, 0, filename.to_ascii_lowercase()),
    }
}

/// Walk `<tap_root>/companion/` and build the companion index for every game
/// directory found. Each `CompanionItem` is attributed to `tap_id`.
///
/// Errors reading individual files or directories are logged at WARN and the
/// offending entry is skipped — a single bad file never fails the indexer.
pub fn index_companion(tap_root: &Path, tap_id: &str) -> Vec<CompanionItem> {
    // Span for profiling companion discovery (ADR-0004); zero-cost without a
    // subscriber. `RUST_LOG=reliquaint=debug` surfaces timings.
    let _span = tracing::info_span!("companion_index", tap_id).entered();

    let companion_root = tap_root.join("companion");
    let game_dirs = match std::fs::read_dir(&companion_root) {
        Ok(rd) => rd,
        // No companion/ directory is the common case, not an error.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(
                tap_id = %tap_id,
                dir = %companion_root.display(),
                error = %e,
                "could not read companion directory; skipping"
            );
            return Vec::new();
        }
    };

    let mut items = Vec::new();
    for game_entry in game_dirs.flatten() {
        let game_path = game_entry.path();
        if !game_path.is_dir() {
            continue;
        }
        let game_id = game_entry.file_name().to_string_lossy().into_owned();
        index_game_dir(&game_path, tap_id, &game_id, &mut items);
    }
    tracing::debug!(items = items.len(), "indexed companion content");
    items
}

/// Index one `companion/<game-id>/` directory: root files (section `None`)
/// plus one level of subdirectories (each a named section). Items are
/// appended in display order — root first, then sections alphabetically,
/// each group ordered by [`file_sort_key`].
fn index_game_dir(game_path: &Path, tap_id: &str, game_id: &str, out: &mut Vec<CompanionItem>) {
    // Root-level files.
    let mut root_files = collect_files(game_path, tap_id, game_id);
    root_files.sort_by_key(|a| file_sort_key(&a.0));

    // Immediate subdirectories, each a section, sorted by name.
    let mut sections: Vec<String> = match std::fs::read_dir(game_path) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(e) => {
            tracing::warn!(
                tap_id = %tap_id,
                game_id = %game_id,
                dir = %game_path.display(),
                error = %e,
                "could not read companion game directory; skipping"
            );
            Vec::new()
        }
    };
    sections.sort();

    for (filename, kind) in root_files {
        out.push(CompanionItem {
            tap_id: tap_id.to_string(),
            game_id: game_id.to_string(),
            title: derive_title(&filename),
            rel_path: filename,
            kind,
            section: None,
        });
    }

    for section in sections {
        let section_path = game_path.join(&section);
        let mut files = collect_files(&section_path, tap_id, game_id);
        files.sort_by_key(|a| file_sort_key(&a.0));
        for (filename, kind) in files {
            out.push(CompanionItem {
                tap_id: tap_id.to_string(),
                game_id: game_id.to_string(),
                title: derive_title(&filename),
                rel_path: format!("{section}/{filename}"),
                kind,
                section: Some(section.clone()),
            });
        }
    }
}

/// Collect recognized files (filename + kind) directly inside `dir`,
/// skipping subdirectories and unrecognized extensions.
fn collect_files(dir: &Path, tap_id: &str, game_id: &str) -> Vec<(String, CompanionKind)> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!(
                tap_id = %tap_id,
                game_id = %game_id,
                dir = %dir.display(),
                error = %e,
                "could not read companion directory; skipping"
            );
            return Vec::new();
        }
    };
    let mut files = Vec::new();
    for entry in rd.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(kind) = classify(&name) {
            files.push((name, kind));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_recognizes_markdown_and_images() {
        assert_eq!(classify("walkthrough.md"), Some(CompanionKind::Markdown));
        assert_eq!(classify("map.png"), Some(CompanionKind::Image));
        assert_eq!(classify("shot.JPG"), Some(CompanionKind::Image));
        assert_eq!(classify("shot.jpeg"), Some(CompanionKind::Image));
        assert_eq!(classify("anim.gif"), Some(CompanionKind::Image));
        assert_eq!(classify("pic.webp"), Some(CompanionKind::Image));
    }

    #[test]
    fn classify_excludes_svg_and_unknown() {
        assert_eq!(classify("diagram.svg"), None);
        assert_eq!(classify("notes.txt"), None);
        assert_eq!(classify("README"), None);
    }

    #[test]
    fn derive_title_strips_numeric_prefix() {
        assert_eq!(derive_title("01-walkthrough.md"), "Walkthrough");
        assert_eq!(derive_title("02-mid-game.md"), "Mid Game");
    }

    #[test]
    fn derive_title_handles_no_prefix() {
        assert_eq!(derive_title("boss-fight-tips.md"), "Boss Fight Tips");
        assert_eq!(derive_title("overview.md"), "Overview");
    }

    #[test]
    fn derive_title_treats_underscores_as_spaces() {
        assert_eq!(derive_title("03_secret_areas.md"), "Secret Areas");
    }

    #[test]
    fn numeric_prefix_requires_separator() {
        // "1992" with no separator is not a sort/title prefix.
        assert_eq!(derive_title("1992.md"), "1992");
    }

    #[test]
    fn index_orders_numeric_before_alpha_and_groups_sections() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("companion/qfg1-ega");
        std::fs::create_dir_all(game.join("maps")).unwrap();
        // Root files: deliberately created out of order.
        std::fs::write(game.join("02-walkthrough.md"), "# wt").unwrap();
        std::fs::write(game.join("01-overview.md"), "# ov").unwrap();
        std::fs::write(game.join("appendix.md"), "# ap").unwrap();
        // A section subdirectory.
        std::fs::write(game.join("maps/spielburg.png"), b"\x89PNG").unwrap();

        let items = index_companion(dir.path(), "core");

        // All four are indexed.
        assert_eq!(items.len(), 4, "items: {items:?}");

        // Root items come first, ordered: numeric (01, 02) then alpha.
        let root: Vec<&CompanionItem> = items.iter().filter(|i| i.section.is_none()).collect();
        let root_titles: Vec<&str> = root.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(root_titles, vec!["Overview", "Walkthrough", "Appendix"]);

        // The map is grouped under the "maps" section.
        let map = items
            .iter()
            .find(|i| i.rel_path == "maps/spielburg.png")
            .expect("map should be indexed");
        assert_eq!(map.section.as_deref(), Some("maps"));
        assert_eq!(map.kind, CompanionKind::Image);
        assert_eq!(map.tap_id, "core");
        assert_eq!(map.game_id, "qfg1-ega");
    }

    #[test]
    fn index_skips_svg_and_unrecognized_files() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("companion/kq5");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("guide.md"), "# g").unwrap();
        std::fs::write(game.join("diagram.svg"), "<svg/>").unwrap();
        std::fs::write(game.join("notes.txt"), "x").unwrap();

        let items = index_companion(dir.path(), "core");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rel_path, "guide.md");
    }

    #[test]
    fn index_returns_empty_when_no_companion_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(index_companion(dir.path(), "core").is_empty());
    }
}
