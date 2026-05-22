# Repository Guidelines

## Project Structure & Module Organization

This repository is a documentation project plus a Rust + Tauri launcher (`classic-launcher`) for running classic DOS and Amiga games on Linux.

**Shared / top-level.** Top-level docs live in `README.md` and `docs/prerequisites.md`. Shared DOSBox configuration lives in `config/default-dosbox-staging.conf`, and shared images live in `img/`. The launcher crate and Svelte/Tauri frontend live under `launcher/` (Rust crate at `launcher/src-tauri/`, frontend at `launcher/src/`).

**DOS collections.** Organized under `dos/<game-collection>/`. Active collections are `dos/quest-for-glory/` (GOG offline installers, extracted with `innoextract`) and `dos/kings-quest/` (Steam, files copied directly from the Steam install). Both follow the same layout: user guide, per-game DOSBox `.conf` files in `config/`, per-game TOML manifests in `manifests/`, screenshots in `img/`, and gitignored source files in `installers/` (QFG) or `games/` (KQ). The `dos/quest-for-glory/scripts/` directory still holds `extract-installers.sh`.

**Amiga collection.** `amiga/amiga.md` is the guide; per-game manifests live in `amiga/manifests/` (currently `fatman.toml`; `example.toml.disabled` is the off-by-default template); FS-UAE model templates live in `amiga/config/` (`a500.fs-uae`, `a1200.fs-uae`). Amiga games launch via FS-UAE rather than DOSBox Staging.

## Build, Test, and Development Commands

The Rust launcher under `launcher/` has its own test suite (`cd launcher && cargo test`). The Markdown guides and `.conf` / manifest files have no build step. Useful validation commands:

```bash
git status --short
find . -path './.git' -prune -o -type f -print | sort
cd dos/quest-for-glory/scripts && ./extract-installers.sh
classic-launcher list
classic-launcher doctor
classic-launcher run qfg1-ega --dry-run
classic-launcher run qfg1-ega
classic-launcher run kq1sci
flatpak run io.github.dosbox-staging -conf dos/quest-for-glory/config/qfg1-ega.conf
cd launcher && cargo test
```

Use `extract-installers.sh` only after placing renamed GOG installers such as `qfg1.exe` and `qfg2.exe` in the collection’s `installers/` directory.

## Coding Style & Naming Conventions

Keep Markdown concise and task-oriented. Use relative links that match the current `dos/<collection>/` nesting, and keep guide commands synchronized with script names, config filenames, and output directories.

Games are launched via `classic-launcher run <id>` (installed from `launcher/src-tauri`). The manifest for each game lives at `dos/<collection>/manifests/<id>.toml` or `amiga/manifests/<id>.toml`. Use lowercase, hyphenated names for game directories, manifest IDs, and config files, for example `qfg1-vga` and `qfg1-vga.conf`. FluidSynth and `/usr/share/sounds/sf2/FluidR3_GM.sf2` remain the Debian-assumed MIDI path; keep that consistent in any new manifests.

## Testing Guidelines

Validate documentation changes by following the affected guide steps from a clean checkout when practical. For script or `.conf` changes, run the matching helper script or launch DOSBox Staging directly with the edited config. Confirm that screenshots and links resolve from the Markdown file where they are referenced.

## Commit & Pull Request Guidelines

Recent commits use short, imperative summaries such as `Add installation steps for QFG2` and `Move quest-for-glory-collection under dos/`. Follow that style and keep each commit focused.

Pull requests should describe the guide, script, or config changed; list any manual validation performed; and include screenshots only when installation screens or documented UI steps change. Do not commit proprietary game installers or extracted game files.
