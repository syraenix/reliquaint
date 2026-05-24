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

use std::process::{Child, ExitStatus};
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
        let child = spec.command.to_command().spawn().map_err(|source| {
            SidecarError::SpawnFailed {
                name: spec.name.clone(),
                source,
            }
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

/// Spawn every sidecar, then the primary, block until primary exits,
/// then shut sidecars down in reverse spawn order. All declared
/// sidecars are required: if any fails to start, already-started
/// sidecars are torn down and the primary is never spawned.
pub fn run_plan(plan: LaunchPlan) -> Result<ExitStatus, SidecarError> {
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
        "spawning primary emulator"
    );
    let status_result = plan
        .primary
        .to_command()
        .spawn()
        .map_err(|source| SidecarError::PrimarySpawnFailed { source })
        .and_then(|mut child| {
            child
                .wait()
                .map_err(|source| SidecarError::WaitFailed { source })
        });

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
