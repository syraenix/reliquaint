# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A documentation repository with step-by-step Markdown guides, DOSBox Staging `.conf` files, TOML game manifests, and a Rust CLI (`classic-launcher`) for installing and running classic DOS and Amiga games on Debian-based Linux. Active collections: Quest for Glory 1–4, King's Quest 1–6 (DOS via DOSBox Staging), and Amiga games (via FS-UAE).

## Repository layout

- `README.md`, `docs/prerequisites.md` — top-level entry points. Prerequisites cover DOSBox Staging (Flatpak), FluidSynth, innoextract, FS-UAE, and the Rust toolchain.
- `config/default-dosbox-staging.conf` — reference/baseline DOSBox Staging config.
- `launcher/` — Rust workspace for `classic-launcher`. Build with `cargo build` from `launcher/`; install with `cargo install --path launcher/src-tauri`.
- `dos/<game-collection>/` — one directory per DOS game collection. Active: `dos/quest-for-glory/` and `dos/kings-quest/`. Each follows this shape:
  - `<collection>.md` — the user-facing guide; source of truth that everything else supports.
  - `manifests/<id>.toml` — per-game TOML manifest (id, title, platform, config path, sidecars, expects_dir).
  - `scripts/extract-installers.sh` — QFG only. Extracts GOG `.exe` installers into `~/games/` using `innoextract`.
  - `config/<game>.conf` — per-game DOSBox Staging config. The `[autoexec]` section mounts the game directory and launches the executable.
  - `installers/` — user drops GOG `.exe` files here (QFG only). Gitignored.
  - `games/` — present in `dos/kings-quest/` to hold Steam-sourced game files (gitignored).
  - `img/` — screenshots embedded in the guide.
- `amiga/` — Amiga collection. `amiga.md` is the guide; `manifests/<id>.toml` for each game; `config/a500.fs-uae` and `config/a1200.fs-uae` for model templates.

## Conventions when editing

- The guide Markdown is the product. Manifests and `.conf` files exist to make the guide reproducible — keep IDs, directory names, config paths, and guide commands in sync (e.g. if a manifest declares `expects_dir = "~/games/qfg1-ega"`, the `.conf` mount line and the guide's setup instruction must match).
- Per-game `.conf` files diverge from `config/default-dosbox-staging.conf` deliberately (e.g. `cycles=fixed N`, MIDI routing to FluidSynth via `midiconfig=128:0`, scaler choice). Don't normalize them to the default.
- Manifests assume FluidSynth's soundfont is at `/usr/share/sounds/sf2/FluidR3_GM.sf2` (the Debian `fluid-soundfont-gm` path). Keep that consistent in new manifests.
- The repo is Linux-only (Flatpak DOSBox Staging, apt-installed FluidSynth/innoextract/FS-UAE). Don't add Windows/macOS branches unless asked.

## Common operations

- Extract installers (after dropping GOG `.exe` files into `dos/quest-for-glory/installers/`):
  ```bash
  cd dos/quest-for-glory/scripts
  chmod +x ./extract-installers.sh
  ./extract-installers.sh
  ```
- Launch a game:
  ```bash
  classic-launcher run qfg1-ega
  classic-launcher run kq1sci
  ```
- Dry-run (preview the command without launching):
  ```bash
  classic-launcher run qfg1-ega --dry-run
  ```
- List all available games:
  ```bash
  classic-launcher list
  ```
- Check host dependencies and install-dir status:
  ```bash
  classic-launcher doctor
  ```
- Launch DOSBox Staging directly with a specific config (useful when iterating on a `.conf`):
  ```bash
  flatpak run io.github.dosbox-staging -conf dos/quest-for-glory/config/qfg1-ega.conf
  ```
- Run the launcher test suite:
  ```bash
  cd launcher && cargo test
  ```
