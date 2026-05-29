//! DOS entry-point detection. Given a directory the platform classifier
//! has identified as DOS, produce a ranked list of likely entry points
//! (the program the launcher should `RUN` after mounting `C:`).
//!
//! Rules per v0.2-tasks.md §2.2:
//! - Prefer `.BAT` over `.EXE` (BAT files are usually wrappers).
//! - Exclude `INSTALL.*` and `SETUP.*` — those are setup scripts, not
//!   entry points.
//! - Ranking:
//!     1. A `.BAT` whose stem matches the directory name.
//!     2. A `.BAT` that invokes a `.EXE` present in the same directory.
//!     3. An `.EXE` whose stem matches the directory name.
//!     4. The largest `.EXE` as fallback.
//! - Returns a `Vec<EntryPointCandidate>` so the wizard can prompt when
//!   ranking is ambiguous.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPointCandidate {
    /// File name as it sits on disk (e.g., `SIERRA.BAT`). Case preserved
    /// from the filesystem.
    pub file_name: String,
    pub kind: EntryPointKind,
    /// Why the candidate ranks where it does — surfaced in the wizard
    /// disambiguation prompt.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryPointKind {
    Bat,
    Exe,
    Com,
}

const EXCLUDED_STEMS: &[&str] = &["INSTALL", "SETUP", "UNINSTAL", "UNINSTALL"];

/// Walk `dir` and return entry-point candidates in descending rank
/// order. An empty Vec means no plausible entry point was found, in
/// which case the wizard prompts for manual entry.
pub fn detect(dir: &Path) -> Vec<EntryPointCandidate> {
    let entries = super::list_dir(dir);
    let dir_stem = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(stem_canonical)
        .unwrap_or_default();

    // Categorize files first.
    let mut bats: Vec<String> = Vec::new();
    let mut exes: Vec<(String, u64)> = Vec::new();
    let mut coms: Vec<String> = Vec::new();

    for (name, is_file) in &entries {
        if !is_file {
            continue;
        }
        let Some((stem, ext)) = name.rsplit_once('.') else {
            continue;
        };
        let stem_upper = stem.to_ascii_uppercase();
        if EXCLUDED_STEMS.contains(&stem_upper.as_str()) {
            continue;
        }
        match ext.to_ascii_uppercase().as_str() {
            "BAT" => bats.push(name.clone()),
            "EXE" => {
                let size = std::fs::metadata(dir.join(name))
                    .map(|m| m.len())
                    .unwrap_or(0);
                exes.push((name.clone(), size));
            }
            "COM" => coms.push(name.clone()),
            _ => {}
        }
    }

    let mut candidates: Vec<EntryPointCandidate> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Tier 1: a BAT whose stem matches the directory name.
    for bat in &bats {
        let stem = file_stem_canonical(bat);
        if !dir_stem.is_empty() && stem == dir_stem && seen_names.insert(bat.clone()) {
            candidates.push(EntryPointCandidate {
                file_name: bat.clone(),
                kind: EntryPointKind::Bat,
                reason: "BAT script matching the directory name".to_string(),
            });
        }
    }

    // Tier 2: a BAT that references an EXE present in the same dir.
    let exe_names_upper: Vec<String> = exes.iter().map(|(n, _)| n.to_ascii_uppercase()).collect();
    for bat in &bats {
        if seen_names.contains(bat) {
            continue;
        }
        if let Some(referenced) = bat_references_exe(&dir.join(bat), &exe_names_upper) {
            if seen_names.insert(bat.clone()) {
                candidates.push(EntryPointCandidate {
                    file_name: bat.clone(),
                    kind: EntryPointKind::Bat,
                    reason: format!("BAT script invoking {referenced}"),
                });
            }
        }
    }

    // Tier 3: an EXE whose stem matches the directory name.
    for (name, _size) in &exes {
        let stem = file_stem_canonical(name);
        if !dir_stem.is_empty() && stem == dir_stem && seen_names.insert(name.clone()) {
            candidates.push(EntryPointCandidate {
                file_name: name.clone(),
                kind: EntryPointKind::Exe,
                reason: "EXE matching the directory name".to_string(),
            });
        }
    }

    // Tier 4: any remaining BATs (lower-confidence wrappers).
    for bat in &bats {
        if seen_names.insert(bat.clone()) {
            candidates.push(EntryPointCandidate {
                file_name: bat.clone(),
                kind: EntryPointKind::Bat,
                reason: "BAT script (fallback)".to_string(),
            });
        }
    }

    // Tier 5: largest EXE first.
    let mut remaining_exes: Vec<&(String, u64)> = exes
        .iter()
        .filter(|(n, _)| !seen_names.contains(n))
        .collect();
    remaining_exes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (name, size) in remaining_exes {
        if seen_names.insert(name.clone()) {
            candidates.push(EntryPointCandidate {
                file_name: name.clone(),
                kind: EntryPointKind::Exe,
                reason: format!("EXE ({} bytes)", size),
            });
        }
    }

    // Tier 6: COM files as a last resort.
    for com in &coms {
        if seen_names.insert(com.clone()) {
            candidates.push(EntryPointCandidate {
                file_name: com.clone(),
                kind: EntryPointKind::Com,
                reason: "COM executable".to_string(),
            });
        }
    }

    candidates
}

fn file_stem_canonical(name: &str) -> String {
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    stem_canonical(stem)
}

/// Canonical form for comparing a directory or file stem against
/// another stem: lowercased, with non-alphanumerics dropped. So
/// `qfg1-ega`, `QFG1_EGA`, and `qfg1ega` all collapse to `qfg1ega`.
fn stem_canonical(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Read the BAT file and look for any of `exe_names_upper` mentioned by
/// name. Returns the first matching EXE name (uppercase form) found, or
/// `None`.
fn bat_references_exe(bat_path: &Path, exe_names_upper: &[String]) -> Option<String> {
    let body = std::fs::read_to_string(bat_path).ok()?;
    let body_upper = body.to_ascii_uppercase();
    for exe in exe_names_upper {
        // Match the EXE name as a substring. This is loose, but BAT
        // scripts vary so widely (cd C:\GAMES & QFG1.EXE) that any
        // tighter regex risks false negatives. The user can override.
        if body_upper.contains(exe) {
            return Some(exe.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "").unwrap();
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn excludes_install_and_setup_scripts() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("INSTALL.BAT"));
        touch(&tmp.path().join("SETUP.EXE"));
        touch(&tmp.path().join("UNINSTAL.EXE"));
        let candidates = detect(tmp.path());
        assert!(
            candidates.is_empty(),
            "setup scripts should not appear: {candidates:?}"
        );
    }

    #[test]
    fn name_matching_bat_ranks_first() {
        let dir = tempdir().unwrap();
        // Rename the tempdir so its stem is predictable.
        let qfg1 = dir.path().join("qfg1-ega");
        std::fs::create_dir_all(&qfg1).unwrap();
        touch(&qfg1.join("QFG1-EGA.BAT"));
        touch(&qfg1.join("SIERRA.EXE"));

        let candidates = detect(&qfg1);
        assert_eq!(candidates[0].file_name, "QFG1-EGA.BAT");
        assert!(candidates[0].reason.contains("matching the directory name"));
    }

    #[test]
    fn bat_invoking_exe_ranks_higher_than_unrelated_bat() {
        let dir = tempdir().unwrap();
        let game = dir.path().join("game");
        std::fs::create_dir_all(&game).unwrap();
        write(&game.join("RUN.BAT"), "@echo off\nQFG1.EXE\n");
        write(&game.join("MISC.BAT"), "@echo off\n");
        touch(&game.join("QFG1.EXE"));

        let candidates = detect(&game);
        let ranked: Vec<&str> = candidates.iter().map(|c| c.file_name.as_str()).collect();
        let idx_run = ranked.iter().position(|n| *n == "RUN.BAT").unwrap();
        let idx_misc = ranked.iter().position(|n| *n == "MISC.BAT").unwrap();
        assert!(
            idx_run < idx_misc,
            "RUN.BAT (invokes QFG1.EXE) should outrank MISC.BAT: {candidates:?}"
        );
    }

    #[test]
    fn largest_exe_is_fallback_when_no_bat() {
        let dir = tempdir().unwrap();
        let game = dir.path().join("game");
        std::fs::create_dir_all(&game).unwrap();
        write(&game.join("SMALL.EXE"), "small");
        write(&game.join("BIG.EXE"), &"x".repeat(10_000));

        let candidates = detect(&game);
        assert_eq!(candidates[0].file_name, "BIG.EXE");
    }

    #[test]
    fn no_executables_returns_empty() {
        let dir = tempdir().unwrap();
        touch(&dir.path().join("readme.txt"));
        let candidates = detect(dir.path());
        assert!(candidates.is_empty());
    }
}
