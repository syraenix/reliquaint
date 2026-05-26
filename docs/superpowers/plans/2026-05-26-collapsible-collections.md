# Collapsible Collections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add collapsible accordion sections to the game catalog browser, a human-readable `collection_name` TOML field, and a flat "Other" area for uncollected games.

**Architecture:** A new optional `collection_name` field is added to the Rust `Game` struct and passed through `CatalogEntryDto` to the frontend. `GameGrid.svelte` is fully rewritten to group games into collapsible accordion sections (keyed on `collection` ID, display name from `collection_name` or auto-formatted from the ID), with expand/collapse state persisted in `localStorage`. Games without a `collection` field appear in a flat always-visible "Other" area below the accordions.

**Tech Stack:** Rust/Serde (TOML parsing), Tauri IPC, Svelte 3, browser `localStorage`

**Spec:** `docs/superpowers/specs/2026-05-26-collapsible-collections-design.md`

---

### Task 1: Add `collection_name` to Rust catalog schema

**Files:**
- Modify: `launcher/src-tauri/src/catalog.rs` (Game struct ~line 23, tests ~line 360)

- [ ] **Step 1: Write the failing test**

  Add this test inside the `#[cfg(test)] mod tests` block in `catalog.rs`, after the existing tests:

  ```rust
  #[test]
  fn parses_collection_name() {
      let text = r#"
  schema_version = 1
  [game]
  id = "kq1"
  title = "King's Quest"
  platform = "dos"
  collection = "kings-quest"
  collection_name = "King's Quest"
  [runtime]
  emulator = "dosbox-staging"
  [runtime.dosbox]
  config = "kq1.conf"
  entry = "KQ.BAT"
  "#;
      let entry = parse_str(text, Path::new("kq1.toml")).unwrap();
      assert_eq!(entry.game.collection_name, Some("King's Quest".to_string()));
  }

  #[test]
  fn collection_name_defaults_to_none() {
      let text = r#"
  schema_version = 1
  [game]
  id = "kq1"
  title = "King's Quest"
  platform = "dos"
  collection = "kings-quest"
  [runtime]
  emulator = "dosbox-staging"
  [runtime.dosbox]
  config = "kq1.conf"
  entry = "KQ.BAT"
  "#;
      let entry = parse_str(text, Path::new("kq1.toml")).unwrap();
      assert_eq!(entry.game.collection_name, None);
  }
  ```

- [ ] **Step 2: Run the tests to confirm they fail**

  ```bash
  cd launcher && cargo test parses_collection_name collection_name_defaults_to_none -- --nocapture 2>&1 | tail -20
  ```

  Expected: compile error — `collection_name` does not exist on `Game`.

- [ ] **Step 3: Add `collection_name` to the `Game` struct**

  In `catalog.rs`, the `Game` struct currently ends at line ~29:

  ```rust
  pub struct Game {
      pub id: String,
      pub title: String,
      pub platform: Platform,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub collection: Option<String>,
  }
  ```

  Add `collection_name` after `collection`:

  ```rust
  pub struct Game {
      pub id: String,
      pub title: String,
      pub platform: Platform,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub collection: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub collection_name: Option<String>,
  }
  ```

- [ ] **Step 4: Run the tests to confirm they pass**

  ```bash
  cd launcher && cargo test parses_collection_name collection_name_defaults_to_none -- --nocapture 2>&1 | tail -20
  ```

  Expected: both tests PASS.

- [ ] **Step 5: Run the full test suite**

  ```bash
  cd launcher && cargo test 2>&1 | tail -20
  ```

  Expected: all tests pass. The round-trip tests (`round_trips_dos_fixture`, `round_trips_amiga_fixture`) must still pass — `skip_serializing_if = "Option::is_none"` ensures `collection_name` is omitted from serialized output when absent.

- [ ] **Step 6: Commit**

  ```bash
  git add launcher/src-tauri/src/catalog.rs
  git commit -m "feat(catalog): add optional collection_name field to Game struct"
  ```

---

### Task 2: Pass `collection_name` through the DTO

**Files:**
- Modify: `launcher/src-tauri/src/commands.rs` (`CatalogEntryDto` ~line 51, `entry_to_dto` ~line 104)

- [ ] **Step 1: Add `collection_name` to `CatalogEntryDto`**

  The struct currently has `collection: Option<String>` at ~line 55. Add `collection_name` directly after it:

  ```rust
  pub collection: Option<String>,
  pub collection_name: Option<String>,
  ```

- [ ] **Step 2: Populate `collection_name` in `entry_to_dto`**

  In the `entry_to_dto` function (~line 104), the current mapping has:

  ```rust
  collection: e.catalog.game.collection.clone(),
  ```

  Add `collection_name` on the next line:

  ```rust
  collection: e.catalog.game.collection.clone(),
  collection_name: e.catalog.game.collection_name.clone(),
  ```

- [ ] **Step 3: Run the full test suite**

  ```bash
  cd launcher && cargo test 2>&1 | tail -20
  ```

  Expected: all tests pass.

- [ ] **Step 4: Commit**

  ```bash
  git add launcher/src-tauri/src/commands.rs
  git commit -m "feat(commands): pass collection_name through CatalogEntryDto"
  ```

---

### Task 3: Rewrite GameGrid.svelte

**Files:**
- Modify: `launcher/src/components/GameGrid.svelte`

The component is small enough to replace in full. The new version implements:
- `autoFormatCollection(id)` — splits on `-`, title-cases each word
- Reactive `collected` map and `standalone` array
- `expanded` state loaded from `localStorage` on mount, written on every change
- Accordion sections with ▶/▼ toggle
- "COLLECTIONS / expand all" toolbar
- Flat "Other" section for standalone games

- [ ] **Step 1: Replace `GameGrid.svelte` with the new implementation**

  Full content:

  ```svelte
  <script>
    import { onMount } from "svelte";
    import { createEventDispatcher } from "svelte";
    import GameCard from "./GameCard.svelte";

    export let games = [];
    const dispatch = createEventDispatcher();

    const STORAGE_KEY = "reliquaint:collection:expanded";

    let expanded = {};

    onMount(() => {
      try {
        const raw = localStorage.getItem(STORAGE_KEY);
        expanded = raw ? JSON.parse(raw) : {};
      } catch {
        expanded = {};
      }
    });

    function saveExpanded() {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(expanded));
      } catch { /* storage unavailable */ }
    }

    function autoFormatCollection(id) {
      return id.split("-").map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
    }

    $: collected = (() => {
      const map = new Map();
      for (const g of games) {
        if (!g.collection) continue;
        if (!map.has(g.collection)) {
          const displayName = g.collection_name ?? autoFormatCollection(g.collection);
          map.set(g.collection, { displayName, games: [] });
        }
        map.get(g.collection).games.push(g);
      }
      const sorted = Array.from(map.entries()).sort((a, b) =>
        a[1].displayName.localeCompare(b[1].displayName)
      );
      for (const [, group] of sorted) {
        group.games.sort((a, b) => a.id.localeCompare(b.id));
      }
      return sorted;
    })();

    $: standalone = [...games]
      .filter((g) => !g.collection)
      .sort((a, b) => a.id.localeCompare(b.id));

    $: allExpanded =
      collected.length > 0 && collected.every(([id]) => expanded[id]);

    function toggleCollection(id) {
      const next = { ...expanded };
      if (next[id]) {
        delete next[id];
      } else {
        next[id] = true;
      }
      expanded = next;
      saveExpanded();
    }

    function expandAll() {
      const next = {};
      for (const [id] of collected) next[id] = true;
      expanded = next;
      saveExpanded();
    }

    function collapseAll() {
      expanded = {};
      saveExpanded();
    }
  </script>

  <div class="grid-container">
    {#if games.length === 0}
      <div class="empty">No games match this filter.</div>
    {:else}
      {#if collected.length > 0}
        <div class="collection-toolbar">
          <span class="toolbar-label">Collections</span>
          <button class="toolbar-btn" on:click={allExpanded ? collapseAll : expandAll}>
            {allExpanded ? "collapse all" : "expand all"}
          </button>
        </div>

        {#each collected as [id, group] (id)}
          <section class="collection">
            <button class="collection-header" on:click={() => toggleCollection(id)}>
              <span class="toggle-icon">{expanded[id] ? "▼" : "▶"}</span>
              <span class="collection-name">{group.displayName}</span>
              <span class="game-count"
                >{group.games.length}
                {group.games.length === 1 ? "game" : "games"}</span
              >
            </button>
            {#if expanded[id]}
              <div class="grid">
                {#each group.games as game (game.id)}
                  <GameCard {game} on:click={() => dispatch("select", game)} />
                {/each}
              </div>
            {/if}
          </section>
        {/each}
      {/if}

      {#if standalone.length > 0}
        <section class="other-section">
          <div class="other-label">Other</div>
          <div class="grid">
            {#each standalone as game (game.id)}
              <GameCard {game} on:click={() => dispatch("select", game)} />
            {/each}
          </div>
        </section>
      {/if}
    {/if}
  </div>

  <style>
    .grid-container {
      flex: 1;
      overflow-y: auto;
      padding: 20px;
    }

    .collection-toolbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 6px 2px;
      margin-bottom: 4px;
      border-bottom: 1px solid #2a2a40;
    }

    .toolbar-label {
      font-size: 0.7rem;
      font-weight: 600;
      color: #555;
      text-transform: uppercase;
      letter-spacing: 0.1em;
    }

    .toolbar-btn {
      background: none;
      border: 1px solid #3a3a55;
      color: #888;
      padding: 2px 10px;
      border-radius: 4px;
      cursor: pointer;
      font-size: 0.75rem;
      transition: color 0.15s, border-color 0.15s;
    }

    .toolbar-btn:hover {
      color: #ccc;
      border-color: #5555aa;
    }

    .collection {
      margin-bottom: 4px;
    }

    .collection-header {
      display: flex;
      align-items: center;
      width: 100%;
      padding: 10px 2px;
      background: none;
      border: none;
      border-bottom: 1px solid #2a2a40;
      cursor: pointer;
      text-align: left;
      gap: 8px;
    }

    .collection-header:hover .collection-name {
      color: #c0c8ff;
    }

    .toggle-icon {
      font-size: 0.65rem;
      color: #a0a8ff;
      width: 12px;
      flex-shrink: 0;
    }

    .collection-name {
      flex: 1;
      font-size: 0.9rem;
      font-weight: 600;
      color: #a0a8ff;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      transition: color 0.15s;
    }

    .game-count {
      font-size: 0.75rem;
      color: #555;
    }

    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 14px;
      padding: 12px 0 16px;
    }

    .other-section {
      margin-top: 20px;
    }

    .other-label {
      font-size: 0.75rem;
      color: #555;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 10px;
      padding-bottom: 6px;
      border-bottom: 1px solid #2a2a40;
    }

    .empty {
      display: flex;
      align-items: center;
      justify-content: center;
      height: 200px;
      color: #555;
      font-size: 0.95rem;
    }
  </style>
  ```

- [ ] **Step 2: Build to catch any compile errors**

  ```bash
  cd launcher && pnpm build 2>&1 | tail -30
  ```

  Expected: build succeeds with no errors.

- [ ] **Step 3: Launch the dev server and verify manually**

  ```bash
  cd launcher && pnpm tauri dev
  ```

  Check each item:
  - All collection sections are collapsed on first launch (only headers visible)
  - Section titles are title-case with no dashes: "Space Quest", "Quest for Glory", "Amiga Forever"
  - "Collections / expand all" toolbar appears above the section list
  - Clicking a section header expands it, showing the game card grid; clicking again collapses it
  - "expand all" expands all sections; button label changes to "collapse all"
  - "collapse all" collapses all sections; button label changes back to "expand all"
  - Close and reopen the app — previously expanded sections remain expanded

- [ ] **Step 4: Commit**

  ```bash
  git add launcher/src/components/GameGrid.svelte
  git commit -m "feat(ui): collapsible collection sections with localStorage persistence"
  ```

---

### Task 4: Update docs/schema.md

**Files:**
- Modify: `docs/schema.md` (`[game]` table ~line 141)

- [ ] **Step 1: Add `collection_name` row to the `[game]` table**

  The current table ends with:

  ```markdown
  | `collection` | string | no | A group key for related games (e.g. `quest-for-glory`). Free-form identifier following the id rules. Pure UI grouping; no semantic effect. |
  ```

  Add a new row directly after it:

  ```markdown
  | `collection_name` | string | no | Human-readable display name for the collection (e.g. `"King's Quest"`). Optional; if absent the UI auto-formats `collection` by replacing hyphens with spaces and title-casing each word. Only needed when the formatted ID would be incorrect (e.g. apostrophes, mixed case). |
  ```

- [ ] **Step 2: Update the skeleton example to show the new field**

  In the common skeleton example (~line 114), the current `[game]` block is:

  ```toml
  [game]
  id       = "qfg1-ega"
  title    = "Quest for Glory I: So You Want to Be a Hero (EGA)"
  platform = "dos"
  collection = "quest-for-glory"   # optional; groups related games in the UI
  ```

  Add a commented-out `collection_name` line to show when it's used:

  ```toml
  [game]
  id       = "qfg1-ega"
  title    = "Quest for Glory I: So You Want to Be a Hero (EGA)"
  platform = "dos"
  collection = "quest-for-glory"        # optional; groups related games in the UI
  # collection_name = "Quest for Glory"  # omit when auto-format is correct
  ```

- [ ] **Step 3: Commit**

  ```bash
  git add docs/schema.md
  git commit -m "docs(schema): document collection_name field in [game] table"
  ```

---

## Verification Checklist

After all tasks are complete:

```bash
cd launcher && cargo test
```
Expected: all tests pass (174+ total).

```bash
cd launcher && cargo build --bin reliquaint
```
Expected: clean compile.

Manual GUI checks via `pnpm tauri dev`:
- [ ] Collections collapsed on first launch
- [ ] Section titles: "Space Quest", "Quest for Glory", "Amiga Forever" (no dashes)
- [ ] Click header → expands; click again → collapses
- [ ] "expand all" expands everything, label changes to "collapse all"
- [ ] "collapse all" collapses everything, label changes to "expand all"
- [ ] State persists after closing and reopening the app
- [ ] Games without `collection` appear in "Other" (test by temporarily removing `collection` from a fixture entry)
- [ ] A `collection_name = "King's Quest"` in a tap entry renders correctly as the section header
