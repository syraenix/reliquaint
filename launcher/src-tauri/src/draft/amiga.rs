//! Draft an Amiga catalog entry for a floppy-loadable game. The
//! resulting `runtime.fs_uae.config` is `None`, so the launcher falls
//! back to its built-in model template (see
//! `crate::launch::compose_fs_uae`).

use super::DraftError;
use crate::catalog::{
    Acquisition, AmigaModel, CatalogEntry, Emulator, FsUaeRuntime, Game, Install, Meta, Platform,
    Runtime,
};
use crate::heuristic::{AmigaEntryPoints, DraftMetadata};

/// Compose a draft Amiga catalog entry from the heuristic's
/// floppy-disk list. Defaults to A500 (the most common single-floppy
/// target); the wizard can override.
pub fn compose(
    metadata: &DraftMetadata,
    entry_points: &AmigaEntryPoints,
) -> Result<CatalogEntry, DraftError> {
    if metadata.id.is_empty() {
        return Err(DraftError::NoIdDerivable);
    }
    let floppies = match entry_points {
        AmigaEntryPoints::Floppies(v) => v.clone(),
        AmigaEntryPoints::ManualEntry { reason } => {
            return Err(DraftError::NoAmigaFloppies {
                reason: reason.clone(),
            });
        }
    };
    Ok(CatalogEntry {
        schema_version: 1,
        game: Game {
            id: metadata.id.clone(),
            title: metadata.title.clone(),
            platform: Platform::Amiga,
            collection: None,
            collection_name: None,
        },
        meta: Meta::default(),
        acquisition: Acquisition::default(),
        install: Install::default(),
        runtime: Runtime {
            emulator: Emulator::FsUae,
            sidecars: vec![],
            dosbox: None,
            fs_uae: Some(FsUaeRuntime {
                model: AmigaModel::A500,
                config: None,
                floppies,
                hard_drives: vec![],
            }),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> DraftMetadata {
        DraftMetadata {
            id: "fatman".to_string(),
            title: "Fatman".to_string(),
        }
    }

    #[test]
    fn compose_uses_floppy_list_verbatim() {
        let eps = AmigaEntryPoints::Floppies(vec![
            "Game-Disk1.adf".to_string(),
            "Game-Disk2.adf".to_string(),
        ]);
        let entry = compose(&metadata(), &eps).unwrap();
        assert_eq!(entry.game.platform, Platform::Amiga);
        let fs = entry.runtime.fs_uae.as_ref().unwrap();
        assert_eq!(fs.model, AmigaModel::A500);
        assert!(fs.config.is_none());
        assert_eq!(fs.floppies, vec!["Game-Disk1.adf", "Game-Disk2.adf"]);
        assert!(fs.hard_drives.is_empty());
    }

    #[test]
    fn compose_round_trips_through_parser() {
        let eps = AmigaEntryPoints::Floppies(vec!["Fatman.adf".to_string()]);
        let entry = compose(&metadata(), &eps).unwrap();
        let text = toml::to_string_pretty(&entry).unwrap();
        let parsed =
            crate::catalog::parse_str(&text, std::path::Path::new("tap/catalog/amiga/fatman.toml"))
                .unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn compose_errors_on_manual_entry_state() {
        let eps = AmigaEntryPoints::ManualEntry {
            reason: "WHDLoad install".to_string(),
        };
        let err = compose(&metadata(), &eps).unwrap_err();
        match err {
            DraftError::NoAmigaFloppies { reason } => assert!(reason.contains("WHDLoad")),
            other => panic!("expected NoAmigaFloppies, got {other:?}"),
        }
    }

    #[test]
    fn compose_errors_without_id() {
        let mut meta = metadata();
        meta.id = String::new();
        let eps = AmigaEntryPoints::Floppies(vec!["x.adf".to_string()]);
        let err = compose(&meta, &eps).unwrap_err();
        assert!(matches!(err, DraftError::NoIdDerivable));
    }
}
