//! Pure-functional directory inspection for the manifest creation
//! wizard. Given a path that the user pointed at, classify what it is
//! (DOS, Amiga, or Unknown), find likely entry points, and extract a
//! starting title + id from the directory name.
//!
//! This module performs **no I/O writes**, prompts no user, and emits
//! no tracing events at INFO or above — it is a pure inspection layer
//! that the CLI wizard (M4) and GUI wizard (M5) both consume.
//!
//! Scope (v0.2):
//! - DOS detection: `.BAT`/`.EXE`/`.COM` files, common Sierra markers.
//! - Amiga detection: floppy-loadable games only. WHDLoad and
//!   HD-installed games return a `ManualEntry` state — see ADR-0003
//!   for the follow-up scope.
//! - README/manual auto-parsing for metadata is **out of scope**.
//!   See `metadata::extract` for the rationale.

use crate::catalog::Platform;
use std::path::Path;

pub mod amiga;
pub mod dos;
pub mod metadata;
pub mod platform;

pub use amiga::{AmigaEntryPoints, detect as detect_amiga_entry_points};
pub use dos::{EntryPointCandidate, EntryPointKind, detect as detect_dos_entry_points};
pub use metadata::{DraftMetadata, extract as extract_metadata};
pub use platform::{Confidence, PlatformDetection, detect as detect_platform};

/// Combined report the wizard uses as a starting point for the draft
/// manifest. Each field is independently computed; callers are free to
/// call the individual `detect_*` / `extract_*` functions instead.
#[derive(Debug, Clone)]
pub struct HeuristicReport {
    pub platform: PlatformDetection,
    pub dos: Vec<EntryPointCandidate>,
    pub amiga: AmigaEntryPoints,
    pub metadata: DraftMetadata,
}

/// Run all heuristics against `dir` and return a combined report. The
/// platform-specific fields are populated unconditionally; the wizard
/// decides which to use based on `platform.platform`.
pub fn analyze(dir: &Path) -> HeuristicReport {
    HeuristicReport {
        platform: detect_platform(dir),
        dos: detect_dos_entry_points(dir),
        amiga: detect_amiga_entry_points(dir),
        metadata: extract_metadata(dir),
    }
}

/// Shared helper: list the top-level entries of `dir` as `(file_name, is_file)`
/// pairs. Returns an empty Vec if `dir` doesn't exist or isn't a directory
/// — the heuristics treat that as "no evidence" rather than an error so
/// the wizard can surface a clear unknown-platform result.
pub(crate) fn list_dir(dir: &Path) -> Vec<(String, bool)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        out.push((name, file_type.is_file()));
    }
    out.sort();
    out
}

/// Convenience: pick a platform from a finished `PlatformDetection`,
/// or `None` if the detection was inconclusive.
pub fn chosen_platform(p: &PlatformDetection) -> Option<Platform> {
    p.platform
}
