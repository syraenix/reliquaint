//! Compose a draft `CatalogEntry` from heuristic output. The output
//! round-trips cleanly through [`crate::catalog::parse_str`]; the
//! wizard validates it before writing to disk.

use crate::catalog::{CatalogEntry, Platform};
use crate::heuristic::HeuristicReport;

pub mod amiga;
pub mod dos;

#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    #[error("could not derive an id from the directory name; please provide one")]
    NoIdDerivable,

    #[error("no DOS entry point detected; please provide one")]
    NoDosEntryPoint,

    #[error("no Amiga floppy disks detected: {reason}")]
    NoAmigaFloppies { reason: String },

    #[error("platform could not be determined; please choose DOS or Amiga")]
    UnknownPlatform,
}

/// Compose a draft for whichever platform the heuristic chose. Returns
/// `UnknownPlatform` if the classifier was inconclusive — in that case
/// the wizard prompts the user to pick a platform and then calls
/// [`dos::compose`] or [`amiga::compose`] directly with the choice.
pub fn from_report(report: &HeuristicReport) -> Result<CatalogEntry, DraftError> {
    match report.platform.platform {
        Some(Platform::Dos) => dos::compose(&report.metadata, &report.dos),
        Some(Platform::Amiga) => amiga::compose(&report.metadata, &report.amiga),
        None => Err(DraftError::UnknownPlatform),
    }
}
