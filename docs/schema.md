# Reliquaint Schema Design

This document specifies the on-disk TOML schemas used by Reliquaint. It is the contract between the launcher and:

- **Tap maintainers**, who write catalog entries and shipped emulator configs.
- **Users**, who write a user-scope launcher config and (implicitly, via the install flow) installation records.
- **The launcher**, which reads all of the above to launch games.

The schemas defined here are versioned. The current schema version is **`1`**.

---

## Conventions

### Identifiers

A `<id>` (used for games, taps, and series) must match: `^[a-z][a-z0-9-]*[a-z0-9]$`.

- Lowercase ASCII only.
- Letters, digits, hyphens.
- Must start with a letter.
- Must end with a letter or digit.
- No consecutive hyphens.
- Reasonable length cap: 64 characters.

Examples: `qfg1-ega`, `kings-quest-6`, `fatman`, `reliquaint-core`.

### Schema versioning

Every TOML file specified in this document has a top-level `schema_version` integer key. The launcher reads `schema_version` first and refuses to process files it does not understand, with a clear error pointing at this document.

Schema changes that add optional fields do not bump the version. Schema changes that rename, remove, or change the semantics of fields do.

### Path resolution

- Paths in catalog entries (shipped) are **relative to the tap root** or **bare filenames** (interpreted relative to the install path at runtime, never absolute).
- Paths in installation records are **absolute and machine-specific**.
- Paths in user config are **absolute or `~`-prefixed** (tilde-expanded at read time).
- Catalog entries never contain absolute filesystem paths. The launcher rejects any that do.

---

## Tap layout

A tap is a versioned source of catalog entries, shipped emulator configs, and companion content.

```plaintext
<tap-root>/
  tap.toml                              # tap metadata
  catalog/
    dos/<id>.toml                       # catalog entry
    dos/<id>.conf                       # shipped DOSBox-Staging config (no autoexec)
    amiga/<id>.toml                     # catalog entry
    amiga/<id>.fs-uae                   # shipped FS-UAE config (optional; falls back to model template)
  companion/                            # v0.4+, see "Companion content" below
    <id>/...
```

The launcher loads catalog entries by walking `catalog/<platform>/*.toml`. The `.conf` / `.fs-uae` files are sibling resources referenced from the catalog entry's `runtime` table.

The bundled tap (named `reliquaint-core`) shipped inside the launcher repository through v0.2. As of v0.3 it lives in its own repository at `https://github.com/syraenix/reliquaint-core` and is fetched via the tap subscription system described below.

### User tap (v0.2)

User-created catalog entries — the ones produced by the `reliquaint add <path>` wizard and the GUI "Add game" flow — live in a local pseudo-tap at:

```bash
${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/tap/
```

It uses the same layout as the bundled tap (`tap.toml` at root, `catalog/<platform>/<id>.toml`, sibling `.conf` / `.fs-uae`). The launcher creates this directory and a minimal `tap.toml` lazily on the first manifest write, so users who never add a custom game never get the directory.

The user tap claims the reserved id **`local`**. The launcher refuses to load any externally supplied tap (subscribed third-party tap, copied-over bundled tap, etc.) that claims this id — the id is exclusively for the user's own writable tap. Catalog entries in the user tap follow the same schema as bundled ones.

### Tap subscription cache (v0.3)

Subscribed taps are git-cloned to:

```bash
${XDG_DATA_HOME:-$HOME/.local/share}/reliquaint/taps/<tap-id>/
```

Each is a full tap directory (same layout as above: `tap.toml`, `catalog/<platform>/*.toml`, sibling configs). The launcher reads every subscribed tap's cache alongside the user tap on every run; no daemon is required. Syncing to the latest commits is done explicitly via `reliquaint tap sync` or the Taps panel in the GUI.

### Reserved tap ids

The following tap ids are reserved and cannot be used by subscribed or third-party taps:

| Id | Reserved for |
| --- | --- |
| `local` | The user's own writable tap at `${XDG_CONFIG_HOME}/reliquaint/tap/`. |

---

## `subscriptions.toml` (v0.3)

Records which community taps the user is subscribed to. Written and maintained by `reliquaint tap add/remove/reorder`.

**Location:** `${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/subscriptions.toml`

```toml
schema_version = 1

[[tap]]
id       = "reliquaint-core"
source   = "https://github.com/syraenix/reliquaint-core"
added_at = 2026-06-01T12:00:00Z
priority = 0
```

### Fields

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Always `1`. |
| `tap[].id` | string | yes | The tap's identifier. Must match the `id` in the fetched tap's `tap.toml`. Follows the identifier convention. Cannot be `local`. |
| `tap[].source` | string | yes | URL or local path used to clone/pull the tap. HTTPS URLs are cloned via system `git`. |
| `tap[].added_at` | datetime | yes | TOML datetime of when the subscription was created. |
| `tap[].priority` | integer | yes | Conflict resolution priority. Lower value wins when two subscribed taps offer the same game id. The user tap (`local`) always wins at implicit priority −1, regardless of this field. |

### Conflict resolution

When two taps provide an entry with the same `[game].id`:

1. The user tap (`local`) always wins — it takes priority over any subscribed tap.
2. Among subscribed taps, the entry from the tap with the **lower** `priority` value is shown in the catalog.
3. The game detail view shows a note indicating alternate versions exist in other taps.

Priorities within a subscription list must be unique. `reliquaint tap reorder` reassigns priorities; gaps in the sequence are valid.

---

## `tap.toml`

The tap's own metadata. One file per tap, at the tap root.

```toml
schema_version = 1

id          = "reliquaint-core"
title       = "Reliquaint Core"
description = "The default catalog of classic DOS and Amiga games shipped with Reliquaint."
version     = "0.1.0"
maintainer  = "Derek <derek@example.com>"
url         = "https://github.com/syraenix/reliquaint"
license     = "CC-BY-SA-4.0"
```

### Fields

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Always `1` for files conforming to this document. |
| `id` | string | yes | Unique tap identifier. Follows the identifier convention. |
| `title` | string | yes | Human-readable tap name. |
| `description` | string | yes | One-paragraph description of the tap's purpose / curation focus. |
| `version` | string | yes | SemVer-compatible version of the tap's content. |
| `maintainer` | string | yes | Maintainer name and contact, free-form. |
| `url` | string | yes | Homepage or repository URL. |
| `license` | string | yes | License for the catalog content. Code in the launcher itself is separate. |

The `id` must be unique among the user's subscribed taps. When two taps would collide on `id`, the user's launcher rejects the later one and surfaces the conflict.

---

## Catalog entry

One TOML file per game, at `<tap-root>/catalog/<platform>/<id>.toml`.

### Common skeleton

```toml
schema_version = 1

[game]
id       = "qfg1-ega"
title    = "Quest for Glory I: So You Want to Be a Hero (EGA)"
platform = "dos"
collection = "quest-for-glory"        # optional; groups related games in the UI
# collection_name = "Quest for Glory"  # omit when auto-format is correct

[meta]
year         = 1989
developer    = "Sierra On-Line"
publisher    = "Sierra On-Line"
genre        = ["adventure", "rpg"]
tags         = ["sierra", "agi", "fantasy"]
description  = "Sierra's hybrid of adventure game and stat-based RPG. Originally released as Hero's Quest."

[acquisition]
gog   = "https://www.gog.com/game/quest_for_glory_so_you_want_to_be_a_hero"
notes = "Bundled in the Quest for Glory 1-5 collection on GOG."

[install]
expects_files = ["SIERRA.BAT", "RESOURCE.000"]

[runtime]
emulator = "dosbox-staging"
sidecars = ["fluidsynth"]

[runtime.dosbox]
config = "qfg1-ega.conf"
entry  = "SIERRA.BAT"
mount  = "c"
```

### `[game]`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | string | yes | Game id. Matches the filename (without `.toml`). |
| `title` | string | yes | Human-readable title as it should appear in the UI. |
| `platform` | string | yes | One of `dos`, `amiga`. Determines which `runtime.*` subtable is consulted. |
| `collection` | string | no | A group key for related games (e.g. `quest-for-glory`). Free-form identifier following the id rules. Pure UI grouping; no semantic effect. |
| `collection_name` | string | no | Human-readable display name for the collection (e.g. `"King's Quest"`). Optional; if absent the UI auto-formats `collection` by replacing hyphens with spaces and title-casing each word. Only needed when the formatted ID would be incorrect (e.g. apostrophes, mixed case). |

### `[meta]`

All fields optional but strongly recommended for discovery. Empty `[meta]` is allowed.

| Field | Type | Description |
| --- | --- | --- |
| `year` | integer | Release year. |
| `developer` | string | Studio that built it. |
| `publisher` | string | Company that released it. |
| `genre` | array of string | Coarse classification. Suggested set: `adventure`, `rpg`, `action`, `arcade`, `strategy`, `simulation`, `puzzle`, `platformer`, `shooter`, `fighting`, `racing`, `sports`, `educational`. |
| `tags` | array of string | Free-form tags for finer filtering. |
| `description` | string | One paragraph for the catalog browser. Plain text; Markdown is not interpreted here. |
| `artwork` | string | Optional display image for the game, as a path **relative to the tap root** (e.g. `art/qfg1-ega.png`). Shown on the game card and detail header. Served read-only over the asset protocol. An image auto-detected in the install directory takes precedence over this (see below). |

**Icon resolution.** The launcher shows one image per game, resolved in this order:

1. **Install directory** — for an installed game, the first match in `~/games/<id>/`: `cover.png`, `cover.jpg`, `icon.png`, `icon.jpg`, `box.png`, `box.jpg`, then the alphabetically-first loose `.png`/`.jpg`/`.bmp` in the directory root.
2. **Tap-provided** — the `[meta] artwork` path above.

If neither resolves, the UI falls back to a colored platform header.

### `[acquisition]`

How a user can legally obtain the game. All fields optional; include what applies.

| Field | Type | Description |
| --- | --- | --- |
| `gog` | string | URL to the GOG listing. |
| `steam` | string | URL to the Steam listing. |
| `developer_site` | string | URL where the developer / rights holder offers the game. |
| `archive` | string | URL to an Internet Archive page if the rights holder has released the game. |
| `amiga_forever` | string | URL to the Amiga Forever (Cloanto) store/product page where the game is available. |
| `notes` | string | Free-form notes: bundle information, abandonware status, regional availability caveats. |

The launcher surfaces these as labelled buttons in the GUI ("Get on GOG", etc.). No auto-fetching; clicking opens the URL in the user's browser.

### `[install]`

Hints for the install flow and the `doctor` command.

| Field | Type | Description |
| --- | --- | --- |
| `expects_files` | array of string | Filenames (no paths) the launcher expects to find in `install_path` after the game is installed. Used to validate that the source held the right files. Case-insensitive matching. |
| `subdir` | string | Optional. A single subfolder *within* the copied/extracted destination that actually holds the game; the recorded `install_path` becomes `<dest>/<subdir>`. Used when one installer ships multiple editions — e.g. the GOG QFG1 `.exe` puts the EGA edition under `EGA/`. Bare filename, no path components. |

The install flow copies/extracts the chosen source into the managed library (default `~/games/<id>`); see "Installation record" below. Source kinds are inferred from the path: a **directory** (recursive copy, any platform), a **`.exe`** (DOS, extracted with `innoextract`), or an Amiga **`.adf`/`.hdf`/`.rp9`** image (`.rp9` is unzipped into the destination).

### `[runtime]`

| Field | Type | Required | Description |
| --- | --- | --- |
| `emulator` | string | yes | Either `dosbox-staging` or `fs-uae`. Must match `[game].platform`. |
| `sidecars` | array of string | no | Auxiliary processes to launch alongside the emulator. v0.1 supports `fluidsynth`. |

### `[runtime.dosbox]` — DOS games only

| Field | Type | Required | Description |
| --- | --- | --- |
| `config` | string | yes | Filename of the sibling `.conf` file in `catalog/dos/`. Relative to tap, no path components. |
| `entry` | string | yes | The file inside the mounted drive that launches the game. Typically a `.BAT` or `.EXE`. |
| `mount` | string | no | Drive letter to mount the install path as. Default `c`. Single ASCII letter. |

### `[runtime.fs_uae]` — Amiga games only

```toml
[runtime.fs_uae]
model       = "a500"
config      = "fatman.fs-uae"   # optional sibling config
floppies    = ["fatman.adf"]    # filenames inside install_path, in disk-swap order
hard_drives = ["system.hdf"]    # filenames inside install_path, mounted as HD0..HD3
```

| Field | Type | Required | Description |
| --- | --- | --- |
| `model` | string | yes | Amiga model: `a500`, `a600`, `a1200`, `a4000`. Determines the model template if no `config` is specified. |
| `config` | string | no | Filename of a sibling `.fs-uae` config in `catalog/amiga/`. If absent, the launcher uses a model template. |
| `floppies` | array of string | no | Filenames (no paths) of `.adf` disk images inside the install path, in disk-swap order. The launcher emulates a single internal drive like a real A500: disk 1 boots in DF0 and all disks form the FS-UAE swap list (`--floppy_image_N`), cycled via the disk menu when the game prompts. |
| `hard_drives` | array of string | no | Filenames (no paths) of `.hdf` hard-disk images inside the install path. Mounted as `--hard_drive_0..3` in order. |

**Autodetect fallback:** when an Amiga entry declares no `config`, `floppies`, or `hard_drives`, the launcher scans `install_path` for a runnable source — preferring an inner `.fs-uae` config, then `.hdf`, then `.adf`. This is what lets a game installed by unzipping a `.rp9` (whose inner filenames vary) launch without per-entry declaration.

---

## Shipped emulator configs

These are siblings of catalog entries in the tap. They contain the per-game tuning and **no user-specific data**.

### DOSBox-Staging (`<id>.conf`)

A normal DOSBox-Staging config file with **no `[autoexec]` section**. The launcher generates the autoexec at runtime from the catalog entry's `[runtime.dosbox]` block plus the installation record's `install_path`.

Example minimal config:

```ini
[sdl]
fullscreen = false

[render]
scaler = normal2x

[cpu]
core   = auto
cycles = fixed 8000

[midi]
mididevice = fluidsynth
midiconfig = 128:0
```

The launcher composes the final config by appending an `[autoexec]` block of the form:

```plaintext
[autoexec]
MOUNT <mount> "<install_path>"
<mount>:
<entry>
EXIT
```

The composition mechanism (temporary file vs `-c` inline commands) is left to the implementation. Whichever yields clearer error messages should win.

### FS-UAE (`<id>.fs-uae`)

A normal FS-UAE config file. The launcher injects floppy and hard-disk paths from the catalog entry's `floppies` / `hard_drives` lists relative to `install_path` before invoking FS-UAE. The shipped file specifies model, chipset, memory, and any per-game quirks.

When no shipped config is present, the launcher uses a model template (e.g. a built-in `a500` profile) and injects the declared disks — or, if none are declared, an autodetected source from `install_path` (see the autodetect fallback above).

---

## Installation record

Per-user, per-game state. Written by the install flow, read at every launch.

**Location:** `${XDG_DATA_HOME:-$HOME/.local/share}/reliquaint/installs/<id>.toml`

```toml
schema_version = 1

[install]
catalog_id   = "qfg1-ega"
tap          = "reliquaint-core"
install_path = "/home/derek/games/qfg1-ega"
installed_at = 2026-05-23T14:32:00Z
```

### Fields

| Field | Type | Required | Description |
| --- | --- | --- |
| `catalog_id` | string | yes | The `[game].id` of the matching catalog entry. |
| `tap` | string | yes | The `id` of the tap the catalog entry came from. Disambiguates when multiple taps offer the same game. |
| `install_path` | string | yes | Absolute path to the directory containing the game's files on this machine. The install flow copies/extracts the game here (default `~/games/<id>`, or `<chosen-library>/<id>`; `+ <subdir>` when the entry declares one). |
| `installed_at` | datetime | yes | TOML datetime of when the install record was created. |

User-side overrides (e.g., adjusting `cycles` for a slower machine, or pointing at a different soundfont) are deferred to implementation. The expected shape is a `[overrides]` table; specifics will be added to this document once the implementation lands.

The launcher refuses to launch a game whose installation record references a `(tap, catalog_id)` pair it cannot find in any subscribed tap. The error surfaces both the missing tap and the missing catalog id.

---

## User launcher config

Per-user, per-machine settings that are independent of any specific game.

**Location:** `${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/config.toml`

```toml
schema_version = 1

[emulators.dosbox-staging]
# Path to the dosbox-staging binary. Empty means "use $PATH".
# Useful for Flatpak invocations: "flatpak run io.github.dosbox-staging"
command = "flatpak run io.github.dosbox-staging"

[emulators.fs-uae]
command = "fs-uae"
kickstart_path = "~/.config/fs-uae/Kickstarts/"
fullscreen = false   # windowed (default); set true to launch fullscreen
window_scale = 3      # integer scale of the native frame when windowed

[sidecars.fluidsynth]
command   = "fluidsynth"
soundfont = "/usr/share/sounds/sf2/FluidR3_GM.sf2"
```

### Fields

All fields under `[emulators.*]` and `[sidecars.*]` are optional. Reasonable Debian defaults are baked into the launcher; the user's config overrides them.

| Field | Type | Description |
| --- | --- | --- |
| `emulators.dosbox-staging.command` | string | Invocation prefix for DOSBox-Staging. May be a multi-word command. |
| `emulators.fs-uae.command` | string | Invocation prefix for FS-UAE. |
| `emulators.fs-uae.kickstart_path` | string | Directory containing the user's Amiga Kickstart ROMs. |
| `emulators.fs-uae.fullscreen` | bool | Launch FS-UAE fullscreen. Default `false` (windowed). |
| `emulators.fs-uae.window_scale` | integer | Integer scale of the native PAL frame when windowed. Default `3`. Ignored when `fullscreen` is set. |
| `sidecars.fluidsynth.command` | string | Invocation prefix for FluidSynth. |
| `sidecars.fluidsynth.soundfont` | string | Path to the SoundFont (`.sf2`) FluidSynth should load. |

The launcher's `doctor` command validates this config: missing binaries, missing kickstart ROMs, missing soundfont files all show up as actionable warnings.

---

## Sidecar handling

A sidecar is an auxiliary process the launcher manages on the game's behalf. v0.1 supports one: `fluidsynth`.

Lifecycle:

1. Before launching the emulator, start the sidecar.
2. Launch the emulator. Block until it exits.
3. After the emulator exits, terminate the sidecar.
4. Clean up.

The launcher's responsibility is process supervision (start, monitor, terminate); the sidecar's configuration lives in the user launcher config under `[sidecars.<name>]`.

FluidSynth in particular needs both a command and a soundfont; both come from user config. The catalog entry just declares "this game needs FluidSynth" via `sidecars = ["fluidsynth"]` in `[runtime]`.

---

## Companion content

Added in v0.4. A tap may ship per-game supplementary material — walkthroughs, maps, hints, install notes — under `companion/<game-id>/`, alongside `catalog/`. Rendering is sandboxed — Markdown only, with raw HTML and remote resources stripped; this section documents only the on-disk layout the launcher discovers.

```plaintext
<tap-root>/
  companion/
    <game-id>/
      01-overview.md                    # Markdown content
      02-walkthrough.md
      maps/                             # a subdirectory is a named section
        spielburg.png                   # image referenced from Markdown
      hints/
        boss-fight-tips.md
```

There is **no `companion.toml` index file** in v0.4 — directory walking plus the conventions below is sufficient. Revisit only if explicit ordering or title overrides become necessary.

**Recognized file types**

- `.md` — Markdown (walkthroughs, hints, install notes, overviews).
- `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp` — images referenced from Markdown.
- **SVG is excluded** — it is structured XML that can carry `<script>` and event handlers. Rasterize maps/diagrams to PNG or WebP.

Files with any other extension are ignored.

**Titles** are derived from filenames: drop the extension, strip a leading numeric sort prefix (`NN-` / `NN_`), turn `-`/`_` into spaces, and capitalize each word. `01-walkthrough.md` → "Walkthrough"; `boss-fight-tips.md` → "Boss Fight Tips". (Digits not followed by a separator, e.g. `1992.md`, are kept verbatim — not treated as a prefix.)

**Ordering** within a directory: numeric-prefixed files first, by numeric value (`01-` before `02-`); non-prefixed files follow, alphabetically.

**Sections:** one level of subdirectory under `<game-id>/` is a first-class grouping; the subdirectory name (`maps`, `hints`) is the section label. Root-level files appear first, then each section in alphabetical order. Deeper nesting is not indexed.

**Across taps:** when more than one subscribed tap ships companion content for the same game, the items aggregate (they are not conflicts). The launcher groups them by tap in subscription-priority order, preserving each tap's internal ordering, and each item keeps its tap-of-origin for attribution.

---

## Future schema concerns

These are explicitly **out of scope for v0.1** and noted here so the v1 schema does not paint into a corner.

- **WHDLoad and hard-drive Amiga installs**: needs its own ADR. Expected to live as additional `[runtime.fs_uae]` fields or a new subtable.
- **ScummVM backend**: would introduce `platform = "scummvm"` and a `[runtime.scummvm]` subtable. Not in v0.1.
- **Per-game save state location**: most current targets save inside the install path, which works. If a game saves outside its install dir, a future `[saves]` table can capture it.
- **Multiple entry points per game**: handled in v0.1 by shipping separate catalog entries with distinct ids (e.g. `qfg1-ega`, `qfg1-vga`). If a single catalog entry needs multiple launch modes (e.g., "with mouse" vs "without"), a future `[runtime.modes]` table will support that.
