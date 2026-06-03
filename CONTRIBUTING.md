# Contributing to Reliquaint

Thanks for your interest. There are two distinct contribution tracks depending on what you want to add.

## Catalog entries vs. launcher code

**Catalog entries** (new game manifests, shipped `.conf` / `.fs-uae` configs) go to the **[`reliquaint-core`](https://github.com/syraenix/reliquaint-core) repository**, not this one. As of v0.3, the bundled tap is an independent repository. Follow the contributing guide there.

**Launcher code** (new features, bug fixes, CLI commands, Svelte components, Tauri commands) lives here in this repository.

## What we accept here

- **Launcher code changes** — bug fixes, ergonomics, new features that align with the project's goals.
- **Documentation** — fixes to guide text, the README, ADRs.
- **Schema changes** — additions to `docs/schema.md` that are backwards-compatible or come with a version bump plan.

## What we don't accept

- **Game binaries or installers.** Reliquaint is a launcher; it doesn't host or distribute game files. Catalog entries link out to legitimate sources (GOG, Steam, developer site, Internet Archive). They do not include or link to abandonware mirrors that distribute proprietary game files without permission.
- **DRM circumvention.** Not a goal of the project.
- **Cross-platform branches** (Windows/macOS). Linux-only by design for v0.1.

If you have a great Windows/macOS port plan, open an issue first to discuss.

## Adding a catalog entry

Catalog entries go to [`reliquaint-core`](https://github.com/syraenix/reliquaint-core). This is the fastest path:

1. **Wizard.** `reliquaint add ~/games/<your-game>` (or **+ Add game** in the GUI) inspects the directory, fills in a draft manifest, and writes it to your local tap at `${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/tap/`.
2. **Play and tune.** Launch via `reliquaint run <id>`; iterate on the generated `.conf` until it runs well. `reliquaint where <id>` prints the manifest and config paths so you can hand-edit.
3. **Export.** `reliquaint submit <id>` re-validates, warns on completeness gaps (missing `[meta]` fields are the common one), and prints a clean canonical manifest to stdout along with the target path. Add `--clipboard` to copy it for paste.
4. **PR.** Open a PR against [`reliquaint-core`](https://github.com/syraenix/reliquaint-core): paste into `catalog/<platform>/<id>.toml`; if your game ships a DOSBox-Staging config, paste that as `catalog/<platform>/<id>.conf` alongside (no `[autoexec]` block — the launcher composes that at runtime).

If you'd rather write the manifest by hand from scratch, the schema is in [`docs/schema.md`](docs/schema.md).

### 1. Pick an id

Match the filename in `catalog/<platform>/<id>.toml`. The id format is `^[a-z][a-z0-9-]*[a-z0-9]$` — lowercase ASCII, hyphens allowed, no consecutive hyphens, max 64 chars. Examples: `qfg1-ega`, `kings-quest-6`, `fatman`.

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

Drop the DOSBox-Staging config next to your `.toml` at `catalog/dos/<id>.conf` in the `reliquaint-core` repository. Do **not** include an `[autoexec]` section — the launcher composes that at runtime from `runtime.dosbox.entry` and the user's install path. See ADR-0002 for the rationale.

A reasonable starting point is `config/default-dosbox-staging.conf` at the launcher repo root.

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

## Contributing companion content

**Companion content** — per-game walkthroughs, maps, and hint sheets — is a separate contributor flow from catalog entries, but it also lives in [`reliquaint-core`](https://github.com/syraenix/reliquaint-core) (or your own tap), not this repo. It's authored as Markdown and images under `companion/<game-id>/`, beside `catalog/`:

```
companion/
  qfg1-ega/
    01-overview.md
    02-walkthrough.md
    maps/spielburg.png
```

The launcher renders it in the GUI's "Guides" panel. A few rules to author against:

- **Plain Markdown only.** Tables, footnotes, and task lists render; **raw HTML is stripped** by the sanitizer and won't appear. Link to other guides with relative `.md` paths; external `http(s)` links open in the browser.
- **Tap-local images.** Reference images with relative paths and ship the file. No remote/`data:` image URLs (stripped), and **no SVG** (rasterize to PNG/WebP) — see ADR-0006.
- **No game binaries**, same as catalog entries.
- Run `reliquaint doctor` after editing — it warns about broken image references, stray `companion/<id>/` directories, and raw HTML that got stripped.

The full conventions are in the [Tap Maintainer Guide](docs/tap-maintainer-guide.md#companion-content) and [`docs/schema.md`](docs/schema.md#companion-content). Open the PR against [`reliquaint-core`](https://github.com/syraenix/reliquaint-core).

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

- **Logging** — ADR-0004. Use `tracing` for instrumentation. Spans for user-meaningful operations (`launch_game`, `load_tap`, ...). Structured fields, not interpolated strings.
- **Errors** — ADR-0005. Library code returns `thiserror`-derived enums; binary/Tauri layers use `anyhow::Result`. Library code does not log errors — it returns them.

Run `cargo check --all-targets` before committing to make sure the GUI side compiles too.

For GUI changes, also run `pnpm tauri dev` from `launcher/` and click through what you changed. There's no automated browser test harness yet.

## Commit and PR style

- One task per commit. Imperative subject lines, `feat(area):` / `fix(area):` / `docs:` / `chore:` prefixes.
- Wrap commit message bodies at ~72 chars.
- PRs describe what changed, what you tested, and link any related issue.
- Screenshots only when GUI behaviour visibly changes.

### PR titles and releases

All PR titles follow [Conventional Commits](https://www.conventionalcommits.org/) — the `pr-title` CI check enforces this, and [release-please](https://github.com/googleapis/release-please) reads the merged title to decide the next version:

| Title prefix | Release effect |
| --- | --- |
| `feat:` | minor bump (`0.4.0` → `0.5.0`) |
| `fix:` | patch bump (`0.4.0` → `0.4.1`) |
| `feat!:` / `fix!:`, or a `BREAKING CHANGE:` footer | major bump (`0.4.0` → `1.0.0`) |
| `chore:`, `docs:`, `refactor:`, `test:`, etc. | no release on their own |

release-please owns versioning: it maintains a release PR on `main` that bumps `launcher/src-tauri/Cargo.toml` (the single source of truth) and the changelog, then tags the release on merge. Don't hand-edit version numbers, and don't push to `main` directly — branch off `main` and open a PR.

## License

By contributing, you agree that your contributions are licensed under the same terms as the project:

- **Code** — [MPL-2.0](LICENSE)
- **Catalog content** (in [`reliquaint-core`](https://github.com/syraenix/reliquaint-core)) — [CC-BY-SA-4.0](https://creativecommons.org/licenses/by-sa/4.0/)
