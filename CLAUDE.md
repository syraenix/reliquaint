# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

The Reliquaint launcher: a Rust CLI (`reliquaint`) + Tauri 2 GUI for browsing, installing, and launching classic DOS and Amiga games on Debian-based Linux. Catalog content lives in *taps* — versioned TOML directories — with the bundled `reliquaint-core` tap at `tap/` (11 entries: 5 QFG, 5 KQ, 1 Amiga).

## Design docs

Read before non-trivial changes:

- `docs/prd.md` — product vision, problem, goals/non-goals, users, scope phases
- `docs/schema.md` — TOML schemas for catalog entries, install records, tap metadata, user config
- `docs/adr-0001-two-layer-manifest-model.md` — shippable catalog vs per-user install records
- `docs/adr-0002-split-dosbox-config-model.md` — shipped `.conf` carries no `[autoexec]`; composed at launch
- `docs/adr-0003-tap-based-distribution.md` — community-maintainable tap repos
- `docs/adr-0004-logging-strategy.md` — `tracing` ecosystem; CLI + GUI share one instrumentation API
- `docs/adr-0005-error-handling-strategy.md` — `thiserror` in library, `anyhow` in binaries

`docs/v0.1-tasks.md` is the historical roadmap kept for context; all v0.1 tasks have shipped.

## Repository layout

- `README.md`, `CONTRIBUTING.md`, `docs/prerequisites.md` — front door.
- `docs/` — design docs.
- `config/default-dosbox-staging.conf` — reference DOSBox config used as a starting point for per-entry `.conf` files.
- `img/` — shared screenshots referenced by the docs.
- `tap/tap.toml` + `tap/catalog/<platform>/<id>.toml` + sibling `<id>.conf` / `<id>.fs-uae` — the bundled `reliquaint-core` tap. Tests use the smaller fixture tap at `launcher/src-tauri/tests/fixtures/tap/`.
- `scripts/extract-installers.sh` — extracts QFG GOG installers into `~/games/`.
- `dos/<game-collection>/`, `amiga/` — collection guide markdown + screenshots. Originally also held the pre-redesign per-game manifests and configs; those migrated into the tap in Milestone 6. The guide prose becomes companion content in v0.4 per ADR-0003. The gitignored `installers/` (QFG) and `games/` (KQ) subdirectories stay put.
- `launcher/` — Rust workspace + Svelte/Tauri frontend.

### Rust modules (`launcher/src-tauri/src/`)

- `catalog`, `install_record`, `tap` — TOML parsers matching `docs/schema.md`. `install_record::register` writes a record for a populated dir.
- `catalog_view` — joins loaded taps with install records into a single browsable view.
- `game_install` — copy/extract a source (directory, DOS `.exe` via `innoextract`, or Amiga `.adf`/`.hdf`/`.rp9`) into the managed library (default `~/games/<id>`). **Stage-then-commit:** pure `plan_install` (classify + build commands) → `stage` (clears `<library>/.<id>.staging`, then runs commands via an injected runner, or unzips `.rp9` in-process) → `commit_dirs` (atomic rename staging → `<library>/<id>`) or `discard_staging` (cancel). The final dir is created only on commit, so a declined/failed install never strands it. `locations()` gives the canonical paths.
- `launch` — composes `LaunchPlan` (program + args + sidecars) from `(CatalogEntry, InstallRecord, UserConfig)`. Amiga: a single internal drive like a real A500 (disk 1 → `--floppy_drive_0`, all floppies → a `--floppy_image_N` swap list, `--floppy_drive_count=1`), `hard_drives` → `--hard_drive_N`, with an autodetect fallback that scans `install_path` for an inner `.fs-uae`/`.hdf`/`.adf` when nothing is declared. Display defaults to windowed + integer-scaled (`--fullscreen=0` + `--window_width=640×scale --window_height=480×scale`, scaling FS-UAE's 640×480 4:3 frame); `[emulators.fs-uae].fullscreen`/`window_scale` in the user config override it.
- `sidecar` — spawns the plan; SIGTERM/grace/SIGKILL sidecar shutdown; `run_plan_with_callback` streams primary stdout/stderr line-by-line (used by the GUI diagnostic panel).
- `user_config` — `${XDG_CONFIG_HOME}/reliquaint/config.toml` with Debian-friendly defaults.
- `doctor::check_install` — host + per-install diagnostics; reuses `ProbeKind`/`ProbeStatus`/`ProbeResult`.
- `paths` — XDG locations and `find_repo_root` heuristic (recognizes either `tap/tap.toml` or legacy `dos/+amiga/` directory markers).
- `cli` — `reliquaint list/run/install/migrate-installs/doctor`.
- `commands` — Tauri command handlers: `list_catalog`, `install_game` (async, streams `install-output`; stages then commits, or returns `MissingFiles`), `commit_install` / `discard_install` (resolve a `MissingFiles` install anyway / cancel), `default_install_dest`, `launch_game`, `run_doctor`, `install_dependency`, `open_url`.
- `gui` — Tauri builder, `AppState`, AppHandle wiring (drives the `logging::TauriBridgeLayer` so tracing events flow to the diagnostic panel).
- `logging` (ADR-0004), `error` (ADR-0005).
- `setup`, `installer` — host-dependency install actions backing the `install_dependency` Tauri command (apt + flatpak, distro-detected).

### Svelte components (`launcher/src/components/`)

`App.svelte` (root) → `FilterBar`, `GameGrid`, `GameCard`, `GameDetail`, `DiagnosticPanel`, `DoctorPanel`.

### Entry-point dispatch

`main.rs` routes by argument presence: a bare `reliquaint` (no arguments) opens the GUI; any arguments are handed to `cli::run()`, where Clap owns parsing, the required-subcommand check, global flags (`-v`/`--verbose`, `--help`, `--version`), and error reporting. Tauri is always compiled in.

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
reliquaint install qfg1-ega ~/Downloads/qfg1.exe      # copy/extract into ~/games/qfg1-ega
reliquaint install kq5 /path/to/kq5-dir --dest /mnt/games   # install under a chosen library dir
reliquaint migrate-installs                # register games already present at ~/games/<id>/
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
- `RELIQUAINT_GAMES_DIR` — override `paths::default_library_dir()` (default `~/games`; test isolation).

### Develop the launcher

```bash
cd launcher && cargo test                  # 143 unit + 31 integration tests
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
