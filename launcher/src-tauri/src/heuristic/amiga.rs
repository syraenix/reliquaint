//! Amiga entry-point detection. v0.2 scope is **floppy-loadable games
//! only** (FS-UAE `--floppy_image_N` swap list). WHDLoad and
//! HD-installed games fall outside this scope; the wizard surfaces a
//! `ManualEntry` state for those, so the user can hand-edit the
//! resulting manifest or skip the directory entirely.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmigaEntryPoints {
    /// Filenames of `.ADF` floppy images in mount order. The Vec is
    /// alphabetically sorted, which matches the natural "Disk 1, Disk
    /// 2, …" ordering for the common case.
    Floppies(Vec<String>),
    /// The directory does not contain a floppy-loadable Amiga game.
    /// `reason` explains what was found instead so the wizard can show
    /// the user.
    ManualEntry { reason: String },
}

/// Walk `dir` and either collect `.ADF` files or return the
/// manual-entry state for HD/WHDLoad layouts.
pub fn detect(dir: &Path) -> AmigaEntryPoints {
    let entries = super::list_dir(dir);

    let mut adfs: Vec<String> = Vec::new();
    let mut whdload = false;
    let mut hdf = false;
    for (name, is_file) in &entries {
        if !is_file {
            continue;
        }
        let upper = name.to_ascii_uppercase();
        if upper.ends_with(".ADF") {
            adfs.push(name.clone());
        }
        if upper.ends_with(".SLAVE") {
            whdload = true;
        }
        if upper.ends_with(".HDF") {
            hdf = true;
        }
    }

    if !adfs.is_empty() {
        adfs.sort();
        return AmigaEntryPoints::Floppies(adfs);
    }

    if whdload {
        return AmigaEntryPoints::ManualEntry {
            reason: "WHDLoad installation (.slave present); v0.2 wizard supports floppy-loadable games only".to_string(),
        };
    }
    if hdf {
        return AmigaEntryPoints::ManualEntry {
            reason: ".hdf hard-drive image present; v0.2 wizard supports floppy-loadable games only".to_string(),
        };
    }
    AmigaEntryPoints::ManualEntry {
        reason: "no .ADF files found".to_string(),
    }
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

    #[test]
    fn single_adf_returns_one_floppy() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("Fatman.adf"));
        match detect(tmp.path()) {
            AmigaEntryPoints::Floppies(v) => assert_eq!(v, vec!["Fatman.adf"]),
            other => panic!("expected Floppies, got {other:?}"),
        }
    }

    #[test]
    fn multi_floppy_sorted_alphabetically() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("Game-Disk3.adf"));
        touch(&tmp.path().join("Game-Disk1.adf"));
        touch(&tmp.path().join("Game-Disk2.adf"));
        match detect(tmp.path()) {
            AmigaEntryPoints::Floppies(v) => assert_eq!(
                v,
                vec!["Game-Disk1.adf", "Game-Disk2.adf", "Game-Disk3.adf"]
            ),
            other => panic!("expected Floppies, got {other:?}"),
        }
    }

    #[test]
    fn whdload_returns_manual_entry() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("Fatman.slave"));
        touch(&tmp.path().join("Fatman.info"));
        match detect(tmp.path()) {
            AmigaEntryPoints::ManualEntry { reason } => {
                assert!(reason.contains("WHDLoad"), "reason: {reason}");
            }
            other => panic!("expected ManualEntry, got {other:?}"),
        }
    }

    #[test]
    fn hdf_returns_manual_entry() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("game.hdf"));
        match detect(tmp.path()) {
            AmigaEntryPoints::ManualEntry { reason } => {
                assert!(reason.contains(".hdf"), "reason: {reason}");
            }
            other => panic!("expected ManualEntry, got {other:?}"),
        }
    }

    #[test]
    fn empty_dir_returns_manual_entry() {
        let tmp = tempdir().unwrap();
        match detect(tmp.path()) {
            AmigaEntryPoints::ManualEntry { reason } => {
                assert!(reason.contains("no .ADF"), "reason: {reason}");
            }
            other => panic!("expected ManualEntry, got {other:?}"),
        }
    }
}
