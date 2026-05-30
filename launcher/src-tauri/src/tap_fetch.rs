use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::debug;

/// Hard ceiling on a single `git clone`. A tap that hasn't cloned in five
/// minutes is treated as stuck and aborted. Not user-configurable yet.
const CLONE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("git is not available on PATH — install it with: apt install git")]
    GitNotFound,
    #[error("git clone failed for {url:?}:\n{stderr}")]
    CloneFailed { url: String, stderr: String },
    #[error("git clone for {url:?} timed out after {seconds}s")]
    CloneTimeout { url: String, seconds: u64 },
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

/// Clone `source` (URL or local path) into `dest` using `git clone --depth=1`,
/// aborting after [`CLONE_TIMEOUT`]. On failure or timeout, removes any partial
/// clone before returning the error.
pub fn clone_tap(source: &str, dest: &Path) -> Result<(), FetchError> {
    clone_tap_with_timeout(source, dest, CLONE_TIMEOUT)
}

fn clone_tap_with_timeout(source: &str, dest: &Path, timeout: Duration) -> Result<(), FetchError> {
    debug!(
        "git clone --depth=1 {:?} -> {:?} (timeout {:?})",
        source, dest, timeout
    );
    let child = Command::new("git")
        .args(["clone", "--depth=1", source])
        .arg(dest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    match wait_with_timeout(child, timeout)? {
        Outcome::Exited { success: true, .. } => {
            debug!("clone complete");
            Ok(())
        }
        Outcome::Exited {
            success: false,
            stderr,
        } => {
            let _ = std::fs::remove_dir_all(dest);
            Err(FetchError::CloneFailed {
                url: source.to_string(),
                stderr,
            })
        }
        Outcome::TimedOut => {
            let _ = std::fs::remove_dir_all(dest);
            Err(FetchError::CloneTimeout {
                url: source.to_string(),
                seconds: timeout.as_secs(),
            })
        }
    }
}

/// Outcome of waiting on a child process under a deadline.
enum Outcome {
    Exited { success: bool, stderr: String },
    TimedOut,
}

/// Wait for `child` to exit, killing it if it runs past `timeout`.
///
/// Both pipes are drained on background threads so a chatty child (git writes
/// clone/pull progress to stderr) can't deadlock by filling the OS pipe buffer
/// while we poll. `child` must be spawned with piped stdout/stderr.
fn wait_with_timeout(mut child: Child, timeout: Duration) -> Result<Outcome, FetchError> {
    let mut stderr_pipe = child.stderr.take();
    let mut stdout_pipe = child.stdout.take();
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(p) = stderr_pipe.as_mut() {
            let _ = p.read_to_string(&mut buf);
        }
        buf
    });
    let stdout_handle = std::thread::spawn(move || {
        let mut sink = Vec::new();
        if let Some(p) = stdout_pipe.as_mut() {
            let _ = p.read_to_end(&mut sink);
        }
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(status) => {
                let _ = stdout_handle.join();
                let stderr = stderr_handle.join().unwrap_or_default();
                return Ok(Outcome::Exited {
                    success: status.success(),
                    stderr,
                });
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Ok(Outcome::TimedOut);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
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

/// The HEAD commit hash of a cached tap. Used to compare against the remote.
pub fn local_head(tap_dir: &Path) -> Result<String, FetchError> {
    git_head_hash(tap_dir)
}

/// The committer date of a cached tap's HEAD as an ISO short date
/// (`YYYY-MM-DD`), used as a "last synced" proxy. `None` if it can't be read.
pub fn last_commit_date(tap_dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C"])
        .arg(tap_dir)
        .args(["log", "-1", "--format=%cs"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// The HEAD commit hash on the remote's default branch, via
/// `git ls-remote <source> HEAD`. Errors (including no network) propagate so
/// callers can degrade gracefully to an "unknown" status.
pub fn remote_head(source: &str) -> Result<String, FetchError> {
    let out = Command::new("git")
        .args(["ls-remote", source, "HEAD"])
        .output()?;
    if !out.status.success() {
        return Err(FetchError::PullFailed {
            path: std::path::PathBuf::from(source),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    // Output looks like: "<sha>\tHEAD". Take the first whitespace token.
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| FetchError::PullFailed {
            path: std::path::PathBuf::from(source),
            stderr: "git ls-remote returned no HEAD ref".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as Cmd;

    #[test]
    fn wait_with_timeout_kills_long_running_child() {
        let child = Cmd::new("sleep")
            .arg("30")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = Instant::now();
        let outcome = wait_with_timeout(child, Duration::from_millis(100)).unwrap();
        assert!(matches!(outcome, Outcome::TimedOut));
        // Returned promptly rather than waiting out the full 30s sleep.
        assert!(start.elapsed() < Duration::from_secs(5));
        // The child was actually killed, not left running.
        let alive = Cmd::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(!alive, "child {pid} should have been killed");
    }

    #[test]
    fn wait_with_timeout_returns_exit_for_fast_child() {
        let child = Cmd::new("true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let outcome = wait_with_timeout(child, Duration::from_secs(5)).unwrap();
        assert!(matches!(outcome, Outcome::Exited { success: true, .. }));
    }

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
