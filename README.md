# Reliquaint

> *REL-i-kwaynt* — a portmanteau of *relic* and *quaint*.

[![CI](https://github.com/syraenix/reliquaint/actions/workflows/ci.yml/badge.svg)](https://github.com/syraenix/reliquaint/actions/workflows/ci.yml)
[![Release](https://github.com/syraenix/reliquaint/actions/workflows/release.yml/badge.svg)](https://github.com/syraenix/reliquaint/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/syraenix/reliquaint)](https://github.com/syraenix/reliquaint/releases/latest)
[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)

A preservation hub for classic DOS and Amiga games that happens to launch. One catalog, one install command, one launch button — plus the future scaffolding for the maps, hint sheets, and manuals that gave these games their flavour.

<p align="center">
  <img src="launcher/src-tauri/icons/icon.png" alt="Reliquaint" />
</p>

## What it is

Reliquaint is a Linux launcher built on top of [DOSBox-Staging](https://www.dosbox-staging.org/) (DOS) and [FS-UAE](https://fs-uae.net/) (Amiga). You bring your own legally-acquired game files; Reliquaint handles the configuration, mounting, sidecars (FluidSynth for MIDI), and a small catalog of curated metadata.

The bundled `reliquaint-core` tap ships with 18 entries — Quest for Glory I–IV, King's Quest I (SCI remake) through V, Space Quest I–VI (including the SQ1 VGA remake), and the Amiga single-disk *Fatman: The Caped Consumer*. The launcher's tap model (per [ADR-0003](docs/adr-0003-tap-based-distribution.md)) is designed for community-maintained additions in future versions.

It does **not** acquire game files for you. It does **not** circumvent DRM. The user provides their own copies; the launcher tells them where to put them.

## Status

**v0.1.** The CLI is feature-complete against the v0.1 design (`list`, `run`, `install`, `migrate-installs`, `doctor`). The GUI is end-to-end usable for browsing, installing, and launching. The bundled tap is populated. Companion content (walkthroughs, maps, hint files) and tap subscription are explicit non-goals for v0.1 — they land in v0.4 and v0.3 respectively per the [PRD](docs/prd.md).

The project is Linux-only by design (Flatpak DOSBox-Staging, apt-installed FluidSynth/FS-UAE). Cross-platform support is a non-goal for v0.1.

## Installing the launcher

Prerequisites are documented in [`docs/prerequisites.md`](docs/prerequisites.md) — DOSBox-Staging (Flatpak), FluidSynth, FS-UAE, and the Rust toolchain.

```bash
git clone https://github.com/syraenix/reliquaint.git
cd reliquaint
cargo install --path launcher/src-tauri
```

This installs the `reliquaint` binary to `~/.cargo/bin/`. Make sure that's on your `PATH`.

For the GUI, you'll additionally need Node.js + pnpm + GTK/webkit system libs:

```bash
cd launcher && pnpm install && pnpm tauri build
```

## Adding a game you own

The launcher doesn't ship game files. Once you've obtained your own copy (see each entry's "How to obtain" buttons in the GUI for legitimate sources):

```bash
# Extract / copy the game's files into a directory of your choice, then:
reliquaint install qfg1-ega ~/games/qfg1-ega
reliquaint run qfg1-ega
```

If you have games already laid out at `~/games/<id>/` from before:

```bash
reliquaint migrate-installs
```

This scans for any catalog entry whose id matches a subdirectory of `~/games/` and registers each one in a single pass.

### Adding a game *not* in the bundled catalog

For DOS or Amiga games we don't have a manifest for, point the wizard at the directory and it will inspect, propose a draft, and write it to your local tap at `${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/tap/`:

```bash
reliquaint add ~/games/my-custom-game            # interactive prompt
reliquaint add ~/games/my-custom-game --yes      # accept the draft as-is
reliquaint add ~/games/my-custom-game --platform amiga   # override detection
```

Or in the GUI: click the **+ Add game** button in the header, pick the directory, review the form, save.

The new entry shows up alongside bundled games with a small **CUSTOM** badge. Tweak it later via:

```bash
reliquaint where my-custom-game     # print on-disk paths for hand-editing
reliquaint remove my-custom-game    # delete the manifest, sibling .conf, and install record
```

When the manifest works well for you, send it upstream so others can use it:

```bash
reliquaint submit my-custom-game --clipboard
# Then paste into tap/catalog/<platform>/<id>.toml in a PR against this repo.
```

Or in the GUI: open the entry's detail view and click **Submit upstream** — it copies the canonical manifest to your clipboard and opens GitHub on the right "new file" path.

## Browsing the catalog

```bash
reliquaint list                       # tabular, grouped by collection
reliquaint list --platform dos
reliquaint list --installed
reliquaint list --format json         # for scripting
reliquaint                            # opens the GUI
```

The GUI gives you per-entry cards with year + developer, full metadata + description on detail view, "Get on GOG / Steam / …" buttons drawn from the catalog's `[acquisition]` block, and a live diagnostic panel that streams launcher tracing events alongside the emulator's own stdout/stderr.

## Diagnostics

```bash
reliquaint doctor          # host deps + per-install checks + orphan records
reliquaint -v list         # tracing at DEBUG; use -vv for TRACE
RUST_LOG=trace reliquaint list
```

In the GUI, the ⚕ Doctor button in the header surfaces the same checks plus per-row "Fix this" buttons for missing apt/flatpak dependencies.

## Contributing a catalog entry

Drop a TOML file into `tap/catalog/<platform>/<id>.toml` matching the schema in [`docs/schema.md`](docs/schema.md), copy any per-game shipped `.conf` next to it (no `[autoexec]` block — the launcher composes that at launch), and open a PR. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full walkthrough.

## Further reading

- [`docs/prd.md`](docs/prd.md) — product vision and scope
- [`docs/schema.md`](docs/schema.md) — TOML schemas (catalog entries, install records, tap metadata, user config)
- [`docs/v0.1-tasks.md`](docs/v0.1-tasks.md) — the v0.1 task list (kept for historical reference)
- ADRs in [`docs/`](docs/): two-layer manifest model (0001), split DOSBox config (0002), tap-based distribution (0003), logging (0004), error handling (0005)

## License

- **Code** — [MPL-2.0](LICENSE). Weak copyleft: file-level, lets the launcher integrate with proprietary or other-licensed code while keeping modifications to Reliquaint's own files open.
- **Catalog content** — [CC-BY-SA-4.0](LICENSE-CONTENT). Anyone can reuse the catalog entries and shipped configs with attribution; derivative tap repositories must share-alike.
