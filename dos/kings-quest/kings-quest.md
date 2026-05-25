# King's Quest - DOSBox Staging Guide

[King's Quest](https://en.wikipedia.org/wiki/King%27s_Quest) is a series of adventure games developed by [Sierra On-Line](https://en.wikipedia.org/wiki/Sierra_Entertainment). This guide walks through copying, configuring, and running King's Quest 1 (SCI remake) through King's Quest 5 with [DOSBox Staging](https://www.dosbox-staging.org/).

> The original AGI version of King's Quest 1 (1984) is not covered here; this guide uses the 1990 SCI remake (`kq1sci`). King's Quest 6 is omitted: the King's Quest Collection ships only its Windows hi-res interpreter (`DOKQ6.EXE`, with no DOS executable), so it can't run under DOSBox. King's Quest 7 and 8 are not DOS games and need to be installed and configured differently.

## Prerequisites

Follow the steps in the [prerequisites guide](../../docs/prerequisites.md) to install the software required to complete the game guides.

> `innoextract` is not needed for this collection — game files come from a Steam install rather than a GOG offline installer, so there is nothing to extract.

## Obtaining the game files

The King's Quest games covered here are available on Steam as the [King's Quest Collection](https://store.steampowered.com/app/10100), a single package that bundles:

- KQ1 — the SCI remake (1990)
- KQ2 — the AGI original (1985)
- KQ3 — the AGI original (1986)
- KQ4 — the SCI version (1988)
- KQ5 (1990)

The collection also includes KQ6 (1992), but that title isn't covered here (see the note above). Purchase and install the collection via Steam. Steam installs it as ready-to-run game folders, not as DOS installers, so the workflow is to copy the DOS game files from the Steam install directory directly into `~/games/`.

### Locating the Steam install directory

On Linux, Steam typically installs games under one of:

- `~/.steam/steam/steamapps/common/`
- `~/.local/share/Steam/steamapps/common/`

The collection installs to a folder such as `King's Quest Collection/`. Inside it you'll find one subfolder per game (`kq1sci/`, `kq2/`, `kq3/`, `kq4/`, `kq5/`).

### Copying game files to ~/games

Each game ends up at `~/games/<id>` using the folder names below. The DOSBox configs and the launcher depend on these exact names:

- King's Quest 1 (SCI remake) => `~/games/kq1sci/`
- King's Quest 2 => `~/games/kq2/`
- King's Quest 3 => `~/games/kq3/`
- King's Quest 4 (SCI) => `~/games/kq4/`
- King's Quest 5 => `~/games/kq5/`

> The exact path of the DOS files inside the Steam folder varies by game — some Steam packages put them at the top level, others bury them in a subfolder. The source you pick (or copy) must contain the game's `.EXE`/`.COM` launcher and resource files at its top level (e.g. the folder that holds `SCIKQ5.EXE`, not its parent).

#### Using the launcher GUI (recommended)

Open `reliquaint` (run it with no arguments), click the **Install** button in the header, and select the **King's Quest** tab. For each game, click **Pick folder…** and navigate into the `King's Quest Collection/` Steam folder to select the subfolder that contains that game's DOS files. Once you've picked at least one game, click **Install** — the launcher copies each picked folder into `~/games/<id>/` and streams progress inline.

#### Terminal alternative

Manually `cp -r` each game's Steam folder into `~/games/<id>` (using the id list above). Make sure the destination ends up containing the game's executable directly.


## Configuration

Each game has its own [config](config) file. The configs diverge from [`config/default-dosbox-staging.conf`](../../config/default-dosbox-staging.conf) deliberately — cycles are tuned per engine era, the [autoexec] section mounts and launches each game, and MIDI is routed to FluidSynth via `midiconfig=128:0`.

Starting cycle values are picked to roughly match each game's target hardware. If a game runs too fast (action sequences are unplayable) or too slow (music stutters), edit the `cycles=fixed N` line in the relevant config:

| Game | Engine | Starting cycles |
|------|--------|-----------------|
| kq1sci | SCI0 | 5000 |
| kq2 | AGI | 1500 |
| kq3 | AGI | 1500 |
| kq4 | SCI0 | 5000 |
| kq5 | SCI1 | 6000 |

## Running the games

`reliquaint` starts FluidSynth (so MIDI works), then launches DOSBox Staging with the matching per-game config. The shipped config carries no `[autoexec]`; the launcher composes one at launch that mounts `~/games/<game>` as `c:` and runs the game executable, so no manual `mount` or `cd` is needed inside DOSBox.

```bash
reliquaint run kq1sci
reliquaint run kq2
reliquaint run kq3
reliquaint run kq4
reliquaint run kq5
```

The games must live at `~/games/<game>`. Copy the game files from Steam directly into `~/games/<game>` (see [Copying game files to ~/games](#copying-game-files-to-games)) before launching.

To preview the commands that would be run without launching anything:

```bash
reliquaint run kq1sci --dry-run
```
