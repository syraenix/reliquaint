# Repository Guidelines

## Redesign in progress

This project is being rebuilt as **Reliquaint** — a preservation hub for classic DOS and Amiga games — and renamed from `classic-launcher` to `reliquaint`. Read these design docs before doing non-trivial work; they define the target state every new commit is moving toward:

- `docs/prd.md` — product vision, problem, goals/non-goals, users, scope phases
- `docs/schema.md` — TOML schemas for catalog entries, install records, tap metadata, user config
- `docs/v0.1-tasks.md` — sequenced implementation tasks; the source of truth for what to work on
- `docs/adr-0001-two-layer-manifest-model.md` — split shippable catalog from per-user install records
- `docs/adr-0002-split-dosbox-config-model.md` — strip `[autoexec]` from shipped `.conf`, compose at launch
- `docs/adr-0003-tap-based-distribution.md` — community-maintainable tap repos for catalog + companion content
- `docs/adr-0004-logging-strategy.md` — `tracing` ecosystem; CLI + GUI share one instrumentation API
- `docs/adr-0005-error-handling-strategy.md` — `thiserror` in library, `anyhow` in binaries

Reading order before any task: PRD → ADR-0001 → ADR-0002 → ADR-0003 → ADR-0004 → ADR-0005 → `schema.md`.

## Project Structure & Module Organization

This repository is a documentation project plus a Rust + Tauri launcher (`reliquaint`) for running classic DOS and Amiga games on Linux.

**Shared / top-level.** Top-level docs live in `README.md` and `docs/prerequisites.md`. Shared DOSBox configuration lives in `config/default-dosbox-staging.conf`, and shared images live in `img/`. The launcher crate and Svelte/Tauri frontend live under `launcher/` (Rust crate at `launcher/src-tauri/`, frontend at `launcher/src/`).

**DOS collections.** Organized under `dos/<game-collection>/`. Active collections are `dos/quest-for-glory/` (GOG offline installers, extracted with `innoextract`) and `dos/kings-quest/` (Steam, files copied directly from the Steam install). Both follow the same layout: user guide, per-game DOSBox `.conf` files in `config/`, per-game TOML manifests in `manifests/`, screenshots in `img/`, and gitignored source files in `installers/` (QFG) or `games/` (KQ). The `dos/quest-for-glory/scripts/` directory still holds `extract-installers.sh`.

**Amiga collection.** `amiga/amiga.md` is the guide; per-game manifests live in `amiga/manifests/` (currently `fatman.toml`; `example.toml.disabled` is the off-by-default template); FS-UAE model templates live in `amiga/config/` (`a500.fs-uae`, `a1200.fs-uae`). Amiga games launch via FS-UAE rather than DOSBox Staging.

> **Migrating away from this layout.** The per-collection `manifests/` and `config/` directories are being replaced by a tap-based catalog at `tap/catalog/<platform>/` per `docs/v0.1-tasks.md` Milestone 6. Do not add new per-collection manifests; wait for the new schema to land or add directly under the new layout once it exists.

## Build, Test, and Development Commands

The Rust launcher under `launcher/` has its own test suite (`cd launcher && cargo test`). The Markdown guides and `.conf` / manifest files have no build step. Useful validation commands:

```bash
git status --short
find . -path './.git' -prune -o -type f -print | sort
cd dos/quest-for-glory/scripts && ./extract-installers.sh
reliquaint list
reliquaint doctor
reliquaint run qfg1-ega --dry-run
reliquaint run qfg1-ega
reliquaint run kq1sci
flatpak run io.github.dosbox-staging -conf dos/quest-for-glory/config/qfg1-ega.conf
cd launcher && cargo test
```

Environment overrides (rename in progress): `RELIQUAINT_REPO_ROOT` points the binary at a specific repo root; `RELIQUAINT_GAMES_DIR` overrides the default `~/games` base (the latter becomes vestigial once the install-record model from ADR-0001 lands).

Use `extract-installers.sh` only after placing renamed GOG installers such as `qfg1.exe` and `qfg2.exe` in the collection’s `installers/` directory.

## Coding Style & Naming Conventions

Keep Markdown concise and task-oriented. Use relative links that match the current `dos/<collection>/` nesting, and keep guide commands synchronized with script names, config filenames, and output directories.

Games are launched via `reliquaint run <id>` (installed from `launcher/src-tauri`). The manifest for each game lives at `dos/<collection>/manifests/<id>.toml` or `amiga/manifests/<id>.toml` (today's layout; being migrated — see "Redesign in progress" above). Use lowercase, hyphenated names for game directories, manifest IDs, and config files, for example `qfg1-vga` and `qfg1-vga.conf`. FluidSynth and `/usr/share/sounds/sf2/FluidR3_GM.sf2` remain the Debian-assumed MIDI path; keep that consistent in any new manifests.

**New backend conventions** (apply to new modules during the v0.1 redesign): use `tracing` for instrumentation per ADR-0004, and `thiserror` for library errors / `anyhow` at binary boundaries per ADR-0005. Library code does not log errors on the way up — it returns them; the binary layer decides how to surface.

## Testing Guidelines

Validate documentation changes by following the affected guide steps from a clean checkout when practical. For script or `.conf` changes, run the matching helper script or launch DOSBox Staging directly with the edited config. Confirm that screenshots and links resolve from the Markdown file where they are referenced.

## Commit & Pull Request Guidelines

Recent commits use short, imperative summaries such as `Add installation steps for QFG2` and `Move quest-for-glory-collection under dos/`. Follow that style and keep each commit focused.

Pull requests should describe the guide, script, or config changed; list any manual validation performed; and include screenshots only when installation screens or documented UI steps change. Do not commit proprietary game installers or extracted game files.
