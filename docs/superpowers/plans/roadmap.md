# Roadmap: Amiga collection + unified launcher

## Context

The repo currently ships Markdown guides plus per-game bash run-scripts and DOSBox Staging `.conf` files for two DOS collections (Quest for Glory, King's Quest). Each game has its own `*-run.sh` that starts FluidSynth and launches DOSBox Staging with a specific config. Helper scripts (`extract-installers.sh`, `copy-games.sh`) handle install/copy. This works, but it doesn't scale well:

- Adding a new game means adding a new run script (per-game boilerplate).
- Running games requires opening a terminal and `cd`-ing to a scripts directory.
- The host-side dependencies (DOSBox Staging flatpak, FluidSynth, soundfont, innoextract) are documented but the user has to read prose and run commands by hand.
- A new platform (Amiga) would duplicate the script-per-game pattern, doubling the maintenance surface.

The desired end state is a single Tauri-based launcher that owns the run logic, reads per-game manifests as the source of truth, supports both DOS and Amiga, and walks users through dependency setup. Existing bash scripts retire. Existing Markdown guides remain (the docs-first nature of the repo is preserved) but become "how to acquire and prepare the game files," with the launcher handling everything from "files on disk" onward.

This document is a **roadmap**, not a per-feature spec. Each phase below is sized to become its own implementation plan when we get there.

## Decisions locked in

| Question | Decision |
|---|---|
| Audience | Personal first; structure for public use later |
| Form factor | GUI windowed app |
| Stack | Tauri (Rust backend + web frontend) |
| Amiga emulator | FS-UAE |
| Amiga formats | `.adf` and `.rp9` only (skip WHDLoad/HDF for now) |
| Auto-install | Detect + guided install (per-dep "Fix this" button) |
| Scripts vs launcher | Launcher becomes the engine; bash run-scripts retire |

## Target architecture

```
classic-game-installation-guides/
├── launcher/                       ← new: Tauri app (Rust + web UI)
│   ├── src-tauri/                  Rust backend + CLI
│   └── src/                        web frontend (game grid, setup wizard)
├── dos/<collection>/
│   ├── <collection>.md             unchanged (acquisition + prep guide)
│   ├── games/<game>/game.toml      new: per-game manifest (source of truth)
│   ├── config/<game>.conf          unchanged (still consumed by manifest)
│   └── installers/                 unchanged
├── amiga/                          ← new
│   ├── amiga.md                    new guide
│   ├── games/                      gitignored .adf/.rp9 drop-zone
│   ├── manifests/<game>.toml       per-game manifest (model, kickstart, options)
│   └── config/                     FS-UAE config templates per Amiga model
└── docs/prerequisites.md           kept; eventually points users at the launcher
```

### Manifest schema (per game, both platforms)

A `game.toml` describes everything needed to launch one game:

```toml
id = "qfg1-ega"
title = "Quest for Glory 1 (EGA)"
platform = "dos"                    # "dos" | "amiga"
collection = "quest-for-glory"

[runtime]
emulator = "dosbox-staging"          # "dosbox-staging" | "fs-uae"
config = "../config/qfg1-ega.conf"   # path relative to manifest
sidecars = ["fluidsynth"]            # background processes to start first

[install]
source = "innoextract"               # how files were obtained
expects_dir = "~/games/qfg1-ega"     # mount target

[ui]
artwork = "../img/qfg1-ega-cover.png"  # optional
```

For Amiga, the same shape with platform-specific fields:

```toml
id = "shadow-of-the-beast"
platform = "amiga"
[runtime]
emulator = "fs-uae"
config = "../config/a500.fs-uae"     # model template
file = "../games/shadow-of-the-beast.adf"
```

`.rp9` files are self-describing — their manifest only needs `id`, `title`, and the file path; the launcher reads model/config from the bundle.

### Tauri app responsibilities

**Rust backend (`src-tauri/`):**
- Discover manifests by walking `dos/*/games/*/game.toml` and `amiga/manifests/*.toml`.
- Spawn processes: `fluidsynth` (if a sidecar), then `flatpak run io.github.dosbox-staging -conf <config>` or `fs-uae <config> <file>`.
- Dependency probes: `which`, `flatpak list`, file-exists checks.
- Dependency installers: invoke `pkexec apt install`, `flatpak install --user`, etc., streaming output back to the UI.
- CLI subcommand (`classic-launcher list`, `classic-launcher run <id>`, `classic-launcher doctor`) so the launcher is usable without the GUI.

**Web frontend (`src/`):**
- Game grid (filterable by platform).
- Per-game detail view: title, artwork, "Launch" button.
- Setup wizard / Doctor page: list of dependencies with status + per-row install button.
- Settings: paths, soundfont location, FS-UAE Kickstart directory.

Framework choice for the web frontend is deferred to the launcher's own implementation plan; Svelte and React are both reasonable. The Rust API surface should be small enough that the frontend choice is reversible.

## Phased delivery

### Phase 1 — Amiga collection (no launcher yet)

**Status:** Complete (commit `0b98722`).

**Why first:** Self-contained, gives you a playable second platform quickly, and lets the manifest format be validated against two emulators before the launcher commits to it. Phase 1 deliberately uses one bash-style run script (`amiga-run.sh <file>`) — a deliberately throwaway intermediate that retires in Phase 2.

**Deliverables:**
- `amiga/amiga.md` covering: install FS-UAE, where to get Kickstart ROMs (legal sources only), where to place them, supported formats, how to run.
- `amiga/config/a500.fs-uae`, `amiga/config/a1200.fs-uae` (config templates per common Amiga model).
- `amiga/scripts/amiga-run.sh <file>` — takes an `.adf` or `.rp9` path, picks the right config (or trusts `.rp9`'s embedded config), launches `fs-uae`.
- `amiga/games/.gitkeep`, `.gitignore` updated for `*.adf`, `*.rp9`, `*.hdf`.
- Top-level `README.md` updated to list the new collection.
- Update `docs/prerequisites.md` to add the FS-UAE install step.

**Verification:**
- Install FS-UAE per the new prereqs section.
- Drop a known-good `.adf` into `amiga/games/`.
- Run `./amiga/scripts/amiga-run.sh ../games/<file>.adf` — FS-UAE launches and the game boots.
- Repeat with an `.rp9`.

### Phase 2 — Manifest schema + Rust CLI engine

**Status:** Complete (commit `53c66b9`).

**Why next:** Lock the manifest contract by using it for both platforms before any UI exists. Shipping a CLI first means the engine is debuggable from the terminal and the GUI is a thin frontend over a proven core.

**Deliverables:**
- Create `launcher/` (Tauri workspace) but build only the CLI binary in this phase.
- Define and document the `game.toml` schema.
- Write manifests for every existing game (5 QFG + 6 KQ + N Amiga). Reference the existing per-game `.conf` files — they don't need to change.
- Implement `classic-launcher list`, `classic-launcher run <id>`, `classic-launcher doctor`.
- `run` reproduces today's behavior: spawn FluidSynth if listed as a sidecar, then DOSBox Staging or FS-UAE.
- **Delete** the per-game run scripts (`qfg*-run.sh`, `kq*-run.sh`, `amiga-run.sh`). Update each collection's `.md` to point users at `classic-launcher run <id>` instead.
- Keep `extract-installers.sh` and `copy-games.sh` for now (folded into the launcher in Phase 5).

**Critical files to study during implementation:**
- `dos/quest-for-glory/scripts/qfg1-ega-run.sh` and siblings — distill the launch pattern into the Rust spawn logic.
- `dos/kings-quest/scripts/copy-games.sh` — only as reference for the Phase 5 install flow; not touched in Phase 2.
- All per-game `.conf` files — referenced by manifests, unchanged.

**Verification:**
- `classic-launcher list` shows every game across DOS and Amiga collections.
- `classic-launcher run qfg1-ega` launches the same game the old `qfg1-ega-run.sh` launched. Repeat for one game in each collection.
- `classic-launcher doctor` lists each host dependency with detected status.

### Phase 3 — Tauri GUI shell

**Status:** Complete (commit `e2dbf34`).

**Deliverables:**
- Flesh out the Tauri app: pick a web framework (Svelte recommended for solo-dev simplicity), wire up the Rust commands as Tauri invokes.
- Game grid, platform filter, detail view, Launch button.
- Manifest reload on app focus (so adding files doesn't require a restart).

**Verification:**
- Launch the app, see every game from every collection.
- Click Launch on one game per platform; it runs.
- Add a new `.adf` to `amiga/games/`, refocus the window, see it appear.

### Phase 4 — Dependency wizard

**Status:** Complete (commit `222a135`).

**Deliverables:**
- "Setup" page in the GUI, sourced from the same probes that back `classic-launcher doctor`.
- Per-dependency "Fix this" button that runs the install command in a visible output pane.
- pkexec for apt; `flatpak install --user` (no pkexec needed); informational-only for things the launcher can't fix (e.g., user-supplied Kickstart ROMs).
- Distro detection minimal: Debian/Ubuntu codepath now, structured so fedora/arch can plug in later.

**Verification:**
- On a clean VM/container, the wizard shows everything as missing.
- Clicking each "Fix this" in order leaves the system able to launch a game.

### Phase 5 — Polish + install workflow

**Deliverables:**
- Fold `extract-installers.sh` into the launcher: a per-collection "Install games" flow that runs `innoextract` on a user-picked `.exe`.
- Fold `copy-games.sh` (King's Quest Steam copy flow) into the launcher.
- Per-game artwork in manifests; display in the grid.
- Optional: package the launcher (AppImage or flatpak) for public-distribution readiness.

**Verification:**
- From a fresh user perspective: install launcher → run setup wizard → install games via launcher → launch game. End-to-end without a terminal.

## Cross-cutting decisions deferred to per-phase plans

These are real questions but not blockers for the roadmap. Each becomes a question for the phase where it lands:

- Manifest file location: alongside the game folder vs. a central registry.
- Web framework for the Tauri frontend (Svelte/React/Solid).
- Where Amiga manifests live for `.rp9` bundles (auto-generated on first sight vs. user-authored).
- Whether to support per-user manifest overrides for personal tweaks.
- Packaging format for the public-release version (AppImage vs Flatpak vs both).

## What this roadmap explicitly does *not* do

- WHDLoad / `.hdf` Amiga support.
- Amiberry or other Amiga emulators.
- Non-Debian distro support in Phase 4 (structured for it, but not delivered).
- Save-state management, in-launcher controller mapping, scraping cover art automatically.
- Windows/macOS support.
