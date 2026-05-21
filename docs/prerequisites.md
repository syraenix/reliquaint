# Prerequisites

In order to finish this guide, you will need to install several prerequisites. This guide assumes you will be using a Debian-based Linux distro. If you are using a different distro, change the listed commands, as necessary.

> **Tip:** Once `classic-launcher` is installed (see the [Rust toolchain](#rust-toolchain) section below), the GUI's **Setup** panel can install most of the dependencies on this page for you — one **Fix this** button per dependency. The manual steps below remain the source of truth and the fallback for non-Debian distros.

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
$ sudo apt install fs-uae unzip
```

`unzip` is used by the Amiga launcher script to read RetroPlatform `.rp9` bundles. It is preinstalled on most Debian systems; the command above is a no-op if it's already present.

FS-UAE itself does not include Kickstart ROMs (the Amiga's firmware), which are required to boot most Amiga software. You must supply your own legally obtained Kickstart ROMs. The [Amiga collection guide](../amiga/amiga.md) explains where FS-UAE expects them.

## Rust toolchain

`classic-launcher` is a Rust binary that replaces the per-game shell scripts. You need the Rust toolchain to build and install it.

Install [rustup](https://rustup.rs/) (the Rust toolchain installer):

```bash
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Accept the defaults. After installation, source the environment or open a new shell:

```bash
$ source "$HOME/.cargo/env"
```

You also need a C linker (required by the Rust compiler):

```bash
$ sudo apt install gcc
```

Build and install `classic-launcher`:

```bash
$ cargo install --path launcher/src-tauri
```

This places `classic-launcher` in `~/.cargo/bin/`. Ensure `~/.cargo/bin` is on your `PATH` (rustup adds this automatically when you source the env file).

## Node.js and pnpm (GUI only)

Required only to build or run the `classic-launcher` GUI. Skip this section if you only need the CLI.

Install Node.js (LTS) via NodeSource:

```bash
$ curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
$ sudo apt install nodejs
```

Install pnpm:

```bash
$ npm install -g pnpm
```

Install Tauri's Linux system libraries (Debian 12 / Ubuntu 22.04 or later):

```bash
$ sudo apt install pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
```

> **Note:** Tauri 2 requires `libwebkit2gtk-4.1-dev` (not `4.0`). Only Debian 12 (Bookworm) and Ubuntu 22.04+ ship the `4.1` version.

Once all dependencies are installed, run the GUI in development mode:

```bash
$ cd launcher && pnpm install && pnpm tauri dev
```