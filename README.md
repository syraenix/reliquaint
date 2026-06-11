# Reliquaint

> *REL-i-kwaynt* — a portmanteau of *relic* and *quaint*.

[![CI](https://github.com/syraenix/reliquaint/actions/workflows/ci.yml/badge.svg)](https://github.com/syraenix/reliquaint/actions/workflows/ci.yml)
[![Release](https://github.com/syraenix/reliquaint/actions/workflows/release.yml/badge.svg)](https://github.com/syraenix/reliquaint/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/syraenix/reliquaint)](https://github.com/syraenix/reliquaint/releases/latest)
[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)

A preservation hub for classic DOS and Amiga games that happens to launch. One catalog, one install command, one launch button — plus the maps, hint sheets, and walkthroughs that gave these games their flavour, rendered right alongside each game.

<p align="center">
  <img src="launcher/src-tauri/icons/icon.png" alt="Reliquaint" />
</p>

## What it is

Reliquaint is a Linux launcher built on top of [DOSBox-Staging](https://www.dosbox-staging.org/) (DOS) and [FS-UAE](https://fs-uae.net/) (Amiga). You bring your own legally-acquired game files; Reliquaint handles the configuration, mounting, sidecars (FluidSynth for MIDI), and a small catalog of curated metadata.

Subscribe to the official [`reliquaint-core`](https://github.com/syraenix/reliquaint-core) tap to get a curated starter set of classic DOS and Amiga titles — Quest for Glory I–IV, King's Quest I–V, Space Quest I–VI, Police Quest 1–4, and the [Amiga Forever](https://www.amigaforever.com/) game collection. The launcher's tap model supports multiple community-maintained taps with priority-based conflict resolution.

Taps can also ship **companion content** — per-game walkthroughs, maps, and hint sheets, written in Markdown and browsable in a "Guides" panel on each game's page. It renders entirely offline through a strict, sandboxed pipeline (no remote resources, no scripts).

It does **not** acquire game files for you. It does **not** circumvent DRM. The user provides their own copies; the launcher tells them where to put them.

## Installing the launcher

Prerequisites are documented in [`docs/prerequisites.md`](docs/prerequisites.md) — DOSBox-Staging (Flatpak), FluidSynth, and FS-UAE.

### Prebuilt packages

Each [release](https://github.com/syraenix/reliquaint/releases/latest) attaches prebuilt `.deb` (Debian/Ubuntu/Raspberry Pi OS) and `.rpm` (Fedora/RHEL) packages for both x86_64 and aarch64/arm64 architectures. An AppImage for cross-distro compatibility is also attached for x86_64 only (arm64 ships as `.deb`/`.rpm`).

Install with `sudo apt install ./Reliquaint_<version>_<arch>.deb` or `sudo dnf install ./Reliquaint-<version>-1.<arch>.rpm`. Note that on arm64 we only guarantee the launcher itself *builds* — actually running games still depends on the emulator stack (DOSBox-Staging, FS-UAE, FluidSynth) being available for your architecture; see [`docs/prerequisites.md`](docs/prerequisites.md).

### Building from source

```bash
git clone https://github.com/syraenix/reliquaint.git
cd reliquaint
cargo install --path launcher/src-tauri
```

This installs the `reliquaint` binary to `~/.cargo/bin/`. Make sure that's on your `PATH`.

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

### Adding a game *not* in a subscribed tap

For DOS or Amiga games we don't have a manifest for, point the wizard at the directory and it will inspect, propose a draft, and write it to your local tap at `${XDG_CONFIG_HOME:-$HOME/.config}/reliquaint/tap/`:

```bash
reliquaint add ~/games/my-custom-game            # interactive prompt
reliquaint add ~/games/my-custom-game --yes      # accept the draft as-is
reliquaint add ~/games/my-custom-game --platform amiga   # override detection
```

Or in the GUI: click the **+ Add game** button in the header, pick the directory, review the form, save.

The new entry shows up alongside subscribed-tap games with a small **CUSTOM** badge. Tweak it later via:

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

## Subscribing to taps

The first time you run Reliquaint, a prompt offers to subscribe to the official `reliquaint-core` tap. You can also manage subscriptions manually:

```bash
reliquaint tap add reliquaint-core      # subscribe to the official tap
reliquaint tap add https://github.com/…/my-tap   # or any URL
reliquaint tap list                     # show subscribed taps
reliquaint tap sync                     # pull latest commits for all taps
reliquaint tap remove my-tap            # unsubscribe
```

The Taps panel (⊞ Taps button in the GUI) provides the same controls visually.

## Contributing a catalog entry

Catalog contributions go to the [`reliquaint-core`](https://github.com/syraenix/reliquaint-core) repository. Clone it, drop a TOML file into `catalog/<platform>/<id>.toml` matching the schema in [`docs/schema.md`](docs/schema.md), add any per-game shipped `.conf` next to it, and open a PR there. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full walkthrough.

## Further reading

- [`docs/schema.md`](docs/schema.md) — TOML schemas (catalog entries, install records, tap metadata, user config)

## License

- **Code** — [MPL-2.0](LICENSE). Weak copyleft: file-level, lets the launcher integrate with proprietary or other-licensed code while keeping modifications to Reliquaint's own files open.
- **Catalog content** — catalog entries and shipped configs live in the [`reliquaint-core`](https://github.com/syraenix/reliquaint-core) repository under [CC-BY-SA-4.0](https://creativecommons.org/licenses/by-sa/4.0/): reuse with attribution; derivative tap repositories must share-alike.
