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

In order to use the [extract-installers](scripts/extract-installers.sh) script, you will need to rename the installers to match the list below. Once the installers are renamed, they should be copied to the [installers](installers) directory.

- Quest for Glory 1 => qfg1.exe
- Quest for Glory 2 => qfg2.exe
- Quest for Glory 3 => qfg3.exe
- Quest for Glory 4 => qfg4.exe

Extract the game files with the following commands:

```bash
cd scripts/

# make the `extract-installers` script executable
chmod +x ./extract-installers.sh
# run the script (pass -c to also copy games into ~/games, where the run scripts expect them)
./extract-installers.sh -c
```

Once the script has finished running, the [games](games) directory should contain the extracted games files. If you passed `-c`, the same game directories will also be copied under `~/games/` for the run scripts to use.

## Running the games

Each game has a launch script in [scripts](scripts) that starts FluidSynth (so MIDI works), then launches DOSBox Staging with the matching per-game config. The config's `[autoexec]` section auto-mounts `~/games/<game>` as `c:` and runs the game executable, so no manual `mount` or `cd` is needed inside DOSBox.

```bash
cd scripts/

# make the run scripts executable (only needed once)
chmod +x ./*-run.sh

# launch a game
./qfg1-vga-run.sh
./qfg1-ega-run.sh
./qfg2-run.sh
./qfg3-run.sh
./qfg4-run.sh
```

The run scripts expect the games to live at `~/games/<game>`. Run `./extract-installers.sh -c` once during extraction (see [Extracting games installers](#extracting-games-installers)) to put them there.
