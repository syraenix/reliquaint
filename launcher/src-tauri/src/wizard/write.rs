//! Atomic file writing for the wizard.

use std::io::Write;
use std::path::Path;

/// Write `contents` to `path` atomically: stage in a temp file in the
/// same directory, then rename. The rename is atomic on the same
/// filesystem, so a crashed wizard can't leave a half-written manifest
/// behind for the catalog loader to trip over.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other(format!("no parent for {}", path.display())))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_atomic_creates_file_with_content() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("out.toml");
        write_atomic(&p, "[section]\nkey = 1\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[section]\nkey = 1\n");
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("out.toml");
        std::fs::write(&p, "old").unwrap();
        write_atomic(&p, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
    }
}
