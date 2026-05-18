# Repository Guidelines

## Project Structure & Module Organization

This repository is a documentation project for running classic DOS games with DOSBox Staging. Top-level docs live in `README.md` and `docs/prerequisites.md`. Shared DOSBox configuration lives in `config/default-dosbox-staging.conf`, and shared images live in `img/`.

Game-specific content is organized under `dos/<game-collection>/`. The current collection, `dos/quest-for-glory-collection/`, contains the user guide, per-game DOSBox `.conf` files in `config/`, helper scripts in `scripts/`, screenshots in `img/`, ignored installer files in `installers/`, and ignored extracted game files in `games/`.

## Build, Test, and Development Commands

There is no application build, package manifest, or automated test suite. Useful validation commands are:

```bash
git status --short
find . -path './.git' -prune -o -type f -print | sort
cd dos/quest-for-glory-collection/scripts && ./extract-installers.sh
cd dos/quest-for-glory-collection/scripts && ./qfg1-ega-run.sh
flatpak run io.github.dosbox-staging -conf dos/quest-for-glory-collection/config/qfg1-ega.conf
```

Use `extract-installers.sh` only after placing renamed GOG installers such as `qfg1.exe` and `qfg2.exe` in the collection’s `installers/` directory.

## Coding Style & Naming Conventions

Keep Markdown concise and task-oriented. Use relative links that match the current `dos/<collection>/` nesting, and keep guide commands synchronized with script names, config filenames, and output directories.

For shell scripts, use Bash, preserve executable script names like `<game>-run.sh`, and keep assumptions consistent across scripts. Current run scripts expect FluidSynth and `/usr/share/sounds/sf2/FluidR3_GM.sf2` on Debian-based Linux. Use lowercase, hyphenated names for game directories, scripts, and config files, for example `qfg1-vga-run.sh` and `qfg1-vga.conf`.

## Testing Guidelines

Validate documentation changes by following the affected guide steps from a clean checkout when practical. For script or `.conf` changes, run the matching helper script or launch DOSBox Staging directly with the edited config. Confirm that screenshots and links resolve from the Markdown file where they are referenced.

## Commit & Pull Request Guidelines

Recent commits use short, imperative summaries such as `Add installation steps for QFG2` and `Move quest-for-glory-collection under dos/`. Follow that style and keep each commit focused.

Pull requests should describe the guide, script, or config changed; list any manual validation performed; and include screenshots only when installation screens or documented UI steps change. Do not commit proprietary game installers or extracted game files.
