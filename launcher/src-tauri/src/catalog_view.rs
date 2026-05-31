//! Catalog + install-record join.
//!
//! Takes the loaded taps from [`crate::tap::load_tap`] and the loaded
//! install records from [`crate::install_record::load_all`] and produces
//! a single queryable view that the CLI (Milestone 4) and GUI
//! (Milestone 5) both consume.
//!
//! Joined by `(tap_id, game_id)`. Records that don't match any catalog
//! entry land in [`CatalogView::orphans`].
//!
//! Multi-tap priority resolution (v0.3): when more than one tap supplies
//! the same `game_id`, lower `priority` values take precedence.
//! [`CatalogView::by_game_id`] returns the winner; [`CatalogView::all_with_game_id`]
//! returns all candidates sorted by priority; [`CatalogView::by_tap_and_id`]
//! performs an exact `(tap_id, game_id)` lookup regardless of priority.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::catalog::{CatalogEntry, Platform};
use crate::companion::{self, CompanionItem};
use crate::install_record::{InstallRecord, LoadedInstallRecord};
use crate::tap::LoadedTap;

#[derive(Debug, Clone)]
pub struct CatalogView {
    entries: Vec<CatalogViewEntry>,
    orphans: Vec<LoadedInstallRecord>,
    /// Companion content (walkthroughs, maps, hints) discovered across all
    /// taps, in aggregation order: grouped by tap in `(priority, tap_id)`
    /// order, each tap's items in `companion::index_companion` discovery
    /// order. Queried via [`Self::companion_for`].
    companion: Vec<CompanionItem>,
}

#[derive(Debug, Clone)]
pub struct CatalogViewEntry {
    pub tap_id: String,
    pub source_path: PathBuf,
    pub catalog: CatalogEntry,
    pub install: Option<InstallRecord>,
    /// Priority of the tap that contributed this entry.
    ///
    /// Lower value = higher precedence. Convention:
    /// - Local user tap: `-1`
    /// - Subscribed taps: their integer priority from the subscription manifest
    /// - Bundled tap: `i32::MAX - 1`
    /// - Taps not in the priorities map: `i32::MAX`
    pub priority: i32,
}

impl CatalogView {
    /// Build a view by joining loaded taps and install records on
    /// `(tap_id, game_id)`, using the supplied priority map to resolve
    /// conflicts when more than one tap provides the same `game_id`.
    ///
    /// `priorities` maps tap_id → priority value. Taps not in the map
    /// receive priority `i32::MAX`.  Within the same `game_id`, the entry
    /// whose tap has the lowest priority value is the "winner" returned by
    /// [`Self::by_game_id`]; all entries (winner and losers) are kept and
    /// accessible via [`Self::all_with_game_id`] and [`Self::by_tap_and_id`].
    ///
    /// Install records are still joined on `(tap_id, game_id)` — a record
    /// that references a specific tap is attached to that tap's entry only,
    /// not silently promoted to the winner.
    ///
    /// Stable ordering: entries are sorted by `(priority, tap_id, game_id)`
    /// so iteration is deterministic across runs.
    pub fn assemble_with_priorities(
        taps: Vec<LoadedTap>,
        installs: Vec<LoadedInstallRecord>,
        priorities: HashMap<String, i32>,
    ) -> Self {
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
        // Per-tap companion groups, tagged with the tap's sort key so the
        // flattened view is grouped by tap in (priority, tap_id) order while
        // preserving each tap's internal discovery order.
        let mut companion_groups: Vec<(i32, String, Vec<CompanionItem>)> = Vec::new();

        for tap in taps {
            let tap_id = tap.metadata.id.clone();
            let priority = priorities.get(&tap_id).copied().unwrap_or(i32::MAX);
            let companion_items = companion::index_companion(&tap.root, &tap_id);
            if !companion_items.is_empty() {
                companion_groups.push((priority, tap_id.clone(), companion_items));
            }
            for (game_id, loaded_entry) in tap.entries {
                let key = (tap_id.clone(), game_id.clone());
                let install = install_by_key.remove(&key).map(|li| li.record);
                entries.push(CatalogViewEntry {
                    tap_id: tap_id.clone(),
                    source_path: loaded_entry.source_path,
                    catalog: loaded_entry.entry,
                    install,
                    priority,
                });
            }
        }

        // Group companion content by tap in (priority, tap_id) order (Task 1.3),
        // then flatten while keeping each tap's discovery order intact.
        companion_groups.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let companion: Vec<CompanionItem> = companion_groups
            .into_iter()
            .flat_map(|(_, _, items)| items)
            .collect();

        // Stable order: by (priority, tap_id, game_id).
        entries.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.tap_id.cmp(&b.tap_id))
                .then_with(|| a.catalog.game.id.cmp(&b.catalog.game.id))
        });

        let mut orphans: Vec<LoadedInstallRecord> = install_by_key.into_values().collect();
        orphans.sort_by(|a, b| a.source_path.cmp(&b.source_path));

        Self {
            entries,
            orphans,
            companion,
        }
    }

    /// Build a view by joining loaded taps and install records on
    /// `(tap_id, game_id)`.
    ///
    /// This is a convenience wrapper around [`Self::assemble_with_priorities`]
    /// that uses an empty priorities map, giving every tap the same default
    /// priority (`i32::MAX`). All existing call sites remain unchanged.
    pub fn assemble(taps: Vec<LoadedTap>, installs: Vec<LoadedInstallRecord>) -> Self {
        Self::assemble_with_priorities(taps, installs, HashMap::new())
    }

    /// Every catalog entry, installed or not, in stable
    /// `(priority, tap_id, game_id)` order.
    pub fn all(&self) -> &[CatalogViewEntry] {
        &self.entries
    }

    /// Return the priority-winning entry for `game_id` (the entry with the
    /// lowest `priority` value among all entries that share the id).
    ///
    /// When only one tap provides `game_id`, that entry is returned directly.
    /// When multiple taps provide it, the winner (lowest priority value) is
    /// returned; use [`Self::all_with_game_id`] to iterate all candidates.
    pub fn by_game_id(&self, game_id: &str) -> Option<&CatalogViewEntry> {
        // entries are sorted by (priority, tap_id, game_id), so the first
        // match is already the winner.
        self.entries.iter().find(|e| e.catalog.game.id == game_id)
    }

    /// Return all entries with the given `game_id`, sorted by priority
    /// ascending (lowest = highest precedence first).
    pub fn all_with_game_id(&self, game_id: &str) -> Vec<&CatalogViewEntry> {
        self.entries
            .iter()
            .filter(|e| e.catalog.game.id == game_id)
            .collect()
    }

    /// Exact lookup by both `tap_id` and `game_id`.
    ///
    /// Used when an install record or command targets a specific
    /// `(tap_id, game_id)` pair, bypassing priority resolution.
    pub fn by_tap_and_id(&self, tap_id: &str, game_id: &str) -> Option<&CatalogViewEntry> {
        self.entries
            .iter()
            .find(|e| e.tap_id == tap_id && e.catalog.game.id == game_id)
    }

    /// Look up the priority-winning entry by game id.
    ///
    /// Delegates to [`Self::by_game_id`]. Kept for backwards compatibility
    /// with existing call sites that pre-date multi-tap support.
    pub fn by_id(&self, id: &str) -> Option<&CatalogViewEntry> {
        self.by_game_id(id)
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

    /// Companion content for `game_id`, aggregated across every tap that
    /// ships it. Items are returned in `(tap-priority, tap-file)` order and
    /// each retains its `tap_id` for attribution. Empty when no tap carries
    /// companion content for the game.
    pub fn companion_for(&self, game_id: &str) -> Vec<&CompanionItem> {
        self.companion
            .iter()
            .filter(|c| c.game_id == game_id)
            .collect()
    }

    /// Every companion item across all taps, in aggregation order. Unlike
    /// [`Self::companion_for`], this includes items whose `game_id` matches no
    /// catalog entry — used by the doctor to flag stray companion directories.
    pub fn all_companion(&self) -> &[CompanionItem] {
        &self.companion
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

    // ---------------------------------------------------------------------------
    // priority / multi-tap tests (M3.1)
    // ---------------------------------------------------------------------------

    #[test]
    fn assemble_with_priorities_lower_priority_value_wins_by_game_id() {
        // tap-a has priority 0 (higher precedence), tap-b has priority 1
        let tap_a = loaded_tap("tap-a", vec![dos_entry("qfg1-ega")]);
        let tap_b = loaded_tap("tap-b", vec![dos_entry("qfg1-ega")]);

        let mut priorities = HashMap::new();
        priorities.insert("tap-a".to_string(), 0_i32);
        priorities.insert("tap-b".to_string(), 1_i32);

        let view = CatalogView::assemble_with_priorities(vec![tap_a, tap_b], vec![], priorities);

        let winner = view.by_game_id("qfg1-ega").expect("should find qfg1-ega");
        assert_eq!(winner.tap_id, "tap-a", "lower priority value should win");
        assert_eq!(winner.priority, 0);
    }

    #[test]
    fn all_with_game_id_returns_all_sorted_by_priority() {
        let tap_a = loaded_tap("tap-a", vec![dos_entry("qfg1-ega")]);
        let tap_b = loaded_tap("tap-b", vec![dos_entry("qfg1-ega")]);

        let mut priorities = HashMap::new();
        priorities.insert("tap-a".to_string(), 0_i32);
        priorities.insert("tap-b".to_string(), 1_i32);

        let view = CatalogView::assemble_with_priorities(vec![tap_a, tap_b], vec![], priorities);

        let all = view.all_with_game_id("qfg1-ega");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].tap_id, "tap-a", "priority 0 entry should come first");
        assert_eq!(
            all[1].tap_id, "tap-b",
            "priority 1 entry should come second"
        );
    }

    #[test]
    fn by_tap_and_id_returns_exact_match_regardless_of_priority() {
        let tap_a = loaded_tap("tap-a", vec![dos_entry("qfg1-ega")]);
        let tap_b = loaded_tap("tap-b", vec![dos_entry("qfg1-ega")]);

        let mut priorities = HashMap::new();
        priorities.insert("tap-a".to_string(), 0_i32);
        priorities.insert("tap-b".to_string(), 1_i32);

        let view = CatalogView::assemble_with_priorities(vec![tap_a, tap_b], vec![], priorities);

        let entry = view
            .by_tap_and_id("tap-b", "qfg1-ega")
            .expect("tap-b should be findable even though tap-a wins");
        assert_eq!(entry.tap_id, "tap-b");
    }

    #[test]
    fn assemble_still_works_as_single_tap_view() {
        // assemble() keeps working unchanged; by_game_id returns the sole entry.
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega"), dos_entry("kq5")]);
        let view = CatalogView::assemble(vec![tap], vec![]);

        let e = view.by_game_id("qfg1-ega").expect("should find qfg1-ega");
        assert_eq!(e.tap_id, "core");
        assert!(view.by_game_id("nonexistent").is_none());
    }

    #[test]
    fn by_game_id_returns_none_for_unknown_id() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);
        assert!(view.by_game_id("no-such-game").is_none());
    }

    #[test]
    fn all_with_game_id_returns_empty_for_unknown_id() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);
        assert!(view.all_with_game_id("no-such-game").is_empty());
    }

    #[test]
    fn by_tap_and_id_returns_none_for_unknown_tap() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);
        assert!(view.by_tap_and_id("other-tap", "qfg1-ega").is_none());
    }

    #[test]
    fn stable_ordering_is_priority_then_tap_id_then_game_id() {
        // Three taps: tap-z priority 0, tap-a priority 1, tap-m priority 1.
        // Within same priority, tap_id breaks ties.
        let tap_z = loaded_tap("tap-z", vec![dos_entry("game-b"), dos_entry("game-a")]);
        let tap_a = loaded_tap("tap-a", vec![dos_entry("game-c")]);
        let tap_m = loaded_tap("tap-m", vec![dos_entry("game-d")]);

        let mut priorities = HashMap::new();
        priorities.insert("tap-z".to_string(), 0_i32);
        priorities.insert("tap-a".to_string(), 1_i32);
        priorities.insert("tap-m".to_string(), 1_i32);

        let view =
            CatalogView::assemble_with_priorities(vec![tap_z, tap_a, tap_m], vec![], priorities);

        let ids: Vec<(&str, &str)> = view
            .all()
            .iter()
            .map(|e| (e.tap_id.as_str(), e.catalog.game.id.as_str()))
            .collect();

        // priority 0: tap-z → game-a, game-b (game_id ascending)
        // priority 1: tap-a → game-c, then tap-m → game-d (tap_id ascending)
        assert_eq!(
            ids,
            vec![
                ("tap-z", "game-a"),
                ("tap-z", "game-b"),
                ("tap-a", "game-c"),
                ("tap-m", "game-d"),
            ]
        );
    }

    #[test]
    fn priority_field_is_set_on_entries() {
        let tap = loaded_tap("core", vec![dos_entry("qfg1-ega")]);
        let mut priorities = HashMap::new();
        priorities.insert("core".to_string(), 42_i32);
        let view = CatalogView::assemble_with_priorities(vec![tap], vec![], priorities);
        assert_eq!(view.all()[0].priority, 42);
    }

    #[test]
    fn unmapped_tap_gets_i32_max_priority() {
        // A tap not in the priorities map should receive i32::MAX priority.
        let tap_mapped = loaded_tap("mapped", vec![dos_entry("game-x")]);
        let tap_unmapped = loaded_tap("unmapped", vec![dos_entry("game-y")]);

        let mut priorities = HashMap::new();
        priorities.insert("mapped".to_string(), 5_i32);

        let view = CatalogView::assemble_with_priorities(
            vec![tap_mapped, tap_unmapped],
            vec![],
            priorities,
        );

        let unmapped_entry = view
            .by_tap_and_id("unmapped", "game-y")
            .expect("should find unmapped entry");
        assert_eq!(unmapped_entry.priority, i32::MAX);
    }

    #[test]
    fn install_record_joins_to_priority_winner() {
        // Install record references "tap-a", which is the higher-priority tap.
        let tap_a = loaded_tap("tap-a", vec![dos_entry("qfg1-ega")]);
        let tap_b = loaded_tap("tap-b", vec![dos_entry("qfg1-ega")]);
        let installs = vec![loaded_install("tap-a", "qfg1-ega")];

        let mut priorities = HashMap::new();
        priorities.insert("tap-a".to_string(), 0_i32);
        priorities.insert("tap-b".to_string(), 1_i32);

        let view = CatalogView::assemble_with_priorities(vec![tap_a, tap_b], installs, priorities);

        let winner = view.by_game_id("qfg1-ega").unwrap();
        assert!(
            winner.install.is_some(),
            "install should be joined to tap-a entry"
        );

        // tap-b entry has no install
        let loser = view.by_tap_and_id("tap-b", "qfg1-ega").unwrap();
        assert!(loser.install.is_none());
    }

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
                collection_name: None,
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
                collection_name: None,
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

    // ---------------------------------------------------------------------------
    // companion content aggregation (v0.4 Milestone 1, Tasks 1.2 + 1.3)
    // ---------------------------------------------------------------------------

    /// Build a `LoadedTap` rooted at a real on-disk path so `index_companion`
    /// (which walks `<root>/companion/`) sees actual files.
    fn loaded_tap_at(
        root: &std::path::Path,
        tap_id: &str,
        entries: Vec<CatalogEntry>,
    ) -> LoadedTap {
        let mut map = BTreeMap::new();
        for entry in entries {
            let game_id = entry.game.id.clone();
            map.insert(
                game_id.clone(),
                LoadedEntry {
                    source_path: root.join(format!("catalog/{game_id}.toml")),
                    entry,
                },
            );
        }
        LoadedTap {
            metadata: tap_metadata(tap_id),
            root: root.to_path_buf(),
            entries: map,
        }
    }

    /// Write a companion file under `<root>/companion/<game_id>/<rel>`.
    fn write_companion(root: &std::path::Path, game_id: &str, rel: &str, contents: &str) {
        let path = root.join("companion").join(game_id).join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn companion_for_aggregates_across_taps_in_priority_order() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        write_companion(dir_a.path(), "qfg1-ega", "guide.md", "# a");
        write_companion(dir_b.path(), "qfg1-ega", "guide.md", "# b");

        let tap_a = loaded_tap_at(dir_a.path(), "tap-a", vec![dos_entry("qfg1-ega")]);
        let tap_b = loaded_tap_at(dir_b.path(), "tap-b", vec![dos_entry("qfg1-ega")]);

        let mut priorities = HashMap::new();
        priorities.insert("tap-a".to_string(), 0_i32);
        priorities.insert("tap-b".to_string(), 1_i32);

        let view = CatalogView::assemble_with_priorities(vec![tap_b, tap_a], vec![], priorities);

        let items = view.companion_for("qfg1-ega");
        assert_eq!(items.len(), 2, "both taps contribute: {items:?}");
        // priority 0 (tap-a) groups first, regardless of tap order passed in.
        assert_eq!(items[0].tap_id, "tap-a");
        assert_eq!(items[1].tap_id, "tap-b");
        // Each item retains correct attribution.
        assert!(items.iter().all(|i| i.game_id == "qfg1-ega"));
    }

    #[test]
    fn companion_for_preserves_within_tap_file_order() {
        let dir = tempfile::tempdir().unwrap();
        write_companion(dir.path(), "qfg1-ega", "02-walkthrough.md", "# wt");
        write_companion(dir.path(), "qfg1-ega", "01-overview.md", "# ov");
        write_companion(dir.path(), "qfg1-ega", "appendix.md", "# ap");

        let tap = loaded_tap_at(dir.path(), "core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);

        let titles: Vec<&str> = view
            .companion_for("qfg1-ega")
            .iter()
            .map(|i| i.title.as_str())
            .collect();
        // Numeric-prefixed first (01, 02), then alpha — discovery order preserved.
        assert_eq!(titles, vec!["Overview", "Walkthrough", "Appendix"]);
    }

    #[test]
    fn companion_for_is_empty_when_no_content() {
        let dir = tempfile::tempdir().unwrap();
        let tap = loaded_tap_at(dir.path(), "core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);
        assert!(view.companion_for("qfg1-ega").is_empty());
        assert!(view.companion_for("no-such-game").is_empty());
    }

    #[test]
    fn all_companion_includes_items_for_games_without_catalog_entries() {
        let dir = tempfile::tempdir().unwrap();
        // Companion content for a game in the catalog AND one that isn't.
        write_companion(dir.path(), "qfg1-ega", "guide.md", "# g");
        write_companion(dir.path(), "ghost-game", "notes.md", "# n");

        // Catalog only knows qfg1-ega.
        let tap = loaded_tap_at(dir.path(), "core", vec![dos_entry("qfg1-ega")]);
        let view = CatalogView::assemble(vec![tap], vec![]);

        let game_ids: Vec<&str> = view
            .all_companion()
            .iter()
            .map(|c| c.game_id.as_str())
            .collect();
        assert!(game_ids.contains(&"qfg1-ega"));
        assert!(
            game_ids.contains(&"ghost-game"),
            "all_companion must surface content for games absent from the catalog: {game_ids:?}"
        );
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
