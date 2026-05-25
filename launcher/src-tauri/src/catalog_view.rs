//! Catalog + install-record join.
//!
//! Takes the loaded taps from [`crate::tap::load_tap`] and the loaded
//! install records from [`crate::install_record::load_all`] and produces
//! a single queryable view that the CLI (Milestone 4) and GUI
//! (Milestone 5) both consume.
//!
//! Joined by `(tap_id, game_id)`. Records that don't match any catalog
//! entry land in [`CatalogView::orphans`].

use std::collections::HashMap;
use std::path::PathBuf;

use crate::catalog::{CatalogEntry, Platform};
use crate::install_record::{InstallRecord, LoadedInstallRecord};
use crate::tap::LoadedTap;

#[derive(Debug, Clone)]
pub struct CatalogView {
    entries: Vec<CatalogViewEntry>,
    orphans: Vec<LoadedInstallRecord>,
}

#[derive(Debug, Clone)]
pub struct CatalogViewEntry {
    pub tap_id: String,
    pub source_path: PathBuf,
    pub catalog: CatalogEntry,
    pub install: Option<InstallRecord>,
}

impl CatalogView {
    /// Build a view by joining loaded taps and install records on
    /// `(tap_id, game_id)`.
    ///
    /// In v0.1 the bundled tap is the only one, so multi-tap collisions
    /// shouldn't occur. ADR-0003 commits to per-tap priority resolution in
    /// v0.3; until then, a duplicate `(tap_id, game_id)` across input taps
    /// is a programming error and panics.
    pub fn assemble(taps: Vec<LoadedTap>, installs: Vec<LoadedInstallRecord>) -> Self {
        let mut install_by_key: HashMap<(String, String), LoadedInstallRecord> =
            HashMap::with_capacity(installs.len());
        for li in installs {
            let key = (
                li.record.install.tap.clone(),
                li.record.install.catalog_id.clone(),
            );
            install_by_key.insert(key, li);
        }

        let mut entries: Vec<CatalogViewEntry> = Vec::new();
        let mut seen_keys: HashMap<(String, String), PathBuf> = HashMap::new();

        for tap in taps {
            let tap_id = tap.metadata.id.clone();
            for (game_id, loaded_entry) in tap.entries {
                let key = (tap_id.clone(), game_id.clone());
                if let Some(prior) = seen_keys.get(&key) {
                    unreachable!(
                        "multi-tap conflict resolution lands in v0.3: \
                         duplicate (tap={tap_id:?}, game={game_id:?}) from {prior:?} \
                         and {new:?}",
                        prior = prior,
                        new = loaded_entry.source_path,
                    );
                }
                seen_keys.insert(key.clone(), loaded_entry.source_path.clone());
                let install = install_by_key.remove(&key).map(|li| li.record);
                entries.push(CatalogViewEntry {
                    tap_id: tap_id.clone(),
                    source_path: loaded_entry.source_path,
                    catalog: loaded_entry.entry,
                    install,
                });
            }
        }

        // Stable order: by (tap_id, game id). LoadedTap's BTreeMap already
        // gives us game-id order within a tap; explicit sort here covers
        // multi-tap input.
        entries.sort_by(|a, b| {
            a.tap_id
                .cmp(&b.tap_id)
                .then_with(|| a.catalog.game.id.cmp(&b.catalog.game.id))
        });

        let mut orphans: Vec<LoadedInstallRecord> = install_by_key.into_values().collect();
        orphans.sort_by(|a, b| a.source_path.cmp(&b.source_path));

        Self { entries, orphans }
    }

    /// Every catalog entry, installed or not, in stable
    /// `(tap_id, game_id)` order.
    pub fn all(&self) -> &[CatalogViewEntry] {
        &self.entries
    }

    /// Look up an entry by game id. v0.1 has a single tap, so at most one
    /// match is possible; the signature returns `Option` accordingly.
    /// Multi-tap conflict resolution (v0.3) will change this to take an
    /// explicit `(tap_id, game_id)` or to return all matches.
    pub fn by_id(&self, id: &str) -> Option<&CatalogViewEntry> {
        self.entries.iter().find(|e| e.catalog.game.id == id)
    }

    pub fn by_platform(&self, platform: Platform) -> impl Iterator<Item = &CatalogViewEntry> {
        self.entries
            .iter()
            .filter(move |e| e.catalog.game.platform == platform)
    }

    pub fn installed_only(&self) -> impl Iterator<Item = &CatalogViewEntry> {
        self.entries.iter().filter(|e| e.install.is_some())
    }

    pub fn not_installed(&self) -> impl Iterator<Item = &CatalogViewEntry> {
        self.entries.iter().filter(|e| e.install.is_none())
    }

    /// Install records whose `(tap, catalog_id)` does not match any loaded
    /// catalog entry. Surfaced in the GUI as "your tap may have been
    /// removed."
    pub fn orphans(&self) -> &[LoadedInstallRecord] {
        &self.orphans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{
        Acquisition, AmigaModel, CatalogEntry, DosboxRuntime, Emulator, FsUaeRuntime, Game,
        Install as CatInstall, Meta, Platform, Runtime,
    };
    use crate::install_record::{Install as InstallRec, InstallRecord};
    use crate::tap::{LoadedEntry, LoadedTap, TapMetadata};
    use std::collections::BTreeMap;
    use std::str::FromStr;

    fn tap_metadata(id: &str) -> TapMetadata {
        TapMetadata {
            schema_version: 1,
            id: id.into(),
            title: id.into(),
            description: "test".into(),
            version: "0.1.0".into(),
            maintainer: "test".into(),
            url: "https://example.test".into(),
            license: "MIT".into(),
        }
    }

    fn dos_entry(id: &str) -> CatalogEntry {
        CatalogEntry {
            schema_version: 1,
            game: Game {
                id: id.into(),
                title: id.into(),
                platform: Platform::Dos,
                collection: None,
            },
            meta: Meta::default(),
            acquisition: Acquisition::default(),
            install: CatInstall::default(),
            runtime: Runtime {
                emulator: Emulator::DosboxStaging,
                sidecars: vec![],
                dosbox: Some(DosboxRuntime {
                    config: format!("{id}.conf"),
                    entry: "TEST.EXE".into(),
                    mount: "c".into(),
                }),
                fs_uae: None,
            },
        }
    }

    fn amiga_entry(id: &str) -> CatalogEntry {
        CatalogEntry {
            schema_version: 1,
            game: Game {
                id: id.into(),
                title: id.into(),
                platform: Platform::Amiga,
                collection: None,
            },
            meta: Meta::default(),
            acquisition: Acquisition::default(),
            install: CatInstall::default(),
            runtime: Runtime {
                emulator: Emulator::FsUae,
                sidecars: vec![],
                dosbox: None,
                fs_uae: Some(FsUaeRuntime {
                    model: AmigaModel::A500,
                    config: None,
                    floppies: vec![],
                    hard_drives: vec![],
                }),
            },
        }
    }

    fn loaded_tap(tap_id: &str, entries: Vec<CatalogEntry>) -> LoadedTap {
        let mut map = BTreeMap::new();
        for entry in entries {
            let game_id = entry.game.id.clone();
            map.insert(
                game_id.clone(),
                LoadedEntry {
                    source_path: PathBuf::from(format!("/test/{tap_id}/{game_id}.toml")),
                    entry,
                },
            );
        }
        LoadedTap {
            metadata: tap_metadata(tap_id),
            root: PathBuf::from(format!("/test/{tap_id}")),
            entries: map,
        }
    }

    fn loaded_install(tap: &str, catalog_id: &str) -> LoadedInstallRecord {
        LoadedInstallRecord {
            source_path: PathBuf::from(format!("/test/installs/{catalog_id}.toml")),
            record: InstallRecord {
                schema_version: 1,
                install: InstallRec {
                    catalog_id: catalog_id.into(),
                    tap: tap.into(),
                    install_path: PathBuf::from(format!("/games/{catalog_id}")),
                    installed_at: toml::value::Datetime::from_str("2026-05-23T14:32:00Z").unwrap(),
                },
            },
        }
    }

    #[test]
    fn join_marks_installed_and_not_installed() {
        let tap = loaded_tap(
            "core",
            vec![
                dos_entry("qfg1-ega"),
                dos_entry("qfg2"),
                amiga_entry("fatman"),
            ],
        );
        let installs = vec![
            loaded_install("core", "qfg1-ega"),
            loaded_install("core", "fatman"),
        ];

        let view = CatalogView::assemble(vec![tap], installs);

        assert_eq!(view.all().len(), 3);
        assert_eq!(view.installed_only().count(), 2);
        assert_eq!(view.not_installed().count(), 1);
        assert_eq!(view.orphans().len(), 0);

        let not_installed_ids: Vec<&str> = view
            .not_installed()
            .map(|e| e.catalog.game.id.as_str())
            .collect();
        assert_eq!(not_installed_ids, vec!["qfg2"]);
    }

    #[test]
    fn install_record_with_unknown_catalog_id_becomes_orphan() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let installs = vec![
            loaded_install("core", "qfg1-ega"),
            loaded_install("core", "ghost-game"),
        ];

        let view = CatalogView::assemble(vec![tap], installs);

        assert_eq!(view.all().len(), 1);
        assert_eq!(view.installed_only().count(), 1);
        assert_eq!(view.orphans().len(), 1);
        assert_eq!(view.orphans()[0].record.install.catalog_id, "ghost-game");
    }

    #[test]
    fn install_record_referencing_unknown_tap_becomes_orphan() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let installs = vec![loaded_install("other-tap", "qfg1-ega")];

        let view = CatalogView::assemble(vec![tap], installs);

        assert_eq!(view.installed_only().count(), 0);
        assert_eq!(view.orphans().len(), 1);
    }

    #[test]
    fn by_id_returns_some_for_known_none_for_unknown() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);

        assert!(view.by_id("qfg1-ega").is_some());
        assert!(view.by_id("nonexistent").is_none());
    }

    #[test]
    fn by_platform_filters_correctly() {
        let tap = loaded_tap(
            "core",
            vec![
                dos_entry("qfg1-ega"),
                dos_entry("qfg2"),
                amiga_entry("fatman"),
            ],
        );
        let view = CatalogView::assemble(vec![tap], vec![]);

        assert_eq!(view.by_platform(Platform::Dos).count(), 2);
        assert_eq!(view.by_platform(Platform::Amiga).count(), 1);
    }

    #[test]
    fn all_returns_stable_order_by_tap_then_id() {
        let tap = loaded_tap(
            "core",
            vec![
                amiga_entry("fatman"),
                dos_entry("qfg2"),
                dos_entry("qfg1-ega"),
            ],
        );
        let view = CatalogView::assemble(vec![tap], vec![]);

        let ids: Vec<&str> = view
            .all()
            .iter()
            .map(|e| e.catalog.game.id.as_str())
            .collect();
        assert_eq!(ids, vec!["fatman", "qfg1-ega", "qfg2"]);
    }
}
