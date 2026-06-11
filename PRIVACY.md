# Privacy

Reliquaint is designed to be a tool you run on your own hardware to play games you legally own. It is not a service. It does not have a backend. It does not phone home.

## What Reliquaint does not do

- **No analytics or telemetry.** The launcher does not collect, aggregate, or transmit usage data of any kind. There is no opt-in flag for this because the feature does not exist.
- **No crash reporting service.** If the launcher crashes, the crash is logged locally only. You can choose to attach the log to a bug report manually; nothing is uploaded automatically.
- **No account system.** Reliquaint has no concept of users or accounts. You do not sign in.
- **No advertising.** Not now, not later.
- **No fingerprinting.** The launcher does not gather information about your hardware, OS, or installed software except as needed to run the games you ask it to run, and that information stays on your machine.

## What Reliquaint does do that involves the network

- **Opening acquisition links in your browser.** When you click "Get on GOG" or a similar button in the catalog browser, the launcher opens that URL in your default browser. The launcher itself does not contact those sites.
- **Tap synchronization.** Syncing a tap means fetching from a URL you explicitly added. The launcher contacts only those URLs, only when you ask it to.

That is the full list.

## What stays on your machine

- Your installation records (`${XDG_DATA_HOME}/reliquaint/installs/`).
- Your launcher config (`${XDG_CONFIG_HOME}/reliquaint/config.toml`).
- Your logs (`${XDG_STATE_HOME}/reliquaint/logs/`, if file logging is enabled).
- Your subscribed taps.
- The game files themselves, wherever you chose to install them.

None of this is transmitted anywhere by Reliquaint.

## A note on emulators and sidecars

The launcher orchestrates third-party software — DOSBox-Staging, FS-UAE, FluidSynth. What those programs do is governed by their own privacy practices, not Reliquaint's. To the maintainers' knowledge they do not phone home either, but their behavior is not under Reliquaint's control.

## Reporting privacy issues

If you discover the launcher transmitting data it should not, treat it as a security issue and report it via the process in [SECURITY.md](SECURITY.md).
