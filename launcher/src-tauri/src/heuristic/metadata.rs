//! Derive a starting title and id from the directory name.
//!
//! README and manual auto-parsing are **deliberately out of scope** for
//! v0.2 — they're noisy, often wrong, and the user reviews the manifest
//! anyway. A wrong auto-guess is worse than an empty field. Per
//! v0.2-tasks.md §2.4, keep this comment so the next implementer doesn't
//! undo it.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftMetadata {
    /// Slug-form catalog id derived from the directory basename. May be
    /// empty if the directory name doesn't yield a valid id, in which
    /// case the wizard prompts the user for one.
    pub id: String,
    /// Title-cased title derived from the directory basename.
    pub title: String,
}

/// Derive title + id from `dir`'s file name only. Returns empty strings
/// when the directory has no usable basename.
pub fn extract(dir: &Path) -> DraftMetadata {
    let Some(basename) = dir.file_name().and_then(|n| n.to_str()) else {
        return DraftMetadata {
            id: String::new(),
            title: String::new(),
        };
    };
    DraftMetadata {
        id: slug(basename),
        title: titlecase(basename),
    }
}

/// Title-case `basename`: split on `-` and `_`, capitalize the first
/// letter of each word, join with spaces.
fn titlecase(basename: &str) -> String {
    let mut out = String::with_capacity(basename.len());
    let mut at_word_boundary = true;
    for c in basename.chars() {
        if c == '-' || c == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            at_word_boundary = true;
            continue;
        }
        if at_word_boundary {
            for u in c.to_uppercase() {
                out.push(u);
            }
            at_word_boundary = false;
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// Slug-form id derived from `basename`:
///
/// 1. Lowercase.
/// 2. Replace `_` with `-`.
/// 3. Drop any character that isn't `[a-z0-9-]`.
/// 4. Collapse consecutive `-` and trim leading/trailing `-`.
/// 5. If the result doesn't start with `[a-z]`, return empty (the
///    wizard will prompt for a manual id).
fn slug(basename: &str) -> String {
    let mut buf = String::with_capacity(basename.len());
    for c in basename.chars() {
        let mapped = if c == '_' { '-' } else { c };
        for lower in mapped.to_lowercase() {
            if lower.is_ascii_alphanumeric() || lower == '-' {
                buf.push(lower);
            }
        }
    }
    // Collapse consecutive `-`.
    let mut collapsed = String::with_capacity(buf.len());
    let mut prev_dash = false;
    for c in buf.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push(c);
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    let trimmed = collapsed.trim_matches('-').to_string();
    if trimmed.is_empty() {
        return String::new();
    }
    if !trimmed
        .chars()
        .next()
        .map(|c| c.is_ascii_lowercase())
        .unwrap_or(false)
    {
        return String::new();
    }
    // If the canonical validator would reject it, the wizard should
    // ask. Bounds (2..=64) are enforced by `is_valid_id`.
    if !crate::catalog::is_valid_id(&trimmed) {
        return String::new();
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn leisure_suit_larry_yields_canonical_id() {
        let m = extract(&PathBuf::from("/home/u/games/leisure-suit-larry"));
        assert_eq!(m.id, "leisure-suit-larry");
        assert_eq!(m.title, "Leisure Suit Larry");
    }

    #[test]
    fn underscores_become_hyphens_in_id_and_spaces_in_title() {
        let m = extract(&PathBuf::from("/home/u/games/Quest_For_Glory_1"));
        assert_eq!(m.id, "quest-for-glory-1");
        assert_eq!(m.title, "Quest For Glory 1");
    }

    #[test]
    fn mixed_case_basename_lowercases_id_only() {
        let m = extract(&PathBuf::from("/home/u/games/QFG1-EGA"));
        assert_eq!(m.id, "qfg1-ega");
        assert_eq!(m.title, "QFG1 EGA");
    }

    #[test]
    fn punctuation_dropped_from_id() {
        let m = extract(&PathBuf::from("/home/u/games/Doom!"));
        assert_eq!(m.id, "doom");
        assert_eq!(m.title, "Doom!");
    }

    #[test]
    fn id_starting_with_digit_is_empty() {
        let m = extract(&PathBuf::from("/home/u/games/2-pak"));
        assert_eq!(m.id, "", "ids must start with [a-z]; wizard prompts");
        assert_eq!(m.title, "2 Pak");
    }

    #[test]
    fn very_short_basename_rejected() {
        let m = extract(&PathBuf::from("/home/u/games/a"));
        assert_eq!(m.id, "");
    }

    #[test]
    fn consecutive_separators_collapse() {
        let m = extract(&PathBuf::from("/home/u/games/foo--bar__baz"));
        assert_eq!(m.id, "foo-bar-baz");
    }
}
