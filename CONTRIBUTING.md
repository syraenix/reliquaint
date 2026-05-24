# Contributing to Reliquaint

Thanks for your interest. Reliquaint is small but the catalog has room to grow — most contributions will be new entries in the bundled `reliquaint-core` tap.

## What we accept

- **Catalog entries** for DOS or Amiga games (TOML files under `tap/catalog/<platform>/`).
- **Per-game configs** — shipped DOSBox-Staging `.conf` or FS-UAE `.fs-uae` files that ship alongside the catalog entry.
- **Launcher code changes** — bug fixes, ergonomics, new features that align with the [PRD](docs/prd.md).
- **Documentation** — fixes to guide text, the README, ADRs.

## What we don't accept

- **Game binaries or installers.** Reliquaint is a launcher; it doesn't host or distribute game files. Catalog entries link out to legitimate sources (GOG, Steam, developer site, Internet Archive). They do not include or link to abandonware mirrors that distribute proprietary game files without permission.
- **DRM circumvention.** Not a goal of the project.
- **Cross-platform branches** (Windows/macOS). Linux-only by design for v0.1.

If you have a great Windows/macOS port plan, open an issue first to discuss.

## Adding a catalog entry

### 1. Pick an id

Match the filename in `tap/catalog/<platform>/<id>.toml`. The id format is `^[a-z][a-z0-9-]*[a-z0-9]$` — lowercase ASCII, hyphens allowed, no consecutive hyphens, max 64 chars. Examples: `qfg1-ega`, `kings-quest-6`, `fatman`.

### 2. Write the TOML

Copy an existing entry as a starting point. Required fields, per [`docs/schema.md`](docs/schema.md):

```toml
schema_version = 1

[game]
id         = "my-game"
title      = "My Game: Full Title"
platform   = "dos"          # or "amiga"
collection = "some-series"  # optional; groups in the UI

[meta]
year        = 1990
developer   = "Studio Name"
publisher   = "Publisher Name"
genre       = ["adventure"]
tags        = ["sci", "fantasy"]
description = "One-paragraph blurb for the catalog browser."

[acquisition]
gog   = "https://www.gog.com/..."
notes = "Bundled in <collection> on GOG."

[install]
expects_files = ["GAME.EXE", "RESOURCE.000"]

[runtime]
emulator = "dosbox-staging"
sidecars = ["fluidsynth"]   # optional; v0.1 supports fluidsynth only

[runtime.dosbox]            # for platform = "dos"
config = "my-game.conf"
entry  = "GAME.EXE"
mount  = "c"
```

For Amiga, use `runtime.fs_uae` with `model` (`a500`/`a600`/`a1200`/`a4000`), optional `config` (a sibling `.fs-uae` file), and `floppies = ["disk1.adf", ...]`.

### 3. Ship a `.conf` (DOS)

Drop the DOSBox-Staging config next to your `.toml` at `tap/catalog/dos/<id>.conf`. Do **not** include an `[autoexec]` section — the launcher composes that at runtime from `runtime.dosbox.entry` and the user's install path. See [ADR-0002](docs/adr-0002-split-dosbox-config-model.md) for the rationale.

A reasonable starting point is `config/default-dosbox-staging.conf` at the repo root.

### 4. Test locally

```bash
# Install the launcher (from this repo):
cargo install --path launcher/src-tauri

# Set up a fake install dir matching your expects_files:
mkdir -p ~/games/my-game && touch ~/games/my-game/GAME.EXE
reliquaint install my-game ~/games/my-game

# Verify the launch command composes correctly:
reliquaint run my-game --dry-run

# If you have real game files there, try a live launch:
reliquaint run my-game
```

`reliquaint doctor` should report the install as ok (or surface a clear missing-file warning if `expects_files` doesn't match what you actually have).

### 5. Open a PR

One catalog entry per commit. Subject line: `feat(<collection>): add <title> manifest`. Include in the PR description:

- Which acquisition source(s) you've verified work
- Whether you've tested a live launch (or only `--dry-run`)
- Anything unusual about the config (non-standard `cycles`, MIDI quirks, etc.)

## Style conventions

- **TOML keys**: snake_case (`expects_files`, `kickstart_path`, `runtime.fs_uae`).
- **Ids**: lowercase-hyphenated.
- **Field order**: follow the example above (schema_version → game → meta → acquisition → install → runtime → runtime sub-tables). Aligns with the schema doc and makes diffs easier to read.
- **`expects_files`**: include the entry-point binary plus one or two structural files (e.g. `RESOURCE.000` for Sierra SCI games). Don't try to list every file.
- **`description`**: one paragraph, plain text. The schema doesn't interpret Markdown.

## Launcher code changes

Run the full test suite:

```bash
cd launcher && cargo test
```

New backend code follows two conventions:

- **Logging** — [ADR-0004](docs/adr-0004-logging-strategy.md). Use `tracing` for instrumentation. Spans for user-meaningful operations (`launch_game`, `load_tap`, ...). Structured fields, not interpolated strings.
- **Errors** — [ADR-0005](docs/adr-0005-error-handling-strategy.md). Library code returns `thiserror`-derived enums; binary/Tauri layers use `anyhow::Result`. Library code does not log errors — it returns them.

Run `cargo check --all-targets` before committing to make sure the GUI side compiles too.

For GUI changes, also run `pnpm tauri dev` from `launcher/` and click through what you changed. There's no automated browser test harness yet.

## Commit and PR style

- One task per commit. Imperative subject lines, `feat(area):` / `fix(area):` / `docs:` / `chore:` prefixes.
- Wrap commit message bodies at ~72 chars.
- PRs describe what changed, what you tested, and link any related issue.
- Screenshots only when GUI behaviour visibly changes.

## License

By contributing, you agree that your contributions are licensed under the same terms as the project:

- **Code** — [MPL-2.0](LICENSE)
- **Catalog content** — [CC-BY-SA-4.0](LICENSE-CONTENT)
