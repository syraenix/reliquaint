# Prerequisites

In order to finish this guide, you will need to install several prerequisites. This guide assumes you will be using a Debian-based Linux distro. If you are using a different distro, change the listed commands, as necessary.

## DOSBox Staging

[DOSBox Staging](https://www.dosbox-staging.org/) is a modern continuation of DOSBox. Existing DOSBox configurations will continue to work as expected.

[Flatpak](https://flatpak.org/setup/Debian) is the easiest way to install DOSBox Staging. If you do not have Flatpak setup, follow the [Debian quick setup guide](https://flatpak.org/setup/Debian).

Once Flatpak is configured, you can install DOSBox Staging by running the following command:

```bash
$ flatpak install flathub io.github.dosbox-staging
```

## FluidSynth

[FluidSynth](https://github.com/FluidSynth/fluidsynth) is a cross-platform, real-time software synthesizer and will be used to provide audio for the games.

FluidSynth can be installed by running the following command:

```bash
$ sudo apt install fluidsynth
```

## innoextract

[innoextract](https://github.com/dscharrer/innoextract) is a tool to unpack installers created by Inno Setup. [Inno Setup](https://jrsoftware.org/isinfo.php) is a tool to create installers for Microsoft Windows applications. This tool will be used to extract the Quest for Glory installers.

innoextract can be installed by running the following command:

```bash
$ sudo apt install innoextract
```

## FS-UAE

[FS-UAE](https://fs-uae.net/) is a cross-platform Amiga emulator based on WinUAE. It is used for the Amiga collection.

FS-UAE can be installed by running the following command:

```bash
$ sudo apt install fs-uae
```

FS-UAE itself does not include Kickstart ROMs (the Amiga's firmware), which are required to boot most Amiga software. You must supply your own legally obtained Kickstart ROMs. The [Amiga collection guide](../amiga/amiga.md) explains where FS-UAE expects them.