//! Process supervisor for the emulator and its sidecars.
//!
//! Lifecycle per `docs/schema.md` §"Sidecar handling":
//!
//! 1. Spawn every sidecar from the [`LaunchPlan`].
//! 2. Spawn the primary emulator.
//! 3. Block until the emulator exits.
//! 4. Terminate sidecars in reverse spawn order: SIGTERM, ~500ms grace
//!    period, then SIGKILL if still alive.
//! 5. Return the emulator's exit status.
//!
//! Dry-run is **not** handled here — the CLI prints the plan and skips
//! [`run_plan`] entirely when `--dry-run` is set, so this module stays
//! a pure side-effect-doer.

use std::io::BufRead;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::launch::{LaunchPlan, SidecarSpec};

const GRACE_PERIOD: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("failed to spawn sidecar {name:?}: {source}")]
    SpawnFailed {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to spawn primary emulator: {source}")]
    PrimarySpawnFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("failed to wait on primary emulator: {source}")]
    WaitFailed {
        #[source]
        source: std::io::Error,
    },
}

struct SidecarHandle {
    name: String,
    child: Child,
}

impl SidecarHandle {
    fn spawn(spec: &SidecarSpec) -> Result<Self, SidecarError> {
        let child =
            spec.command
                .to_command()
                .spawn()
                .map_err(|source| SidecarError::SpawnFailed {
                    name: spec.name.clone(),
                    source,
                })?;
        tracing::info!(name = %spec.name, pid = child.id(), "sidecar started");
        Ok(Self {
            name: spec.name.clone(),
            child,
        })
    }

    /// SIGTERM, wait up to GRACE_PERIOD, SIGKILL if still alive.
    fn shutdown(mut self) {
        let pid = self.child.id() as i32;
        // SAFETY: libc::kill is safe to call with a valid pid; on Linux
        // sending SIGTERM to a known-good child PID is unconditionally OK.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }

        let deadline = Instant::now() + GRACE_PERIOD;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    tracing::debug!(name = %self.name, "sidecar exited after SIGTERM");
                    return;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(e) => {
                    tracing::warn!(
                        name = %self.name,
                        error = %e,
                        "wait failed during sidecar shutdown"
                    );
                    return;
                }
            }
        }

        tracing::warn!(name = %self.name, "sidecar required SIGKILL after grace period");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where a line came from when streaming primary-emulator output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSource {
    Stdout,
    Stderr,
}

/// Spawn every sidecar, then the primary, block until primary exits,
/// then shut sidecars down in reverse spawn order. All declared
/// sidecars are required: if any fails to start, already-started
/// sidecars are torn down and the primary is never spawned.
///
/// The primary's stdout/stderr inherit the parent's handles (good for
/// the CLI). Use [`run_plan_with_callback`] when you need to capture
/// the output line-by-line (e.g. for the GUI diagnostic panel).
pub fn run_plan(plan: LaunchPlan) -> Result<ExitStatus, SidecarError> {
    run_plan_inner(plan, None)
}

/// Same as [`run_plan`] but pipes the primary's stdout/stderr and calls
/// `on_line` once per line on background reader threads. The callback
/// must be `Send + Sync` because it runs from both reader threads.
pub fn run_plan_with_callback<F>(plan: LaunchPlan, on_line: F) -> Result<ExitStatus, SidecarError>
where
    F: Fn(OutputSource, &str) + Send + Sync + 'static,
{
    let cb: LineCallback = Arc::new(on_line);
    run_plan_inner(plan, Some(cb))
}

/// Per-line callback shared across the stdout/stderr reader threads.
type LineCallback = Arc<dyn Fn(OutputSource, &str) + Send + Sync>;

fn run_plan_inner(
    plan: LaunchPlan,
    on_line: Option<LineCallback>,
) -> Result<ExitStatus, SidecarError> {
    let mut handles: Vec<SidecarHandle> = Vec::with_capacity(plan.sidecars.len());
    for spec in &plan.sidecars {
        match SidecarHandle::spawn(spec) {
            Ok(h) => handles.push(h),
            Err(e) => {
                for h in handles {
                    h.shutdown();
                }
                return Err(e);
            }
        }
    }

    tracing::info!(
        program = %plan.primary.program,
        sidecar_count = handles.len(),
        piped = on_line.is_some(),
        "spawning primary emulator"
    );

    let mut primary_cmd = plan.primary.to_command();
    if on_line.is_some() {
        primary_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }

    let status_result = (|| -> Result<ExitStatus, SidecarError> {
        let mut child = primary_cmd
            .spawn()
            .map_err(|source| SidecarError::PrimarySpawnFailed { source })?;

        let mut reader_handles: Vec<std::thread::JoinHandle<()>> = Vec::new();
        if let Some(cb) = &on_line {
            if let Some(stdout) = child.stdout.take() {
                let cb = cb.clone();
                reader_handles.push(std::thread::spawn(move || {
                    let reader = std::io::BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        cb(OutputSource::Stdout, &line);
                    }
                }));
            }
            if let Some(stderr) = child.stderr.take() {
                let cb = cb.clone();
                reader_handles.push(std::thread::spawn(move || {
                    let reader = std::io::BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        cb(OutputSource::Stderr, &line);
                    }
                }));
            }
        }

        let status = child
            .wait()
            .map_err(|source| SidecarError::WaitFailed { source })?;

        for h in reader_handles {
            let _ = h.join();
        }

        Ok(status)
    })();

    // Always reap sidecars, even if the primary errored.
    for h in handles.into_iter().rev() {
        h.shutdown();
    }

    status_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::{LaunchPlan, PreparedCommand, SidecarSpec};

    fn cmd(program: &str, args: &[&str]) -> PreparedCommand {
        PreparedCommand {
            program: program.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            working_dir: None,
        }
    }

    #[test]
    fn run_plan_returns_primary_exit_status_with_no_sidecars() {
        let plan = LaunchPlan {
            primary: cmd("true", &[]),
            sidecars: vec![],
        };
        let status = run_plan(plan).unwrap();
        assert!(status.success());
    }

    #[test]
    fn run_plan_reflects_primary_nonzero_exit() {
        let plan = LaunchPlan {
            primary: cmd("false", &[]),
            sidecars: vec![],
        };
        let status = run_plan(plan).unwrap();
        assert!(!status.success());
    }

    #[test]
    fn sidecar_is_reaped_when_emulator_exits_within_grace_period() {
        // The explicit "Done when" criterion for Task 3.4: sleep-5
        // sidecar wrapped around a sleep-1 emulator should return in
        // about 1 second + the grace period, not 5 seconds.
        let plan = LaunchPlan {
            primary: cmd("sleep", &["1"]),
            sidecars: vec![SidecarSpec {
                name: "test".into(),
                command: cmd("sleep", &["5"]),
            }],
        };

        let start = Instant::now();
        let status = run_plan(plan).unwrap();
        let elapsed = start.elapsed();

        assert!(status.success());
        // 1s primary + up to 500ms grace + slack for scheduling.
        assert!(
            elapsed < Duration::from_secs(2),
            "expected ~1s, got {elapsed:?} — sidecar likely not reaped"
        );
    }

    #[test]
    fn run_plan_with_callback_captures_stdout_and_stderr_lines() {
        use std::sync::Mutex;
        let captured: Arc<Mutex<Vec<(OutputSource, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_cb = captured.clone();
        let plan = LaunchPlan {
            // sh -c writes to both stdout and stderr.
            primary: cmd(
                "sh",
                &["-c", "echo hello-out; echo hello-err 1>&2; echo line-two"],
            ),
            sidecars: vec![],
        };

        let status = run_plan_with_callback(plan, move |src, line| {
            captured_for_cb
                .lock()
                .unwrap()
                .push((src, line.to_string()));
        })
        .unwrap();
        assert!(status.success());

        let lines = captured.lock().unwrap();
        let stdout_lines: Vec<&str> = lines
            .iter()
            .filter(|(s, _)| *s == OutputSource::Stdout)
            .map(|(_, l)| l.as_str())
            .collect();
        let stderr_lines: Vec<&str> = lines
            .iter()
            .filter(|(s, _)| *s == OutputSource::Stderr)
            .map(|(_, l)| l.as_str())
            .collect();
        assert!(
            stdout_lines.contains(&"hello-out"),
            "stdout lines: {stdout_lines:?}"
        );
        assert!(stdout_lines.contains(&"line-two"));
        assert!(
            stderr_lines.contains(&"hello-err"),
            "stderr lines: {stderr_lines:?}"
        );
    }

    #[test]
    fn sidecar_spawn_failure_prevents_primary_launch() {
        let plan = LaunchPlan {
            primary: cmd("true", &[]),
            sidecars: vec![SidecarSpec {
                name: "ghost".into(),
                command: cmd("/definitely/not/a/real/binary/anywhere", &[]),
            }],
        };

        let err = run_plan(plan).unwrap_err();
        match err {
            SidecarError::SpawnFailed { name, .. } => assert_eq!(name, "ghost"),
            other => panic!("expected SpawnFailed, got {other:?}"),
        }
    }
}
