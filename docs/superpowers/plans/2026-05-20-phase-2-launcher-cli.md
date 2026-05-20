# Phase 2 — `classic-launcher` Rust CLI Implementation Plan

**Goal:** Replace every per-game bash run script with a single Rust CLI (`classic-launcher`) driven by per-game TOML manifests. Reproduces today's launch behavior exactly and adds `list`, `run`, and `doctor` subcommands.

**Roadmap reference:** Phase 2 of the multi-phase roadmap. Phase 1 (Amiga collection) shipped in `docs/superpowers/plans/2026-05-19-amiga-collection.md` and is now the baseline.

**Plan style note:** This is the executive plan. TDD micro-steps (every test case, every commit) are deliberately not enumerated — the executor expands each task in the sequence below into the standard `write failing test → run → impl → run → commit` cycle, following the style of `2026-05-19-amiga-collection.md`.

---

## Context

Phase 1 of the roadmap (Amiga collection) shipped. Phase 2 is the first phase that introduces a real engine: a Rust CLI that consumes per-game manifests, owns the launch logic, and retires the script-per-game pattern. After Phase 2 the repo will have:

- A single binary (`classic-launcher`) on the user's `PATH` that lists, launches, and diagnoses every game across DOS and Amiga collections.
- Per-game TOML manifests as the new source of truth (alongside existing `.conf` files, which manifests reference).
- Zero per-game bash run scripts. `extract-installers.sh` stays (Phase 5 folds it in).
- Updated guides pointing users at `classic-launcher run <id>` instead of `./<game>-run.sh`.

The engine is built CLI-first deliberately: locking the manifest contract and the spawn logic in a terminal-debuggable shape before Phase 3 layers a Tauri GUI on top of the same modules.

## Decisions locked in (from earlier in this session)

| Question | Decision |
|---|---|
| Manifest layout | `dos/<collection>/manifests/<game>.toml` and `amiga/manifests/<game>.toml` (symmetric across platforms) |
| Bash script fate | All per-game run scripts deleted in Phase 2 (incl. `amiga-run.sh` and `test-amiga-run.sh`) |
| Install method | `cargo install --path launcher/src-tauri` → `~/.cargo/bin/classic-launcher`; `rustup` added to prereqs |
| Worktree branch | `phase-2-launcher-cli` |
| `.rp9` auto-discovery | Deferred to Phase 5 (with `extract-installers.sh` folding) |
| Amiga manifests | No real ones committed (no `.adf`/`.rp9` in repo). Ship one `amiga/manifests/example.toml.disabled` as schema docs |

## File structure

**Create — Rust workspace under `launcher/`:**
- `launcher/Cargo.toml` — workspace root, single member `src-tauri`
- `launcher/.gitignore` — `/target`
- `launcher/src-tauri/Cargo.toml` — binary + lib targets, deps locked below
- `launcher/src-tauri/src/main.rs` — thin entrypoint calling `cli::run`
- `launcher/src-tauri/src/lib.rs` — module declarations
- `launcher/src-tauri/src/manifest.rs` — TOML schema + parse/validate
- `launcher/src-tauri/src/paths.rs` — tilde expand + manifest-relative resolution
- `launcher/src-tauri/src/discovery.rs` — walk `dos/*/manifests/*.toml` + `amiga/manifests/*.toml`
- `launcher/src-tauri/src/runner.rs` — pure `Command` builders + `spawn` orchestration
- `launcher/src-tauri/src/doctor.rs` — host-dep probes + per-manifest dir checks
- `launcher/src-tauri/src/cli.rs` — clap parser + subcommand dispatch
- `launcher/src-tauri/tests/cli.rs` — `assert_cmd` integration tests
- `launcher/src-tauri/tests/fixtures/` — minimal repo tree (a few manifests + dummy .conf)

**Create — manifests:**
- `dos/quest-for-glory/manifests/{qfg1-ega,qfg1-vga,qfg2,qfg3,qfg4}.toml`
- `dos/kings-quest/manifests/{kq1sci,kq2,kq3,kq4,kq5,kq6}.toml`
- `amiga/manifests/example.toml.disabled` — schema docs; `.disabled` suffix makes discovery skip it
- `amiga/manifests/.gitkeep`

**Modify:**
- `.gitignore` — add `/launcher/target`
- `README.md` — switch from per-game scripts to `classic-launcher`
- `docs/prerequisites.md` — add Rust toolchain section
- `dos/quest-for-glory/quest-for-glory.md` — rewrite "Running the games" section
- `dos/kings-quest/kings-quest.md` — same
- `amiga/amiga.md` — rewrite "Running a game"; document required per-game manifest authoring
- `AGENTS.md`, `CLAUDE.md` — point at `classic-launcher` instead of `*-run.sh`

**Delete (`git rm`):**
- `dos/quest-for-glory/scripts/qfg{1-ega,1-vga,2,3,4}-run.sh`
- `dos/kings-quest/scripts/kq{1sci,2,3,4,5,6}-run.sh`
- `amiga/scripts/amiga-run.sh`, `amiga/scripts/test-amiga-run.sh`, then `rmdir amiga/scripts/`
- `dos/quest-for-glory/scripts/extract-installers.sh` **stays.**

## Manifest schema (locked)

```toml
# Required top-level fields (both platforms)
id          = "qfg1-ega"               # kebab-case; CLI identifier
title       = "Quest for Glory 1 (EGA)"
platform    = "dos"                    # "dos" | "amiga"
collection  = "quest-for-glory"

# DOS-only [runtime]
[runtime]
emulator = "dosbox-staging"
config   = "../config/qfg1-ega.conf"   # relative to manifest file
sidecars = ["fluidsynth"]              # defaults to []

# DOS-only [install]
[install]
expects_dir = "~/games/qfg1-ega"       # tilde-expanded at runtime

# Amiga [runtime]
# emulator = "fs-uae"
# file     = "../games/lemmings.adf"   # .adf or .rp9, relative to manifest
# model    = "a500"                    # "a500" | "a1200"; ignored if .rp9 ships config
# config   = "../config/a500.fs-uae"   # optional explicit override

# Optional [ui] (both platforms)
[ui]
artwork = "../img/qfg1-ega-cover.png"
```

**Validation:** `serde(deny_unknown_fields)` on every struct so typos fail loudly. DOS manifests require `runtime.config` + `install.expects_dir`. Amiga manifests require `runtime.file`; `runtime.sidecars` must be empty.

## Module responsibilities

| Module | Public surface | Responsibility |
|---|---|---|
| `manifest` | `Manifest`, `Platform`, `Runtime`, `Install`, `Ui`, `ManifestError`, `parse_str`, `parse_file` | TOML → typed struct; validate cross-field rules |
| `paths` | `expand_tilde`, `resolve_relative` | Tilde + manifest-relative path helpers |
| `discovery` | `find_repo_root`, `discover`, `find_by_id` | Walk repo tree, parse all manifests, lookup by id (skips `.disabled`) |
| `runner` | `build_dos_command`, `build_fluidsynth_command`, `build_amiga_command`, `RunOpts`, `run` | Pure `Command` builders + spawn orchestration with fluidsynth sidecar (poll `pgrep -x fluidsynth` every 200ms up to 10s, 1:1 with bash) |
| `doctor` | `ProbeResult`, `ProbeStatus`, `run_all` | Probes: flatpak DOSBox-Staging, fluidsynth, soundfont, innoextract, fs-uae, unzip + per-manifest `expects_dir` existence |
| `cli` | `run() -> ExitCode` | clap `#[derive(Parser)]` with `List`, `Run { id, dry_run, windowed }`, `Doctor` subcommands |

**Testability invariant:** builders never spawn — tests assert on `Command::get_program()` and `get_args()`. Only `runner::run` spawns; integration tests cover it via `--dry-run` (prints the would-be commands instead of running them).

## Dependencies (locked in `launcher/src-tauri/Cargo.toml`)

| Crate | Version | Features | Use |
|---|---|---|---|
| `clap` | 4.5 | `derive` | Subcommand parser |
| `serde` | 1.0 | `derive` | (for `toml`) |
| `toml` | 0.8 | default | Parse manifests |
| `anyhow` | 1.0 | default | Top-level error in `main` + tests |
| `thiserror` | 1.0 | default | Typed `ManifestError` for unit `matches!` checks |
| `shellexpand` | 3.1 | default | `~` expansion |
| `zip` | 0.6 | default | Unpack `.rp9` (Zip) |
| `tempfile` | 3.10 | default | RAII `TempDir` for `.rp9` unpack |
| `assert_cmd` | 2.0 | (dev) | Drive binary in integration tests |
| `predicates` | 3.1 | (dev) | Assertions on stdout |

No `walkdir` (single-level `read_dir`), no `regex`, no logger.

## CLI surface

```
classic-launcher list
classic-launcher run <id> [--dry-run] [--windowed]
classic-launcher doctor
```

- `list`: prints `<id>  <platform>  <collection>  <title>` for every discovered manifest, sorted by id.
- `run <id>`: starts fluidsynth (if sidecar), polls `pgrep -x fluidsynth` until ready, spawns `flatpak run io.github.dosbox-staging -conf <abs config>` (DOS) or `fs-uae ...` (Amiga). Returns the primary's exit code. `--dry-run` prints `[sidecar] ...` and `[primary] ...` lines without spawning. `--windowed` is Amiga-only.
- `doctor`: prints each host probe with `[ ok | missing | unknown ]` plus per-DOS-manifest install-dir checks. Exits 2 if any probe is `missing`.

## Task sequence

Bottom-up + commit-after-every-test; each leaves the tree green.

1. **Worktree + Rust prereq doc** — create `phase-2-launcher-cli` worktree via `superpowers:using-git-worktrees`; add Rust toolchain section to `docs/prerequisites.md`. Commit.
2. **Cargo workspace scaffolding** — write `launcher/Cargo.toml`, `launcher/src-tauri/Cargo.toml`, stub modules, ensure `cargo build` and `cargo run` succeed with a placeholder message. Add `/launcher/target` to root `.gitignore`. Commit.
3. **`manifest` happy path** — DOS + Amiga round-trip tests; types per schema; `parse_str` + `parse_file`. Commit.
4. **`manifest` error cases** — missing required fields, unknown fields, DOS-without-config, Amiga-without-file, sidecar-on-amiga. Commit.
5. **`paths`** — `expand_tilde`, `resolve_relative` with tests. Commit.
6. **`discovery`** — fixture tree; ensure `.disabled` files are skipped; `find_by_id` returns `Option`. Commit.
7. **`runner` DOS command builder** — assert constructed `Command` matches `flatpak run io.github.dosbox-staging -conf <abs-path>`. Commit.
8. **`runner` Amiga `.adf` builder** — assert `fs-uae <abs-model-config> --floppy_drive_0=<abs-adf>`; missing model config errors cleanly. Commit.
9. **`runner` `.rp9` unpack** — real Zip fixture; if inner `.fs-uae` present use it, else fall back to inner `.adf` + model. Returns `(Command, Option<TempDir>)` so caller keeps temp dir alive across spawn. Commit.
10. **`runner::run` orchestration + `--dry-run`** — spawn sidecar, poll `pgrep -x fluidsynth` every 200ms up to 10s, then primary; on `--dry-run`, print and return. Commit.
11. **`doctor` probes** — six host probes + per-manifest dir checks; unit tests with tempdir + mocked PATH. Commit.
12. **`cli` dispatch** — clap parser; wire subcommands to module entrypoints; doctor returns exit 2 on failure. Commit.
13. **`assert_cmd` integration tests** — fixture-driven end-to-end against the binary: `list`, `run --dry-run`, `doctor`. Commit.
14. **Author 11 DOS manifests** — full TOML for QFG (5) + KQ (6). Verify `list` shows all 11 sorted. Commit.
15. **Amiga placeholder** — `amiga/manifests/example.toml.disabled` + `.gitkeep`; verify `list` ignores it. Commit.
16. **Smoke test (no commit)** — `cargo install --path launcher/src-tauri`; run `classic-launcher doctor`; run `classic-launcher run qfg1-ega` and confirm the game boots in DOSBox-Staging. Repeat for one KQ. If anything fails, halt here — the bash scripts are still on disk and recoverable.
17. **Delete bash scripts** — `git rm` 11 DOS scripts + 2 Amiga scripts; `rmdir amiga/scripts`. Verify `extract-installers.sh` is the only thing left under `dos/quest-for-glory/scripts/`. Commit.
18. **Update per-collection guides** — rewrite "Running the games" / "Running a game" sections in `quest-for-glory.md`, `kings-quest.md`, `amiga.md` to use `classic-launcher run <id>`. Commit.
19. **Update top-level docs** — `README.md`, `AGENTS.md`, `CLAUDE.md` references to scripts → CLI. Commit.
20. **Final verification** — see below.

## Verification (Task 20)

Run from the worktree root:

```bash
# 1. Workspace tests are green
cd launcher && cargo test
# Expect: all unit + integration tests pass.

# 2. Binary builds and is installable
cargo install --path src-tauri
# Expect: classic-launcher installed to ~/.cargo/bin

# 3. Discovery
classic-launcher list
# Expect: 11 lines, sorted: kq1sci, kq2, kq3, kq4, kq5, kq6, qfg1-ega, qfg1-vga, qfg2, qfg3, qfg4

# 4. Doctor (host probes + per-manifest dir checks)
classic-launcher doctor
# Expect: each host dep printed with [ ok / missing ]; missing install dirs flagged but not fatal unless probe missing.

# 5. Dry-run preview
classic-launcher run qfg1-ega --dry-run
# Expect stdout contains both:
#   [sidecar] fluidsynth -i /usr/share/sounds/sf2/FluidR3_GM.sf2
#   [primary] flatpak run io.github.dosbox-staging -conf <abs>/dos/quest-for-glory/config/qfg1-ega.conf

# 6. End-to-end launch (QFG)
classic-launcher run qfg1-ega
# Expect: fluidsynth starts; DOSBox-Staging opens at QFG1 EGA title screen. Quit cleanly; binary exits 0.

# 7. End-to-end launch (KQ) — picks up the other code path
classic-launcher run kq1sci
# Expect: KQ1 SCI title screen.

# 8. No regressed scripts
ls dos/*/scripts/ amiga/ 2>/dev/null
# Expect: only dos/quest-for-glory/scripts/extract-installers.sh; no amiga/scripts/ directory.
```

If all eight pass, open the PR against `develop`.

## Critical files for execution

The executor should keep these open while implementing:

- **Existing run scripts (to mirror behavior):** `dos/quest-for-glory/scripts/qfg1-ega-run.sh`, `amiga/scripts/amiga-run.sh`
- **`.conf` files (referenced by manifests, unchanged):** `dos/*/config/*.conf`, `amiga/config/{a500,a1200}.fs-uae`
- **Phase 1 reference:** `docs/superpowers/plans/2026-05-19-amiga-collection.md` — same plan style, same conventions
- **Project conventions:** `CLAUDE.md`, `AGENTS.md`

## Out of scope (explicitly)

- Any Tauri/GUI work (Phase 3).
- `extract-installers.sh` rewrite (Phase 5).
- `.rp9` filesystem auto-discovery in `amiga/games/` (Phase 5).
- Non-Debian distro support, Windows/macOS, WHDLoad/.hdf, Amiberry.
