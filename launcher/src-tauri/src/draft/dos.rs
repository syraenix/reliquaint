//! Draft a DOS catalog entry and the sibling `.conf` that lives next
//! to it. Inputs come from the heuristic engine; the wizard hands the
//! resulting `CatalogEntry` to the user for review/edit before writing.

use super::DraftError;
use crate::catalog::{
    Acquisition, CatalogEntry, DosboxRuntime, Emulator, Game, Install, Meta, Platform, Runtime,
};
use crate::heuristic::{DraftMetadata, EntryPointCandidate};

/// The repo-shipped DOSBox-Staging baseline. Embedded at compile time
/// so the launcher can hand out a fresh copy without depending on the
/// repo root being present on disk.
const DEFAULT_CONF_TEMPLATE: &str = include_str!("../../../../config/default-dosbox-staging.conf");

/// Compose a draft DOS catalog entry using the top-ranked entry-point
/// candidate. Returns `NoIdDerivable` or `NoDosEntryPoint` when the
/// wizard needs to prompt for input.
pub fn compose(
    metadata: &DraftMetadata,
    candidates: &[EntryPointCandidate],
) -> Result<CatalogEntry, DraftError> {
    if metadata.id.is_empty() {
        return Err(DraftError::NoIdDerivable);
    }
    let entry = candidates
        .first()
        .ok_or(DraftError::NoDosEntryPoint)?
        .file_name
        .clone();
    Ok(CatalogEntry {
        schema_version: 1,
        game: Game {
            id: metadata.id.clone(),
            title: metadata.title.clone(),
            platform: Platform::Dos,
            collection: None,
            collection_name: None,
        },
        meta: Meta::default(),
        acquisition: Acquisition::default(),
        install: Install::default(),
        runtime: Runtime {
            emulator: Emulator::DosboxStaging,
            sidecars: vec![],
            dosbox: Some(DosboxRuntime {
                config: format!("{}.conf", metadata.id),
                entry,
                mount: "c".to_string(),
            }),
            fs_uae: None,
        },
    })
}

/// Generate a default DOSBox-Staging config to ship alongside a
/// user-created catalog entry. Starts from the repo's
/// `config/default-dosbox-staging.conf` and strips the `[autoexec]`
/// section per ADR-0002 — the launcher composes autoexec at runtime.
pub fn default_dosbox_config() -> String {
    strip_autoexec(DEFAULT_CONF_TEMPLATE)
}

fn strip_autoexec(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_autoexec = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_autoexec = trimmed.eq_ignore_ascii_case("[autoexec]");
            if in_autoexec {
                continue;
            }
        }
        if in_autoexec {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // Trim trailing blank lines.
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristic::EntryPointKind;

    fn metadata() -> DraftMetadata {
        DraftMetadata {
            id: "my-custom-game".to_string(),
            title: "My Custom Game".to_string(),
        }
    }

    fn bat_candidate(name: &str) -> EntryPointCandidate {
        EntryPointCandidate {
            file_name: name.to_string(),
            kind: EntryPointKind::Bat,
            reason: "test".to_string(),
        }
    }

    #[test]
    fn compose_uses_top_ranked_candidate() {
        let entry = compose(
            &metadata(),
            &[bat_candidate("RUN.BAT"), bat_candidate("OTHER.BAT")],
        )
        .unwrap();
        assert_eq!(entry.game.id, "my-custom-game");
        assert_eq!(entry.game.platform, Platform::Dos);
        let dos = entry.runtime.dosbox.as_ref().unwrap();
        assert_eq!(dos.entry, "RUN.BAT");
        assert_eq!(dos.config, "my-custom-game.conf");
        assert_eq!(dos.mount, "c");
    }

    #[test]
    fn compose_round_trips_through_parser() {
        let entry = compose(&metadata(), &[bat_candidate("RUN.BAT")]).unwrap();
        let text = toml::to_string_pretty(&entry).unwrap();
        let parsed = crate::catalog::parse_str(
            &text,
            std::path::Path::new("tap/catalog/dos/my-custom-game.toml"),
        )
        .unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn compose_errors_without_id() {
        let mut meta = metadata();
        meta.id = String::new();
        let err = compose(&meta, &[bat_candidate("RUN.BAT")]).unwrap_err();
        assert!(matches!(err, DraftError::NoIdDerivable));
    }

    #[test]
    fn compose_errors_without_entry_point() {
        let err = compose(&metadata(), &[]).unwrap_err();
        assert!(matches!(err, DraftError::NoDosEntryPoint));
    }

    #[test]
    fn default_config_omits_autoexec_block() {
        let conf = default_dosbox_config();
        assert!(!conf.to_ascii_lowercase().contains("[autoexec]"));
        // Sanity: the rest is still recognizable DOSBox config.
        assert!(conf.contains("[sdl]"));
        assert!(conf.contains("[cpu]"));
    }

    #[test]
    fn strip_autoexec_handles_section_in_middle() {
        let input = "[sdl]\nx=1\n[autoexec]\nMOUNT C foo\n[cpu]\ny=2\n";
        let out = strip_autoexec(input);
        assert!(out.contains("[sdl]"));
        assert!(out.contains("[cpu]"));
        assert!(!out.contains("[autoexec]"));
        assert!(!out.contains("MOUNT"));
    }
}
