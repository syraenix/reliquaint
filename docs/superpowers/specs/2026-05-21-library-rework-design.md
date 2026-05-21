# Design: Library Rework + Unified Install

**Date:** 2026-05-21
**Status:** Approved
**Roadmap:** roadmap-v2.md (Phases 6–7)

---

## Context

The launcher has grown through five phases from helper scripts to a full Tauri GUI. Having used it, the next generation of improvements focuses on usability:

- **Discovery** should scan `~/games` rather than relying on `expects_dir` fields in manifests — the filesystem is the truth about what's installed.
- **Installation** should accept any supported source (`.exe`, folder, `.adf`, `.hdf`, `.rp9`) through a unified flow, always writing to `~/games/{id}`.
- **Uninstall** should cleanly remove system packages and the launcher binary (not game files).
- **Cover art** should be auto-detected from the game directory on install/discovery.

Future items noted but not scoped: in-launcher game manuals/guides; custom tagging and library management.

This work becomes a new `docs/superpowers/plans/roadmap-v2.md`. The existing roadmap stays as historical record.

---

## Architecture

### Core principle: filesystem-as-state

`~/games/{id}/` is the canonical install location for all games. Manifests (in `dos/{collection}/manifests/` and `amiga/manifests/`) become pure launch configuration — they define *how* to run a game, not whether it's installed.

| Before | After |
|--------|-------|
| `[install] expects_dir` in manifest | Derived: `~/games/{id}` |
| `[ui] artwork` path in manifest | Discovered live from `~/games/{id}/` |
| Discovery walks manifest dirs | Discovery scans `~/games/`, cross-refs manifests |
| Doctor checks `expects_dir` existence | Doctor checks `~/games/{id}` existence |

A `paths::games_dir(base: &Path, id: &str) -> PathBuf` helper derives the path from the configurable base (default `~/games`).

---

## Discovery

Two-pass on every app launch and window focus:

1. Scan `~/games/` for subdirectories
2. For each subdirectory, look up a manifest whose `id` matches the directory name
3. Match found + directory non-empty → game is installed, include in library
4. Resolve artwork:
   - Priority 1: named files — `cover.png`, `cover.jpg`, `icon.png`, `icon.jpg`, `box.png`, `box.jpg`
   - Priority 2: any `.png`, `.jpg`, or `.bmp` in the directory root
5. Unmatched directories are silently ignored

**The game grid shows only installed games.** This is a library view, not a catalog.

The games base path is configurable in Settings (key: `games_base_dir`, default: `~/games`).

---

## Install flow

### Entry point

A `+` / "Add game" button opens a **catalog panel** showing all known manifests filtered to uninstalled games (i.e., no matching `~/games/{id}/` directory). From here the user initiates install.

### Collection-level install (DOS — QFG and KQ)

1. User selects a collection in the catalog panel
2. Prompted for source: `.exe` installer file (QFG) or source folder (KQ)
3. Existing extraction/copy logic runs, writing to `~/games/{id}/` for each game in the collection
4. Progress streamed to UI (existing event pattern)

### Per-game install (Amiga)

1. User selects an uninstalled game from the catalog panel
2. Prompted for source file: `.adf`, `.hdf`, or `.rp9`
3. File is copied into `~/games/{id}/` (directory created if needed)
4. `.hdf` support is added here (previously deferred)

### Post-install (both modes)

- Scan newly written `~/games/{id}/` for artwork (same priority order as discovery)
- On next discovery pass (window refocus or manual refresh), game appears in the library

---

## Uninstall flow

A dedicated "Uninstall" button in the Doctor/Settings panel.

**What it removes:**
- `pkexec apt remove fluidsynth innoextract fs-uae` (consistent with how dependency install works)
- `flatpak uninstall io.github.dosbox-staging`
- The `classic-launcher` binary (`rm $(which classic-launcher)`)

**What it does not remove:** `~/games/` and all game files remain untouched.

**Flow:**
1. Confirmation dialog listing every item to be removed
2. User confirms → removals run sequentially with streamed output (same event pattern as dependency install)
3. Launcher exits after completion

---

## Manifest schema changes

### Remove from all manifests

```toml
# REMOVE this section entirely:
[install]
expects_dir = "~/games/qfg1-ega"

# REMOVE artwork from [ui] (or remove [ui] entirely if artwork was the only field):
[ui]
artwork = "../img/qfg1-ega-cover.png"
```

Manifests affected: all files in `dos/quest-for-glory/manifests/`, `dos/kings-quest/manifests/`, and `amiga/manifests/`.

### Rust struct changes

- `manifest.rs`: drop `Install` struct; drop `Ui.artwork` field; remove validation enforcing `expects_dir` presence
- `paths.rs`: add `games_dir(base: &Path, id: &str) -> PathBuf`
- `discovery.rs`: rework to scan `~/games/`, cross-ref manifests, resolve artwork
- `commands.rs`: update `list_games`, `install_games`; add catalog listing command; add uninstall command
- `game_install.rs`: update install targets to use `games_dir()`; add `.hdf` copy support

---

## Frontend changes

| Component | Change |
|-----------|--------|
| `App.svelte` | Add catalog panel state; wire uninstall flow |
| `GameGrid.svelte` | Library-only (installed games); no change to display logic |
| `InstallPanel.svelte` | Rework: becomes catalog browser with per-game/collection install prompts |
| `CatalogPanel.svelte` | **New**: uninstalled games list + install entry point |
| `UninstallPanel.svelte` | **New** (or integrated into DoctorPanel): confirmation + streamed output |

---

## Phased delivery

### Phase 6 — Library & unified install

- Drop `expects_dir` and `[ui] artwork` from all manifests
- Rework `discovery.rs` to scan `~/games/`
- Game grid: installed-only view
- Catalog browser / "Add game" UI (`CatalogPanel.svelte`)
- Unified install flow (collection-level DOS, per-game Amiga)
- `.hdf` Amiga support
- Live artwork detection during discovery
- Rework `InstallPanel.svelte`

### Phase 7 — Uninstall & settings

- Uninstall action (removes system packages + launcher binary)
- Configurable games base path in Settings

### Future (not scoped)

- In-launcher game manuals/guides per game
- Custom tagging and library management (a state file — `~/.config/classic-launcher/state.toml` — will be needed as a foundation)

---

## Verification

- `classic-launcher list` still shows installed games after manifest changes
- Drop a folder into `~/games/{id}/`, relaunch → game appears in grid
- Install a game via the new unified flow → verify it lands in `~/games/{id}/`
- Artwork auto-detected: place `cover.png` in `~/games/{id}/`, refocus window → artwork appears in grid
- Uninstall flow: confirmation dialog shows correct items; removals run; launcher exits
- `classic-launcher doctor` updated to check `~/games/{id}` instead of `expects_dir`
- Cargo test suite passes after manifest struct changes
