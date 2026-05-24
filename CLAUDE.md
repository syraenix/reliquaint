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

The Reliquaint launcher: a Rust CLI (`reliquaint`) + Tauri 2 GUI for browsing, installing, and launching classic DOS and Amiga games on Debian-based Linux. Catalog content lives in *taps* — versioned TOML directories — with the bundled `reliquaint-core` tap shipped in `tap/` (populated in Milestone 6).

## Repository layout

- `README.md`, `docs/prerequisites.md` — top-level entry points. Prerequisites cover DOSBox Staging (Flatpak), FluidSynth, FS-UAE, and the Rust toolchain.
- `docs/` — design docs (PRD, schema, ADRs 0001–0005, v0.1 tasks).
- `config/default-dosbox-staging.conf` — reference/baseline DOSBox config used as a starting point for per-entry `.conf` files in the tap.
- `tap/tap.toml` + `tap/catalog/<platform>/<id>.toml` + sibling `<id>.conf` / `<id>.fs-uae` — the bundled `reliquaint-core` tap (populated in Milestone 6). Until then, the on-disk tap is the fixture at `launcher/src-tauri/tests/fixtures/tap/`.
- `dos/<game-collection>/`, `amiga/` — legacy directories with guide markdown, per-game configs, screenshots. Their `manifests/` subdirectories were the pre-redesign data model and are gone. Guides and `.conf` files remain as migration source material for Milestone 6.
- `launcher/` — Rust workspace + Svelte/Tauri frontend.

### Rust modules (`launcher/src-tauri/src/`)

- `catalog`, `install_record`, `tap` — TOML parsers matching `docs/schema.md`.
- `catalog_view` — joins loaded taps with install records into a single browsable view.
- `launch` — composes `LaunchPlan` (program + args + sidecars) from `(CatalogEntry, InstallRecord, UserConfig)`.
- `sidecar` — spawns the plan; SIGTERM/grace/SIGKILL sidecar shutdown; `run_plan_with_callback` streams primary stdout/stderr line-by-line (used by the GUI diagnostic panel).
- `user_config` — `${XDG_CONFIG_HOME}/reliquaint/config.toml` with Debian-friendly defaults.
- `doctor::check_install` — host + per-install diagnostics; reuses `ProbeKind`/`ProbeStatus`/`ProbeResult`.
- `paths` — XDG locations and `find_repo_root` heuristic (recognizes either `tap/tap.toml` or legacy `dos/+amiga/` directory markers).
- `cli` — `reliquaint list/run/install/doctor`.
- `commands` — Tauri command handlers: `list_catalog`, `install_game`, `launch_game`, `run_doctor`, `install_dependency`, `open_url`.
- `gui` — Tauri builder, `AppState`, AppHandle wiring (drives the `logging::TauriBridgeLayer` so tracing events flow to the diagnostic panel).
- `logging` (ADR-0004), `error` (ADR-0005).
- `setup`, `installer` — host-dependency install actions backing the `install_dependency` Tauri command (apt + flatpak, distro-detected).

### Svelte components (`launcher/src/components/`)

`App.svelte` (root) → `FilterBar`, `GameGrid`, `GameCard`, `GameDetail`, `DiagnosticPanel`, `DoctorPanel`.

### Entry-point dispatch

`main.rs` routes by first arg: presence of `list`/`run`/`install`/`doctor`/`--help`/`--version` stays CLI; otherwise opens the GUI. Tauri is always compiled in.

## Conventions when editing

- New backend code uses `tracing` per ADR-0004 and `thiserror`/`anyhow` per ADR-0005. Library modules return `thiserror` enums and don't log errors on the way up; binary entry points (`main.rs`, `gui.rs`, Tauri commands) use `anyhow::Result` and own all user-facing error formatting.
- **Path resolution:** XDG resolution lives in `paths.rs` only — don't reference `.local/share` or `.config` literals from other modules. New app-owned paths get added there.
- FluidSynth's soundfont path defaults to `/usr/share/sounds/sf2/FluidR3_GM.sf2` (Debian `fluid-soundfont-gm`). Override via `[sidecars.fluidsynth].soundfont` in the user config.
- The repo is Linux-only (Flatpak DOSBox Staging, apt-installed FluidSynth/FS-UAE). Don't add Windows/macOS branches unless asked.
- Per-entry `.conf` files in the tap diverge from `config/default-dosbox-staging.conf` deliberately (cycles, MIDI routing, scalers). Don't normalize them to the default.

## Common operations

### CLI

```bash
reliquaint list                            # browse the catalog
reliquaint list --platform dos --installed
reliquaint list --format json              # for scripting
reliquaint install qfg1-ega ~/games/qfg1-ega
reliquaint run qfg1-ega --dry-run          # print the resolved command
reliquaint run qfg1-ega
reliquaint doctor                          # host + per-install diagnostics
reliquaint -v list                         # DEBUG-level tracing to stderr
RUST_LOG=trace reliquaint list             # TRACE
```

**Env vars (development / testing):**
- `RELIQUAINT_REPO_ROOT` — override where the launcher searches for the bundled tap.
- `RELIQUAINT_INSTALLS_DIR` — override `paths::installs_dir()` (test isolation).
- `RELIQUAINT_USER_CONFIG_PATH` — override `paths::user_config_path()` (test isolation).

### Develop the launcher

```bash
cd launcher && cargo test                  # 100+ unit + 24 integration tests
cd launcher && cargo build --bin reliquaint
cd launcher && pnpm install && pnpm tauri dev   # GUI dev (Node + pnpm + Tauri system libs)
cd launcher && pnpm tauri build            # release build
cargo install --path launcher/src-tauri    # install the binary on PATH
```

### Iterate on a per-game `.conf`

Edit the file under `tap/catalog/<platform>/<id>.conf`, then launch DOSBox Staging directly:

```bash
flatpak run io.github.dosbox-staging -conf tap/catalog/dos/<id>.conf
```

Once it feels right, `reliquaint run <id>` composes the launch with the user's install path and any sidecars.
