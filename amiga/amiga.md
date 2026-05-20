# Amiga Collection

This guide covers running Amiga software in [FS-UAE](https://fs-uae.net/) using a single generic launcher script. Unlike the DOS collections, there is no per-game script — drop your Amiga files into `amiga/games/` and launch any of them by path.

## Prerequisites

Complete the steps in [docs/prerequisites.md](../docs/prerequisites.md), specifically the FS-UAE section.

## Kickstart ROMs

FS-UAE needs the Amiga firmware (Kickstart ROMs) to boot software. These are copyrighted and not distributed with FS-UAE — you must supply your own legally obtained copies (for example, via the [Amiga Forever](https://www.amigaforever.com/) package from Cloanto).

FS-UAE searches `~/Documents/FS-UAE/Kickstarts/` by default. Create that directory and place your ROMs there:

```bash
mkdir -p ~/Documents/FS-UAE/Kickstarts/
cp /path/to/your/kickstart-*.rom ~/Documents/FS-UAE/Kickstarts/
```

For the A500 config template, FS-UAE expects a Kickstart 1.3 ROM. For the A1200 template, a Kickstart 3.1 ROM. FS-UAE will print a clear error at launch if the required ROM is missing.

## Supported file formats

| Format | What it is | How the launcher handles it |
|---|---|---|
| `.adf` | Raw Amiga floppy disk image (single disk). | Paired with a model config (default: A500); inserted into floppy drive 0. |
| `.rp9` | RetroPlatform bundle — a zip containing an `rp9-manifest.xml`, optional FS-UAE config, and one or more disk images. | Unpacked to a temp directory. If the bundle ships a `.fs-uae` config, that is used directly. Otherwise the first inner `.adf` is paired with the selected model config (`--model`, default `a500`), same as a raw `.adf` file. The `rp9-manifest.xml` is not parsed. |

WHDLoad (`.hdf`, `.lha`) and multi-disk games requiring disk swapping are not covered by Phase 1 of this collection.

## Adding games

Drop your `.adf` or `.rp9` files into `amiga/games/`:

```bash
cp /path/to/lemmings.adf amiga/games/
```

Files in `amiga/games/` are gitignored.

## Running a game

Each Amiga game requires a per-game manifest in `amiga/manifests/<id>.toml` that points at the game file and declares the Amiga model. See `amiga/manifests/example.toml.disabled` for the full schema.

Once a manifest is in place, launch the game with:

```bash
classic-launcher run <id>
```

For example, if you have a manifest `amiga/manifests/lemmings.toml` with `id = "lemmings"`:

```bash
classic-launcher run lemmings
```

For a windowed session (instead of fullscreen):

```bash
classic-launcher run lemmings --windowed
```

To preview the FS-UAE command without launching:

```bash
classic-launcher run lemmings --dry-run
```

If the game file is an `.rp9` bundle that ships its own FS-UAE config, `classic-launcher` uses it directly. Otherwise it pairs the `.adf` or inner `.adf` from the `.rp9` with the model config declared in the manifest (`model = "a500"` or `"a1200"`).

## Model config templates

| File | Amiga model | Kickstart | Use for |
|---|---|---|---|
| `amiga/config/a500.fs-uae` | A500 | 1.3 (OCS) | Most late-80s Amiga games. |
| `amiga/config/a1200.fs-uae` | A1200 | 3.1 (AGA) | Early/mid-90s AGA-required games. |

Both templates default to fullscreen. To tweak a template (e.g. change audio settings, switch back to windowed), edit the file directly. Changes apply on the next launch.

## Troubleshooting

- **"Kickstart ROM not found"** — FS-UAE could not locate a Kickstart ROM in `~/Documents/FS-UAE/Kickstarts/`. Verify the file is there and named consistently with what FS-UAE expects (filenames don't have to match exactly; FS-UAE inspects the ROM contents).
- **Game launches but mouse/keyboard input doesn't reach the Amiga** — click inside the FS-UAE window to grab input. Press `F12` to open the in-emulator menu.
- **`classic-launcher: command not found`** — ensure `~/.cargo/bin` is on your `PATH`. Run `source "$HOME/.cargo/env"` or add it to your shell profile, then reinstall: `cargo install --path launcher/src-tauri`.
