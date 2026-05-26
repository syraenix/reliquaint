# Collapsible Collections & Display Names

**Date:** 2026-05-26  
**Status:** Approved

## Context

The catalog browser shows every game card at once, grouped by collection. As the catalog grows (95 Amiga Forever titles already), users must scroll extensively to reach other collections. Collection section headers also render the raw kebab-case collection ID in uppercase (e.g. `SPACE-QUEST`), which is visually noisy and can't represent names with special characters like "King's Quest".

This spec covers:
1. Collapsible accordion sections per collection (default: collapsed)
2. An "expand all / collapse all" toolbar
3. A human-readable `collection_name` TOML field
4. A flat "Other" area for games without a collection

Search and filter UI are explicitly out of scope — separate future spec.

---

## Schema Change — `catalog.rs` / `docs/schema.md`

Add an optional `collection_name` field to the `[game]` TOML table, alongside the existing `collection` field:

```toml
[game]
id         = "kq1"
title      = "King's Quest: Quest for the Crown"
platform   = "dos"
collection = "kings-quest"
collection_name = "King's Quest"   # optional; omit if auto-format is correct
```

**Rules:**
- `collection_name` is optional. When absent the frontend computes the display name by splitting `collection` on `-`, title-casing each word, and joining with spaces (`"space-quest"` → `"Space Quest"`).
- All entries in the same collection may omit `collection_name` if the auto-formatted result is correct. Only entries in collections with special characters (apostrophes, mixed case, etc.) need it.
- When multiple entries in the same collection define `collection_name`, they must agree — the frontend uses the first non-null value encountered.

**Rust changes:**

`catalog.rs` — `Game` struct:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub collection_name: Option<String>,
```

`commands.rs` — `CatalogEntryDto`:
```rust
pub collection_name: Option<String>,
```
Populated in the DTO mapping from `entry.game.collection_name.clone()`.

---

## Frontend — `GameGrid.svelte`

This component is fully rewritten. Key responsibilities stay the same; the new behaviour:

### Grouping logic

```
collected  = Map<collection_id, { displayName: string, games: Game[] }>
standalone = Game[]

for each game:
  if game.collection:
    add to collected[game.collection], set displayName once (first non-null collection_name, else auto-format)
  else:
    add to standalone

sort collected by displayName (localeCompare)
sort games within each group by id (localeCompare)
sort standalone by id
```

Auto-format helper:
```javascript
function autoFormatCollection(id) {
  return id.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
}
```

### Expand/collapse state

- Stored in `localStorage` under key `"reliquaint:collection:expanded"` as a JSON object mapping `collection_id → true`.
- Missing key = collapsed (the default). Only `true` is stored; removing a key collapses it.
- State is keyed on `collection` ID, not display name — renaming via `collection_name` doesn't reset state.
- Initialised on mount; written on every toggle.

### Toolbar

Rendered as a header row inside `GameGrid` above the accordion list:

```
COLLECTIONS                              [expand all]
```

Button label logic:
- "expand all" when any collection is collapsed
- "collapse all" when all collections are expanded
- Clicking "expand all" sets all IDs to `true` in localStorage; "collapse all" clears all.

### Section headers

Each collection header row:
```
▶ Space Quest                                  6 games
▼ King's Quest                                 8 games   ← expanded
```
- Clicking anywhere on the row toggles that collection.
- `▶` = collapsed, `▼` = expanded.
- Game count shown on the right.
- When expanded, the game card grid appears directly below the header.

### Standalone "Other" area

Games without a `collection` field are shown below all accordion sections in a flat, always-visible grid with a dim "Other" label. No collapse control. Sorted by `id`. If there are no standalone games, the "Other" section is not rendered.

---

## Frontend — `FilterBar.svelte`

No changes. The toolbar ("COLLECTIONS / expand all") lives entirely inside `GameGrid`.

---

## Files Changed

| File | Change |
|------|--------|
| `launcher/src-tauri/src/catalog.rs` | Add `collection_name: Option<String>` to `Game` struct |
| `launcher/src-tauri/src/commands.rs` | Add `collection_name` to `CatalogEntryDto`, populate from `entry.game.collection_name` |
| `launcher/src/components/GameGrid.svelte` | Full rewrite — accordion sections, localStorage, toolbar, "Other" area |
| `docs/schema.md` | Document `collection_name` field in `[game]` table |
| Tap `.toml` files | No immediate changes needed — all current collection IDs auto-format correctly |

---

## Verification

1. `cd launcher && cargo test` — all tests pass (no existing tests depend on collection rendering).
2. `cd launcher && cargo build --bin reliquaint` — compiles cleanly.
3. `cd launcher && pnpm tauri dev` — open the GUI and confirm:
   - All collection sections are collapsed on first launch.
   - Clicking a section header expands/collapses it.
   - Section titles render as title case with no dashes (e.g. "Space Quest", "Quest for Glory", "Amiga Forever").
   - "expand all" button expands all; label changes to "collapse all"; clicking again collapses all.
   - Close and reopen the app — previously expanded sections stay expanded.
   - Add a test entry with `collection_name = "King's Quest"` — header renders correctly.
   - Add a test entry with no `collection` field — it appears in the "Other" grid below.
