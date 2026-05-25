# Amiga Collection

This guide covers running Amiga software in [FS-UAE](https://fs-uae.net/) through Reliquaint. Amiga games install and launch exactly like the DOS collections: install your own game file against a catalog entry, then launch by id. The bundled `reliquaint-core` tap currently ships one Amiga entry — *Fatman: The Caped Consumer* (`fatman`).

## Prerequisites

Complete the steps in [docs/prerequisites.md](../docs/prerequisites.md), specifically the FS-UAE section.

## Kickstart ROMs

FS-UAE needs the Amiga firmware (Kickstart ROMs) to boot software. These are copyrighted and not distributed with FS-UAE — you must supply your own legally obtained copies (for example, via the [Amiga Forever](https://www.amigaforever.com/) package from Cloanto).

FS-UAE searches `~/Documents/FS-UAE/Kickstarts/` by default. Create that directory and place your ROMs there:

```bash
mkdir -p ~/Documents/FS-UAE/Kickstarts/
cp /path/to/your/kickstart-*.rom ~/Documents/FS-UAE/Kickstarts/
```

A typical late-80s OCS title (the A500 default) expects a Kickstart 1.3 ROM; AGA titles expect Kickstart 3.1. FS-UAE will print a clear error at launch if the required ROM is missing.

## Supported file formats

| Format | What it is | How the launcher handles it |
|---|---|---|
| `.adf` | Raw Amiga floppy disk image. | Inserted into the internal floppy drive; multiple `.adf`s declared in the entry become a disk-swap list. |
| `.hdf` | Amiga hard-disk image (e.g. WHDLoad installs). | Mounted as a hard drive (`HD0`..`HD3`). |
| `.rp9` | RetroPlatform bundle — a zip containing an `rp9-manifest.xml`, optional FS-UAE config, and one or more disk images. | Unzipped into the install directory. If the bundle ships a `.fs-uae` config it is used directly; otherwise the launcher autodetects a runnable source (inner `.fs-uae`, then `.hdf`, then `.adf`). The `rp9-manifest.xml` is not parsed. |

Multi-disk games are supported via the entry's `floppies` swap list; smooth mid-play swapping depends on the individual game.

## Installing an Amiga game

You provide your own legally acquired game file (`.adf`, `.hdf`, or `.rp9`). Install it against the catalog entry with `reliquaint install <id> <source>` — the launcher copies/unpacks it into the managed library at `~/games/<id>/`:

```bash
reliquaint install fatman /path/to/fatman.adf
```

You can also use the GUI's **Install** panel, which streams progress inline. After installing, the entry shows up green in the **Doctor** panel (`reliquaint doctor`).

## Running a game

```bash
reliquaint run fatman
```

The launcher reads the Amiga model and any declared floppies/hard drives from the catalog entry (`runtime.fs_uae` — see [docs/schema.md](../docs/schema.md)), composes the FS-UAE command, and launches. When an entry declares no config, floppies, or hard drives (common for `.rp9` installs, whose inner filenames vary), the launcher autodetects a runnable source from the install directory.

Display defaults to a windowed, integer-scaled 4:3 frame. Override it via the `[emulators.fs-uae]` block in your user config (`fullscreen`, `window_scale`).

To preview the FS-UAE command without launching:

```bash
reliquaint run fatman --dry-run
```

## Troubleshooting

- **"Kickstart ROM not found"** — FS-UAE could not locate a Kickstart ROM in `~/Documents/FS-UAE/Kickstarts/`. Verify the file is there. Filenames don't have to match exactly; FS-UAE inspects the ROM contents.
- **Game launches but mouse/keyboard input doesn't reach the Amiga** — click inside the FS-UAE window to grab input. Press `F12` to open the in-emulator menu.
- **`reliquaint: command not found`** — ensure `~/.cargo/bin` is on your `PATH`. Run `source "$HOME/.cargo/env"` or add it to your shell profile, then reinstall: `cargo install --path launcher/src-tauri`.
