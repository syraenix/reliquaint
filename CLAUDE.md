# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Redesign in progress

The project is being rebuilt as **Reliquaint** — a preservation hub for classic DOS and Amiga games — and renamed from `classic-launcher` to `reliquaint`. Read these design docs before doing non-trivial work; they define the target state every new commit is moving toward:

- `docs/prd.md` — product vision, problem, goals/non-goals, users, scope phases
- `docs/schema.md` — TOML schemas for catalog entries, install records, tap metadata, user config
- `docs/v0.1-tasks.md` — sequenced implementation tasks; the source of truth for what to work on next
- `docs/adr-0001-two-layer-manifest-model.md` — split shippable catalog from per-user install records
- `docs/adr-0002-split-dosbox-config-model.md` — strip `[autoexec]` from shipped `.conf`, compose at launch
- `docs/adr-0003-tap-based-distribution.md` — community-maintainable tap repos for catalog + companion content
- `docs/adr-0004-logging-strategy.md` — `tracing` ecosystem; CLI + GUI share one instrumentation API
- `docs/adr-0005-error-handling-strategy.md` — `thiserror` in library, `anyhow` in binaries

Reading order before any task: PRD → ADR-0001 → ADR-0002 → ADR-0003 → ADR-0004 → ADR-0005 → `schema.md`.

## What this repo is

A documentation repository with step-by-step Markdown guides, DOSBox Staging `.conf` files, TOML game manifests, a Rust CLI (`reliquaint`), and a Tauri 2 GUI for installing and running classic DOS and Amiga games on Debian-based Linux. Active collections: Quest for Glory 1–4, King's Quest 1–6 (DOS via DOSBox Staging), and Amiga games (via FS-UAE). The repository is being refactored into the Reliquaint launcher per the design docs above.

## Repository layout

- `README.md`, `docs/prerequisites.md` — top-level entry points. Prerequisites cover DOSBox Staging (Flatpak), FluidSynth, innoextract, FS-UAE, and the Rust toolchain.
- `config/default-dosbox-staging.conf` — reference/baseline DOSBox Staging config.
- `launcher/` — Rust workspace + Svelte/Tauri frontend for `reliquaint`.
  - `launcher/src-tauri/` — Rust crate (CLI + Tauri backend). Modules: `cli`, `commands`, `discovery`, `doctor`, `game_install` (per-collection installable-game catalog), `gui`, `installer` (runs install shell-outs and streams output), `manifest`, `paths`, `runner`, `setup` (host-dependency install actions for apt + flatpak, keyed off distro detection).
  - `launcher/src/` — Svelte frontend components.
  - `launcher/src-tauri/tauri.conf.json` — Tauri 2 configuration.
  - Tauri is always compiled in. Running with no args opens the GUI; running with a subcommand (`list`/`run`/`doctor`) stays CLI.
  - Build: `cargo build` (CLI) or `pnpm tauri dev` / `pnpm tauri build` (GUI), both from `launcher/`. Install CLI: `cargo install --path launcher/src-tauri`.
- `dos/<game-collection>/` — one directory per DOS game collection. Active: `dos/quest-for-glory/` and `dos/kings-quest/`. **Being migrated:** the per-collection `manifests/` and `config/` directories are replaced by a tap-based catalog at `tap/catalog/<platform>/` in `docs/v0.1-tasks.md` Milestone 6. Don't add new per-collection manifests. Each collection currently follows this shape:
  - `<collection>.md` — the user-facing guide; source of truth that everything else supports.
  - `manifests/<id>.toml` — per-game TOML manifest (id, title, platform, config path, sidecars, expects_dir).
  - `scripts/extract-installers.sh` — QFG only. Extracts GOG `.exe` installers into `~/games/` using `innoextract`.
  - `config/<game>.conf` — per-game DOSBox Staging config. The `[autoexec]` section mounts the game directory and launches the executable.
  - `installers/` — user drops GOG `.exe` files here (QFG only). Gitignored.
  - `games/` — present in `dos/kings-quest/` to hold Steam-sourced game files (gitignored).
  - `img/` — screenshots embedded in the guide.
- `amiga/` — Amiga collection. `amiga.md` is the guide; `manifests/<id>.toml` for each game (currently `fatman.toml`; `example.toml.disabled` is the off-by-default template); `config/a500.fs-uae` and `config/a1200.fs-uae` for model templates.

## Conventions when editing

- The guide Markdown is the product. Manifests and `.conf` files exist to make the guide reproducible — keep IDs, directory names, config paths, and guide commands in sync (e.g. if a manifest declares `expects_dir = "~/games/qfg1-ega"`, the `.conf` mount line and the guide's setup instruction must match).
- Per-game `.conf` files diverge from `config/default-dosbox-staging.conf` deliberately (e.g. `cycles=fixed N`, MIDI routing to FluidSynth via `midiconfig=128:0`, scaler choice). Don't normalize them to the default.
- Manifests assume FluidSynth's soundfont is at `/usr/share/sounds/sf2/FluidR3_GM.sf2` (the Debian `fluid-soundfont-gm` path). Keep that consistent in new manifests.
- The repo is Linux-only (Flatpak DOSBox Staging, apt-installed FluidSynth/innoextract/FS-UAE). Don't add Windows/macOS branches unless asked.
- **New backend code** uses `tracing` per ADR-0004 and `thiserror`/`anyhow` per ADR-0005 — see Tasks 1.1 and 1.2 in `docs/v0.1-tasks.md`. Library modules return `thiserror` enums and don't log errors on the way up; binary entry points (`main.rs`, `gui.rs`, Tauri commands) use `anyhow::Result` and own all user-facing error formatting.
- **Path resolution:** XDG resolution (`XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`) lives in `paths.rs` only — don't reference `.local/share` or `.config` literals from other modules.

## Common operations

### Install game files

- Extract GOG installers (after dropping `.exe` files into `dos/quest-for-glory/installers/`):
  ```bash
  cd dos/quest-for-glory/scripts
  chmod +x ./extract-installers.sh
  ./extract-installers.sh
  ```

### Launch and inspect

- Launch a game:
  ```bash
  reliquaint run qfg1-ega
  reliquaint run kq1sci
  ```
- Dry-run (preview the command without launching):
  ```bash
  reliquaint run qfg1-ega --dry-run
  ```
- List all available games:
  ```bash
  reliquaint list
  ```
- Check host dependencies and install-dir status:
  ```bash
  reliquaint doctor
  ```
- Environment overrides (rename in progress): `RELIQUAINT_REPO_ROOT` points the binary at a specific repo root; `RELIQUAINT_GAMES_DIR` overrides the default `~/games` base.
- Launch DOSBox Staging directly with a specific config (useful when iterating on a `.conf`):
  ```bash
  flatpak run io.github.dosbox-staging -conf dos/quest-for-glory/config/qfg1-ega.conf
  ```

### Develop the launcher

- Run the launcher test suite (no Node.js required):
  ```bash
  cd launcher && cargo test
  ```
- Run the GUI in development mode (requires Node.js + pnpm + Tauri system libs):
  ```bash
  cd launcher && pnpm install && pnpm tauri dev
  ```
- Build the GUI for release:
  ```bash
  cd launcher && pnpm tauri build
  ```
