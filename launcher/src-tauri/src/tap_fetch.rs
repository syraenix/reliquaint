use std::path::Path;
use std::process::Command;

use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("git is not available on PATH — install it with: apt install git")]
    GitNotFound,
    #[error("git clone failed for {url:?}:\n{stderr}")]
    CloneFailed { url: String, stderr: String },
    #[error("git pull failed at {path:?}:\n{stderr}")]
    PullFailed {
        path: std::path::PathBuf,
        stderr: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Returns the version string (e.g. "git version 2.43.0") or an error if git
/// is not on `$PATH`.
pub fn check_git() -> Result<String, FetchError> {
    Command::new("git")
        .arg("--version")
        .output()
        .map_err(|_| FetchError::GitNotFound)
        .and_then(|out| {
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(FetchError::GitNotFound)
            }
        })
}

/// Clone `source` (URL or local path) into `dest` using `git clone --depth=1`.
/// On failure, removes any partial clone before returning the error.
pub fn clone_tap(source: &str, dest: &Path) -> Result<(), FetchError> {
    debug!("git clone --depth=1 {:?} -> {:?}", source, dest);
    let output = Command::new("git")
        .args(["clone", "--depth=1", source])
        .arg(dest)
        .output()?;
    if output.status.success() {
        debug!("clone complete");
        return Ok(());
    }
    let _ = std::fs::remove_dir_all(dest);
    Err(FetchError::CloneFailed {
        url: source.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub struct PullResult {
    pub before_hash: String,
    pub after_hash: String,
    pub already_up_to_date: bool,
}

/// Pull the latest commits on the default branch using `git pull --ff-only`.
/// Warns (does not abort) if the working tree is dirty.
/// Returns the before/after HEAD hashes.
pub fn pull_tap(tap_dir: &Path) -> Result<PullResult, FetchError> {
    let status_out = Command::new("git")
        .args(["-C"])
        .arg(tap_dir)
        .args(["status", "--porcelain"])
        .output()?;
    if !status_out.stdout.is_empty() {
        tracing::warn!(
            "Tap at {:?} has local modifications. \
             Use 'reliquaint tap remove' then 'tap add' to reset.",
            tap_dir
        );
    }

    let before = git_head_hash(tap_dir)?;
    let output = Command::new("git")
        .args(["-C"])
        .arg(tap_dir)
        .args(["pull", "--ff-only"])
        .output()?;
    if !output.status.success() {
        return Err(FetchError::PullFailed {
            path: tap_dir.to_path_buf(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let after = git_head_hash(tap_dir)?;
    debug!("pull: {} -> {}", before, after);
    Ok(PullResult {
        already_up_to_date: before == after,
        before_hash: before,
        after_hash: after,
    })
}

fn git_head_hash(tap_dir: &Path) -> Result<String, FetchError> {
    let out = Command::new("git")
        .args(["-C"])
        .arg(tap_dir)
        .args(["rev-parse", "HEAD"])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as Cmd;

    #[test]
    fn check_git_succeeds_when_git_present() {
        let version = check_git().expect("git should be on PATH in dev environment");
        assert!(version.contains("git version"), "got: {version}");
    }

    fn make_local_tap_repo() -> (tempfile::TempDir, tempfile::TempDir) {
        let bare = tempfile::tempdir().unwrap();
        Cmd::new("git")
            .args(["init", "--bare"])
            .arg(bare.path())
            .output()
            .unwrap();
        let work = tempfile::tempdir().unwrap();
        Cmd::new("git")
            .args(["clone"])
            .arg(bare.path())
            .arg(work.path())
            .output()
            .unwrap();
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();
        std::fs::write(work.path().join("tap.toml"), "id=\"test\"\n").unwrap();
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["push"])
            .output()
            .unwrap();
        (bare, work)
    }

    #[test]
    fn clone_tap_succeeds_against_local_repo() {
        let (bare, _work) = make_local_tap_repo();
        let dest = tempfile::tempdir().unwrap();
        let dest_child = dest.path().join("clone");
        let url = format!("file://{}", bare.path().display());
        clone_tap(&url, &dest_child).expect("clone should succeed");
        assert!(dest_child.join("tap.toml").exists());
    }

    #[test]
    fn clone_tap_cleans_up_on_failure() {
        let dest = tempfile::tempdir().unwrap();
        let target = dest.path().join("bad-clone");
        let result = clone_tap("file:///nonexistent/repo/xyz", &target);
        assert!(result.is_err(), "expected error for bad URL");
        assert!(!target.exists(), "partial clone dir should be removed");
    }

    #[test]
    fn pull_tap_reports_up_to_date_when_no_new_commits() {
        let (bare, _work) = make_local_tap_repo();
        let dest = tempfile::tempdir().unwrap();
        let dest_child = dest.path().join("clone");
        let url = format!("file://{}", bare.path().display());
        clone_tap(&url, &dest_child).unwrap();

        let result = pull_tap(&dest_child).expect("pull should succeed");
        assert_eq!(
            result.before_hash, result.after_hash,
            "no new commits — hashes should match"
        );
        assert!(result.already_up_to_date);
    }
}
