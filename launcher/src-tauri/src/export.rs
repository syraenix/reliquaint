//! Produce a "submission-ready" form of a user-tap manifest.
//!
//! The user can copy this output verbatim into a bundled-tap PR. The
//! catalog entry type is already portable (no user paths leak into it
//! by design), so we run a full re-validation and add warnings about
//! completeness gaps that reviewers usually flag — no value stripping
//! is needed for v0.2.

use crate::catalog::CatalogEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedManifest {
    /// Canonical TOML body, ready to drop into
    /// `tap/catalog/<platform>/<id>.toml`.
    pub content: String,
    /// Per-field completeness gaps that reviewers usually ask about.
    /// Non-fatal; users may still submit.
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("validation failed: {0}")]
    Validation(String),
}

/// Validate `entry` end-to-end, render it as canonical TOML, and
/// gather completeness warnings.
pub fn export_manifest(entry: &CatalogEntry) -> Result<ExportedManifest, ExportError> {
    crate::wizard::validate(entry).map_err(|e| match e {
        crate::wizard::WizardError::Validation(s) => ExportError::Validation(s),
        other => ExportError::Validation(other.to_string()),
    })?;
    let content =
        toml::to_string_pretty(entry).map_err(|e| ExportError::Validation(e.to_string()))?;
    Ok(ExportedManifest {
        content,
        warnings: gather_warnings(entry),
    })
}

fn gather_warnings(entry: &CatalogEntry) -> Vec<String> {
    let mut w = Vec::new();
    let m = &entry.meta;
    if m.year.is_none() {
        w.push("[meta].year is missing — reviewers usually want a release year".to_string());
    }
    if m.developer.is_none() {
        w.push("[meta].developer is missing".to_string());
    }
    if m.publisher.is_none() {
        w.push("[meta].publisher is missing".to_string());
    }
    if m.description.is_none() {
        w.push(
            "[meta].description is missing — reviewers usually want a short description"
                .to_string(),
        );
    }
    if m.genre.is_empty() {
        w.push("[meta].genre is empty".to_string());
    }
    let a = &entry.acquisition;
    if a.gog.is_none()
        && a.steam.is_none()
        && a.developer_site.is_none()
        && a.archive.is_none()
        && a.amiga_forever.is_none()
    {
        w.push("[acquisition] has no purchase or download link — reviewers ask how the game is distributed".to_string());
    }
    w
}

/// GitHub URL the user can open to file a PR adding the manifest to
/// the bundled tap. `branch` is the target branch on the upstream
/// repo (typically `develop` for v0.2).
pub fn github_new_file_url(platform: &str, branch: &str) -> String {
    format!("https://github.com/syraenix/reliquaint/new/{branch}/tap/catalog/{platform}/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{
        Acquisition, CatalogEntry, DosboxRuntime, Emulator, Game, Install, Meta, Platform, Runtime,
    };

    fn minimal_dos_entry() -> CatalogEntry {
        CatalogEntry {
            schema_version: 1,
            game: Game {
                id: "minimal".to_string(),
                title: "Minimal".to_string(),
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
                    config: "minimal.conf".to_string(),
                    entry: "RUN.BAT".to_string(),
                    mount: "c".to_string(),
                }),
                fs_uae: None,
            },
        }
    }

    #[test]
    fn export_round_trips_through_parser() {
        let entry = minimal_dos_entry();
        let out = export_manifest(&entry).unwrap();
        let parsed = crate::catalog::parse_str(
            &out.content,
            std::path::Path::new("tap/catalog/dos/minimal.toml"),
        )
        .unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn warnings_flag_missing_meta_and_acquisition() {
        let entry = minimal_dos_entry();
        let out = export_manifest(&entry).unwrap();
        // Bare entry has no meta and no acquisition; expect at least 5 warnings.
        assert!(out.warnings.iter().any(|w| w.contains("year")));
        assert!(out.warnings.iter().any(|w| w.contains("developer")));
        assert!(out.warnings.iter().any(|w| w.contains("publisher")));
        assert!(out.warnings.iter().any(|w| w.contains("description")));
        assert!(out.warnings.iter().any(|w| w.contains("genre")));
        assert!(out.warnings.iter().any(|w| w.contains("[acquisition]")));
    }

    #[test]
    fn warnings_quiet_when_meta_complete() {
        let mut entry = minimal_dos_entry();
        entry.meta = Meta {
            year: Some(1989),
            developer: Some("Sierra".to_string()),
            publisher: Some("Sierra".to_string()),
            genre: vec!["adventure".to_string()],
            tags: vec![],
            description: Some("desc".to_string()),
        };
        entry.acquisition = Acquisition {
            gog: Some("https://gog".to_string()),
            ..Default::default()
        };
        let out = export_manifest(&entry).unwrap();
        assert!(out.warnings.is_empty(), "got warnings: {:?}", out.warnings);
    }

    #[test]
    fn github_url_uses_platform_and_branch() {
        let url = github_new_file_url("dos", "develop");
        assert!(url.contains("/new/develop/tap/catalog/dos/"));
    }
}
