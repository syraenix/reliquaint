# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A documentation repository — not an application. It contains step-by-step Markdown guides, DOSBox Staging `.conf` files, and helper bash scripts for installing and running classic DOS games (currently Quest for Glory 1–4 and King's Quest 1–6) on Debian-based Linux via DOSBox Staging. There is no build, no test suite, and no package manifest. "Work" in this repo is editing Markdown, tweaking `.conf` files, and adjusting shell scripts.

## Repository layout

- `README.md`, `docs/prerequisites.md` — top-level entry points. Prerequisites cover the host-side install of DOSBox Staging (Flatpak `io.github.dosbox-staging`), FluidSynth, and `innoextract`.
- `config/default-dosbox-staging.conf` — reference/baseline DOSBox Staging config.
- `dos/<game-collection>/` — one directory per game collection. Active collections are `dos/quest-for-glory/` and `dos/kings-quest/`. Each collection follows the same shape:
  - `<collection>.md` — the user-facing guide; the source of truth that everything else supports.
  - `scripts/extract-installers.sh` — QFG only. Extracts GOG `.exe` installers from `installers/` directly into `~/games/` using `innoextract`. The QFG1 installer ships both EGA and VGA — the script extracts to `~/games/qfg1-vga/` then moves the bundled `EGA/` subdir to `~/games/qfg1-ega/`. Expects installers renamed to `qfg1.exe`, `qfg2.exe`, etc. KQ uses Steam (no extraction step).
  - `scripts/<game>-run.sh` — launches a single game: starts `fluidsynth` with `FluidR3_GM.sf2` in the background, waits for it, then runs `flatpak run io.github.dosbox-staging -conf ../config/<game>.conf`.
  - `config/<game>.conf` — per-game DOSBox Staging config. The `[autoexec]` section mounts the game directory and launches the game executable.
  - `installers/` — user drops GOG `.exe` files here (QFG only). Gitignored.
  - `games/` — present in `dos/kings-quest/` to hold Steam-sourced game files (gitignored). The QFG collection has no `games/` dir — files extract directly to `~/games/`.
  - `img/` — screenshots embedded in the guide.

## Conventions when editing

- The guide Markdown is the product. Scripts and `.conf` files exist to make the guide reproducible — keep filenames, directory names, and command examples in the three in sync (e.g. if `extract-installers.sh` writes to `~/games/qfg1-ega/`, the `.conf` mount line and the guide's `cd` instruction must match).
- Per-game `.conf` files diverge from `config/default-dosbox-staging.conf` deliberately (e.g. `cycles=fixed N`, MIDI routing to FluidSynth via `midiconfig=128:0`, scaler choice). Don't normalize them to the default — copy the closest sibling `.conf` and adjust.
- Run scripts assume FluidSynth's soundfont is at `/usr/share/sounds/sf2/FluidR3_GM.sf2` (the Debian/Ubuntu `fluid-soundfont-gm` path). Keep that assumption consistent across new run scripts.
- The repo is Linux-only (Flatpak DOSBox Staging, apt-installed FluidSynth/innoextract). Don't add Windows/macOS branches unless the user asks.

## Common operations

- Extract installers (after dropping GOG `.exe` files into `dos/quest-for-glory/installers/`):
  ```bash
  cd dos/quest-for-glory/scripts
  chmod +x ./extract-installers.sh
  ./extract-installers.sh
  ```
- Launch a game via its run script:
  ```bash
  cd dos/quest-for-glory/scripts
  ./qfg1-ega-run.sh
  ```
- Launch DOSBox Staging directly with a specific config (useful when iterating on a `.conf`):
  ```bash
  flatpak run io.github.dosbox-staging -conf dos/quest-for-glory/config/qfg1-ega.conf
  ```
