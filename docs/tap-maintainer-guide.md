# Tap Maintainer Guide

A **tap** is a versioned directory of catalog entries (TOML files) and shipped emulator configs that Reliquaint users can subscribe to. This guide covers setting up your own tap, structuring its content, and keeping it healthy.

## Tap anatomy

```plaintext
<tap-root>/
  tap.toml                          # tap metadata (id, title, version, …)
  catalog/
    dos/<id>.toml                   # DOS catalog entry
    dos/<id>.conf                   # shipped DOSBox-Staging config (no [autoexec])
    amiga/<id>.toml                 # Amiga catalog entry
    amiga/<id>.fs-uae               # shipped FS-UAE config (optional)
```

Full schema for all files is in [`docs/schema.md`](schema.md). The short version:

- `tap.toml` identifies the tap (unique `id`, human `title`, `version`, `maintainer`, `url`, `license`).
- Each `catalog/<platform>/<id>.toml` describes one game: metadata, acquisition links, install expectations, and runtime parameters.
- Sibling `.conf` / `.fs-uae` files carry per-game emulator tuning with **no user-specific data** (no absolute paths, no `[autoexec]`).

## Naming your tap

The tap `id` (in `tap.toml`) must match `^[a-z][a-z0-9-]*[a-z0-9]$`:

- Lowercase ASCII, digits, hyphens.
- Starts with a letter; ends with a letter or digit.
- No consecutive hyphens.
- Max 64 characters.

The id **`local`** is reserved for the user's own writable tap and cannot be used.

Pick a name that is specific enough to be unambiguous among community taps. `reliquaint-core`, `scummvm-classics`, `amiga-arcade-pack` are fine examples.

## Hosting on GitHub (recommended)

1. Create a public repository named after your tap id.
2. Populate it with `tap.toml` and at least one `catalog/` entry.
3. Users subscribe via: `reliquaint tap add https://github.com/<you>/<tap-id>`

If you register your tap in the [known taps table](../launcher/src-tauri/src/known_taps.rs) (via a PR to this repo), users can subscribe with just the short name: `reliquaint tap add <tap-id>`.

## Validation CI

Use `reliquaint tap validate .` in CI to catch broken entries on every push. Example GitHub Actions workflow:

```yaml
name: Validate catalog
on: [push, pull_request]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install reliquaint
        run: cargo install --git https://github.com/syraenix/reliquaint reliquaint
      - name: Validate tap
        run: reliquaint tap validate .
```

`tap validate` checks:

- `tap.toml` is present and parses against the schema.
- Every `catalog/<platform>/*.toml` file parses without errors.
- Bad entries are reported by filename; the command exits non-zero if any fail.

## Adding entries

Each entry is one TOML file. Copy a nearby example as a starting point, or generate a draft with the Reliquaint wizard (`reliquaint add <game-dir>`). Required top-level tables:

| Table | Purpose |
| --- | --- |
| `[game]` | `id`, `title`, `platform`. |
| `[meta]` | `year`, `developer`, `genre`, `description` (all optional but strongly recommended). |
| `[acquisition]` | Links to legitimate purchase/download sources (`gog`, `steam`, `archive`, etc.). |
| `[install]` | `expects_files` — filenames the launcher checks after install. |
| `[runtime]` | `emulator` (`dosbox-staging` or `fs-uae`), and platform sub-tables. |

Game ids must be globally unique within your tap. Duplicates cause a validation error.

## Companion content

A tap can ship per-game **companion content** — walkthroughs, maps, hint sheets, install notes — that the launcher renders alongside the catalog entry in the GUI's "Guides" section. It lives in a `companion/` tree beside `catalog/`:

```plaintext
<tap-root>/
  catalog/dos/qfg1-ega.toml
  companion/
    qfg1-ega/
      01-overview.md
      02-walkthrough.md
      maps/
        spielburg.png
      hints/
        boss-fight-tips.md
```

The directory name under `companion/` is the game `id`. Full conventions are in [`docs/schema.md`](schema.md#companion-content); the essentials:

- **Files.** Markdown (`.md`) plus images (`.png`, `.jpg`/`.jpeg`, `.gif`, `.webp`). One level of subdirectory (e.g. `maps/`, `hints/`) becomes a named section in the UI.
- **Ordering and titles.** A numeric prefix sorts a file first (`01-overview.md` before `02-walkthrough.md`); un-prefixed files follow alphabetically. The display title derives from the filename: the prefix is dropped and `-`/`_` become spaces, so `02-mid-game.md` → "Mid Game".

### Authoring Markdown

Write **plain Markdown** — paragraphs, headings, lists, tables, code blocks, footnotes, and task lists all render. Two linking rules:

- **Link to another guide** with a relative path to its `.md` file (`[next](02-mid-game.md)`); the launcher navigates to it inside the viewer.
- **External links** (`http://`, `https://`) open in the user's system browser.

### Images

Images are **tap-local only** — reference them with a relative path (`![map](maps/spielburg.png)`) and ship the file in the tap. The launcher serves them through an internal protocol scoped to the game's companion directory.

- **No remote image URLs.** `http(s)`, `data:`, and `file:` image sources are stripped (privacy and security). A tracking-pixel image would leak the user's activity; tap-local images can't.
- **No SVG.** SVG is structured XML that can carry scripts, so it's excluded. Rasterize maps and diagrams to PNG or WebP. Aim for reasonable file sizes (a few hundred KB per image is plenty).

### The sanitizer: don't submit raw HTML

Companion Markdown is rendered server-side and run through a strict HTML sanitizer before it reaches the webview. **Raw HTML is stripped** — a `<div>`, `<style>`, `<script>`, or inline `<span style=…>` in your source will not render, and the result will surprise both you and the user. Stick to Markdown. `reliquaint doctor` warns when a companion file contains a significant amount of raw HTML (and when an image reference points at a missing file, or a `companion/<id>/` directory has no matching catalog entry), so run it after editing.

## Conflict resolution across taps

When a user subscribes to multiple taps and two of them provide the same game id, the launcher resolves the conflict by **priority**: the tap with the lower integer priority wins. The user tap (`local`) always wins at implicit priority −1.

Users adjust priorities via `reliquaint tap reorder`. As a tap maintainer you cannot force a particular priority — that's the user's choice. If your tap covers games also in `reliquaint-core`, document the differences (different edition, different config, etc.) in the catalog entry's `[meta].description` so users can make an informed choice.

## Versioning

`tap.toml` carries a `version` field (SemVer-compatible). Bump it in the same commit that makes a breaking change (e.g. removing an entry, renaming an id). Additive changes (new entries, new optional fields) do not require a version bump.

## Licensing catalog content

The `license` field in `tap.toml` applies to the catalog entries and shipped configs in your tap. `reliquaint-core` uses CC-BY-SA-4.0. Community taps may choose their own license, but note that entries based on `reliquaint-core` content are subject to the ShareAlike requirement.

Do not include game binaries, installers, or disk images in a tap repository. The `[acquisition]` table links to legitimate sources; the tap only carries metadata.
