# Quest for Glory - DOSBox Staging Guide

[Quest for Glory](https://en.wikipedia.org/wiki/Quest_for_Glory) is a series of hybrid adventure/role-playing video games, which were designed by Corey and Lori Ann Cole for [Sierra Entertainment](https://en.wikipedia.org/wiki/Sierra_Entertainment).

This guide will walkthrough downloading, installing, and configuring Quest for Glory 1-4 with [DOSBox Staging](https://www.dosbox-staging.org/). 

> Quest for Glory 5 is not a DOS game and will need to be installed and configured differently.

## Prerequisites

Follow the steps in the [prerequisites guide](docs/prerequisites.md) to install the software required to complete the game guides.

## Downloading and extracting game files

Quest for Glory 1-5 can be purchased and downloaded via [GOG](https://www.gog.com/en/game/quest_for_glory). Once downloaded, the game installers will need to extracted with `innoextract`.

> For GOG installers, you will want to make sure you are downloading the files under the "Download Offline Backup Game Installers" and **not** the GOG Galaxy installer.

Once downloaded, you should have 5 installer files; one for each game in the series. We will now extract the game files from the installers using `innoextract`.

### Extracting games installers

The installer for Quest for Glory 1 includes versions of the game: the original, EGA, version of the game and the VGA remake.

You will need to rename the installers to match the list below, and copy them to the [installers](installers) directory.

- Quest for Glory 1 => qfg1.exe
- Quest for Glory 2 => qfg2.exe
- Quest for Glory 3 => qfg3.exe
- Quest for Glory 4 => qfg4.exe

#### Using the launcher GUI (recommended)

Open `reliquaint` (run it with no arguments), click the **Install** button in the header, and select the **Quest for Glory** tab. The launcher defaults to this collection's `installers/` directory; clicking **Install selected** runs `innoextract` for each game and streams the output inline. After completion, every Quest for Glory entry in the **Doctor** panel turns green.

#### Terminal alternative

```bash
cd scripts/

# make the `extract-installers` script executable
chmod +x ./extract-installers.sh
# run the script (extracts game files directly into ~/games/)
./extract-installers.sh
```

Once the script has finished running, the game files will be extracted directly to `~/games/` for the launcher to use.


## Running the games

`reliquaint` starts FluidSynth (so MIDI works), then launches DOSBox Staging with the matching per-game config. The shipped config carries no `[autoexec]`; the launcher composes one at launch that mounts `~/games/<game>` as `c:` and runs the game executable, so no manual `mount` or `cd` is needed inside DOSBox.

```bash
reliquaint run qfg1-vga
reliquaint run qfg1-ega
reliquaint run qfg2
reliquaint run qfg3
reliquaint run qfg4
```

The games must live at `~/games/<game>`. Use the launcher's **Install** panel — or run `./scripts/extract-installers.sh` — once during extraction (see [Extracting games installers](#extracting-games-installers)) to put them there.

To preview the commands that would be run without launching anything:

```bash
reliquaint run qfg1-ega --dry-run
```
