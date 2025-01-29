# Quest for Glory Collection - DOSBox Staging Guide

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

In order to use the [extract-installers](scripts/extract-installers.sh) script, you will need to rename the installers to match the list below. Once the installers are renamed, they should be copied to the `/installers` directory.

- Quest for Glory 1 => qfg1.exe
- Quest for Glory 2 => qfg2.exe
- Quest for Glory 3 => qfg3.exe
- Quest for Glory 4 => qfg4.exe

Extract the game files with the following commands:

```bash
cd scripts/

# make the `extract-installers` script executable
chmod +x ./extract-installers.sh
# run the script
./extract-installers.sh
```

Once the script has finished running, the `/games` directory should contain the extracted games files.

## Installation

### Quest for Glory I: So You Want to Be a Hero

### Quest for Glory II: Trial By Fire

### Quest for Glory III: Wages of War

### Quest for Glory IV: Shadows of Darkness

## Configuration

## Running the games
