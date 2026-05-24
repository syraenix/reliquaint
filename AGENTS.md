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

This repository is a Rust + Tauri launcher (`reliquaint`) for running classic DOS and Amiga games on Linux, plus accompanying documentation and a bundled tap of catalog entries (the latter populated in Milestone 6).

**Top-level.** `README.md` and `docs/prerequisites.md` cover the front door; `docs/` carries the design docs listed in "Redesign in progress" above. Shared DOSBox baseline at `config/default-dosbox-staging.conf`. Shared images in `img/`. The launcher crate and Svelte/Tauri frontend live under `launcher/` (Rust crate at `launcher/src-tauri/`, frontend at `launcher/src/`).

**Bundled tap (post-Milestone-6).** `tap/tap.toml` + `tap/catalog/<platform>/<id>.toml` + sibling `<id>.conf` (DOS) or `<id>.fs-uae` (Amiga). See `docs/schema.md`. Until Milestone 6 lands, the only on-disk tap is the fixture at `launcher/src-tauri/tests/fixtures/tap/`.

**Legacy collection directories (`dos/quest-for-glory/`, `dos/kings-quest/`, `amiga/`).** Hold the in-progress migration source material — guide markdown, per-game configs, screenshots, gitignored installer/games directories. Their `manifests/` subdirectories were the old data model and have been removed; the per-game `config/` `.conf` files are migrated entry-by-entry in Milestone 6.

**Rust modules** (`launcher/src-tauri/src/`):

- `catalog`, `install_record`, `tap` — TOML parsers + types matching the schema doc.
- `catalog_view` — joins one or more loaded taps with install records into a single browsable view.
- `launch` — composes `LaunchPlan` (program + args + sidecars) from a catalog entry, install record, and user config.
- `sidecar` — spawns the plan, supervises sidecars (SIGTERM → grace → SIGKILL), streams primary stdout/stderr to a callback when needed.
- `user_config` — parses `${XDG_CONFIG_HOME}/reliquaint/config.toml` with Debian-friendly defaults.
- `doctor::check_install` — host + per-install diagnostics.
- `paths` — XDG-aware locations (`tap_root`, `installs_dir`, `user_config_path`) plus `find_repo_root` heuristic.
- `cli` — the `reliquaint list/run/install/doctor` subcommands.
- `commands` — Tauri command handlers (`list_catalog`, `install_game`, `launch_game`, `run_doctor`, `install_dependency`, `open_url`).
- `gui` — Tauri builder, AppState, AppHandle wiring.
- `logging` (ADR-0004), `error` (ADR-0005) — instrumentation and panic hook foundation.
- `setup`, `installer` — host-dependency install actions (`install_dependency` Tauri command, apt/flatpak chains).

**Svelte components** (`launcher/src/components/`):

- `App.svelte` — root: header + catalog grid + Doctor panel toggle.
- `FilterBar`, `GameGrid`, `GameCard`, `GameDetail` — catalog browser and per-entry detail.
- `DiagnosticPanel` — live tracing events + emulator stdout/stderr stream.
- `DoctorPanel` — host + install diagnostics with "Fix this" buttons for actionable items.

## Build, Test, and Development Commands

```bash
git status --short
cd launcher && cargo test          # 100+ unit + 24 integration tests
cd launcher && cargo build --bin reliquaint
cd launcher && pnpm tauri dev      # requires Node + pnpm + GTK/webkit
reliquaint list                    # browse the catalog
reliquaint doctor                  # host + install diagnostics
reliquaint install qfg1-ega ~/games/qfg1-ega
reliquaint run qfg1-ega --dry-run
reliquaint run qfg1-ega
```

**Env vars** (development / testing):
- `RELIQUAINT_REPO_ROOT` — override where `find_repo_root` would land (used by integration tests).
- `RELIQUAINT_INSTALLS_DIR` — override `paths::installs_dir()` (isolated state for tests).
- `RELIQUAINT_USER_CONFIG_PATH` — override `paths::user_config_path()` (so tests don't pick up the dev's real config).
- `RUST_LOG` — overrides the CLI `-v`/`-vv` verbosity flags.

## Coding Style & Naming Conventions

Lowercase-hyphenated ids for games (e.g. `qfg1-vga`), taps, and collections. Catalog entries shipped as TOML following `docs/schema.md`. New backend code follows `tracing` (ADR-0004) and `thiserror`-in-library / `anyhow`-in-binaries (ADR-0005). Library code does not log errors on the way up — it returns them; the binary layer decides how to surface.

FluidSynth's soundfont path defaults to `/usr/share/sounds/sf2/FluidR3_GM.sf2` (Debian `fluid-soundfont-gm`). Override via `[sidecars.fluidsynth].soundfont` in the user config.

## Testing Guidelines

Most Rust modules carry unit tests inline. Integration tests at `launcher/src-tauri/tests/cli.rs` drive the binary end-to-end against the fixture tap. GUI behaviour is verified manually — run `pnpm tauri dev` from `launcher/`. Don't commit proprietary game installers or extracted game files.

## Commit & Pull Request Guidelines

One task per commit. Subject lines follow `feat(<area>): <short>` or `docs:` / `rename:` etc. Reference the milestone + task number in the body for design-docs-driven work. PRs describe the change, list manual validation performed, and include screenshots only when GUI behaviour visibly changes.
