# Tap Maintainer Guide

A **tap** is a versioned directory of catalog entries (TOML files) and shipped emulator configs that Reliquaint users can subscribe to. This guide covers setting up your own tap, structuring its content, and keeping it healthy.

## Tap anatomy

```
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
|---|---|
| `[game]` | `id`, `title`, `platform`. |
| `[meta]` | `year`, `developer`, `genre`, `description` (all optional but strongly recommended). |
| `[acquisition]` | Links to legitimate purchase/download sources (`gog`, `steam`, `archive`, etc.). |
| `[install]` | `expects_files` — filenames the launcher checks after install. |
| `[runtime]` | `emulator` (`dosbox-staging` or `fs-uae`), and platform sub-tables. |

Game ids must be globally unique within your tap. Duplicates cause a validation error.

## Conflict resolution across taps

When a user subscribes to multiple taps and two of them provide the same game id, the launcher resolves the conflict by **priority**: the tap with the lower integer priority wins. The user tap (`local`) always wins at implicit priority −1.

Users adjust priorities via `reliquaint tap reorder`. As a tap maintainer you cannot force a particular priority — that's the user's choice. If your tap covers games also in `reliquaint-core`, document the differences (different edition, different config, etc.) in the catalog entry's `[meta].description` so users can make an informed choice.

## Versioning

`tap.toml` carries a `version` field (SemVer-compatible). Bump it in the same commit that makes a breaking change (e.g. removing an entry, renaming an id). Additive changes (new entries, new optional fields) do not require a version bump.

## Licensing catalog content

The `license` field in `tap.toml` applies to the catalog entries and shipped configs in your tap. `reliquaint-core` uses CC-BY-SA-4.0. Community taps may choose their own license, but note that entries based on `reliquaint-core` content are subject to the ShareAlike requirement.

Do not include game binaries, installers, or disk images in a tap repository. The `[acquisition]` table links to legitimate sources; the tap only carries metadata.
