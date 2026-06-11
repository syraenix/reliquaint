# Security Policy

## Reporting a vulnerability

If you believe you've found a security vulnerability in Reliquaint, please **do not open a public GitHub issue**. Public disclosure before a fix is in place puts users at risk.

Instead, report privately through **GitHub Private Vulnerability Reporting**: use the "Report a vulnerability" button in the [Security tab](https://github.com/syraenix/reliquaint/security/advisories/new) of this repository.

We aim to acknowledge receipt within 7 days, and to communicate a fix timeline once the report is assessed. The size of this project means we can't promise enterprise-grade response windows, but security reports are taken seriously and worked on in good faith.

## Scope

**In scope:**

- The Reliquaint launcher binaries (CLI and GUI), including their handling of TOML manifests, filesystem paths, process spawning, and tap content.
- The bundled `reliquaint-core` tap content shipped with the launcher.
- The launcher's IPC surface (Tauri commands).

**Out of scope:**

- Vulnerabilities in upstream emulators (DOSBox-Staging, FS-UAE) or sidecars (FluidSynth). Report those to their respective maintainers.
- Issues that require an attacker to already have local access to your filesystem and config directories.
- Issues in third-party taps. Tap content is the responsibility of the tap maintainer; Reliquaint renders what it is given and takes no editorial position on tap contents. If a tap is hosting actively malicious content, please report it to us anyway — we'd like to know.

## Disclosure

We'll coordinate disclosure with the reporter. The default flow:

1. Acknowledge the report.
2. Confirm or close the issue.
3. Develop and test a fix.
4. Release a patched version.
5. Publish a security advisory crediting the reporter (unless anonymity is requested).

For severe issues, this timeline may be shortened. For low-severity findings, fixes may be rolled into normal releases without a dedicated advisory.

## Supported versions

Reliquaint is pre-1.0. Security fixes go into the latest release. Older releases are unsupported.

| Version | Supported |
| --- | --- |
| 0.x latest | Yes |
| Older | No |
