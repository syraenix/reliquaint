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
| `.rp9` | RetroPlatform bundle — self-contained zip with config, ROMs metadata, and disk images. | Passed directly to FS-UAE, which reads model + config from the bundle. |

WHDLoad (`.hdf`, `.lha`) and multi-disk games requiring disk swapping are not covered by Phase 1 of this collection.

## Adding games

Drop your `.adf` or `.rp9` files into `amiga/games/`:

```bash
cp /path/to/lemmings.adf amiga/games/
```

Files in `amiga/games/` are gitignored.

## Running a game

From `amiga/scripts/`:

```bash
cd amiga/scripts
./amiga-run.sh ../games/lemmings.adf
```

For an A1200-targeted game:

```bash
./amiga-run.sh --model a1200 ../games/pinball-illusions.adf
```

For an `.rp9` bundle (the model flag is ignored — the bundle drives the config):

```bash
./amiga-run.sh ../games/turrican.rp9
```

To preview the FS-UAE command without launching:

```bash
./amiga-run.sh --dry-run ../games/lemmings.adf
```

## Model config templates

| File | Amiga model | Kickstart | Use for |
|---|---|---|---|
| `amiga/config/a500.fs-uae` | A500 | 1.3 (OCS) | Most late-80s Amiga games. |
| `amiga/config/a1200.fs-uae` | A1200 | 3.1 (AGA) | Early/mid-90s AGA-required games. |

To tweak a template (e.g. enable fullscreen, change audio settings), edit the file directly. Changes apply on the next launch.

## Troubleshooting

- **"Kickstart ROM not found"** — FS-UAE could not locate a Kickstart ROM in `~/Documents/FS-UAE/Kickstarts/`. Verify the file is there and named consistently with what FS-UAE expects (filenames don't have to match exactly; FS-UAE inspects the ROM contents).
- **Game launches but mouse/keyboard input doesn't reach the Amiga** — click inside the FS-UAE window to grab input. Press `F12` to open the in-emulator menu.
- **`./amiga-run.sh: command not found`** — the script needs to be executable. Run `chmod +x amiga/scripts/amiga-run.sh`.
