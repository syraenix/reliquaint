# Reliquaint — Product Requirements Document

> **Status:** v0.1 — feature-complete foundation, shipping.
> **Name:** *Reliquaint* — a portmanteau of *relic* and *quaint*, pronounced *REL-i-kwaynt*.

## Vision

Reliquaint is a preservation hub for classic DOS and Amiga games that happens to launch. It exists to preserve not just the binaries — that is emulator territory — but the cultural context around the games: install knowledge, configuration recipes, walkthroughs, maps, hint files, box art, manuals. The launching part is the entry point; preservation is the goal.

The name reflects the temperature of the project: affectionate and a little homely, treating these games as quaint relics worth tending to rather than artifacts to be analyzed from a clinical distance.

The product treats old games as relics worth keeping whole, and the act of running a 30-year-old game on modern hardware as something that should be possible without either deep technical knowledge or surrendering to a closed ecosystem.

## Problem

A user with a legally owned copy of a 1989 Sierra adventure or a 1991 Amiga platformer faces three friction points to actually playing it:

1. **Configuration.** Getting DOSBox-Staging or FS-UAE tuned for a specific game requires per-game knowledge: cycles, scalers, MIDI routing, model selection, mount setup. Each game has its own quirks.
2. **Context.** The original manuals, maps, and hint sheets that shipped with these games are scattered across abandonware sites, fan wikis, and Internet Archive. They are often essential for actually finishing the game and increasingly hard to find.
3. **Discovery.** "What else from this era is worth my time?" has no good answer without trusting a closed-platform curation feed.

Existing tools solve fragments: Lutris and similar launchers solve part of the launching problem; archive.org solves part of the preservation problem; fan wikis solve part of the context problem. Nothing brings them together with respect for both open-source values and the legal rights of original creators.

## Goals

- **Make launching trivial** for any game with a catalog entry. One command (or one click), correct emulator, correct config, correct mounts.
- **Make companion content first-class.** Walkthroughs, maps, install notes, hint files belong alongside the launch button, not on a separate website.
- **Lower the contribution bar.** Adding a game, fixing a config, contributing a walkthrough should require minimal launcher-internal knowledge.
- **Support community curation.** No single team should bottleneck the catalog. The architecture should encourage independent maintainers.
- **Stay legal, clearly.** The launcher takes no position on game file acquisition beyond pointing at legitimate sources. It does not host, fetch, or distribute game binaries.
- **Be inspectable.** Configurations, manifests, and content should be plain-text and editable without specialized tools.

## Non-goals

- **Acquiring game files for the user.** The launcher will not fetch, scrape, torrent, or otherwise obtain game binaries. The user provides their own files.
- **DRM circumvention.** Not a goal, not a feature.
- **Cross-platform abundance.** Linux-first. Other platforms may follow; they are not on the critical path.
- **Replacing emulators.** DOSBox-Staging and FS-UAE are the runtime; the launcher orchestrates, it does not emulate.
- **Curating the universe.** The default catalog is intentionally small and quality-focused. Breadth comes from community taps, not from the core project.
- **Modern game support.** The product is for classic-era DOS and Amiga titles. Scope expansion to other platforms (ScummVM, console emulators) is a future conversation, not a v1 concern.

## Users

- **The collector.** Has a legally acquired collection of DOS and Amiga games. Wants a clean way to launch them, possibly with notes and maps available. Tinkers with configs willingly but appreciates sensible defaults.
- **The returning player.** Played these games as a teenager, wants to revisit. Has lower tolerance for setup friction. Wants "install, click, play, with a walkthrough handy when stuck."
- **The contributor.** Cares about preservation, wants to add a game manifest or a walkthrough or fix a config. Comfortable with git and Markdown. Not necessarily a Rust developer.
- **The curator.** Wants to maintain a themed catalog (e.g., adventure games, Amiga AGA-only, demoscene) as a tap, with full editorial control.

## Functional areas

- **Launcher core.** Rust CLI + Tauri GUI. Reads manifests, generates emulator configs, launches games.
- **Catalog.** Per-game manifests describing identity, runtime requirements, recommended configuration, and legal acquisition links. Splits into shipped catalog entries (immutable, location-agnostic) and per-user installation records (where the user's copy lives). See ADR-0001.
- **Configuration.** Per-game emulator configs split into shipped tuning and runtime-generated mount/autoexec sections. See ADR-0002.
- **Tap subscription.** A tap is a versioned, fetchable source of catalog entries and companion content. The launcher ships with a default core tap and supports adding others. See ADR-0003.
- **Companion content.** Per-game supplementary content (walkthroughs, maps, hints, install notes, manuals, box art) carried by taps and surfaced in the GUI alongside the launch button.
- **Doctor.** Diagnostics for host dependencies, install paths, and configuration sanity (already exists as a `doctor` subcommand).
- **Install workflow.** A flow for taking a user's game files and a catalog entry and producing a working installation record. Also: a flow for creating a new manifest for a game the catalog does not know about, with optional upstream submission.

## Scope phases

These are sequencing guidance, not commitments. Each phase ends in a state where the launcher is usable for the personas served by that phase.

**v0.1 — Public-ready foundation.** Two-layer manifest model. Split DOSBox config model. Existing collections (Quest for Glory, King's Quest, Amiga starter) refactored into the new model. CLI + GUI both work. Install flow exists for known games. Documentation sufficient for an outside user to install and run.

**v0.2 — Manifest creation flow.** "I have a game folder, make a manifest for it" wizard. Scans for likely entry points, drafts a manifest, drops user into editor for review. Foundation for community contribution.

**v0.3 — Tap subscription.** Multi-source catalog. `launcher tap add/remove/sync`. Default tap split out from launcher repo. Catalog browsing in GUI.

**v0.4 — Companion content.** Per-game walkthroughs, maps, hints rendered in GUI. Markdown sandbox. Multi-tap content attribution.

**Beyond.** Submission tooling for catalog/companion contributions. Cross-tap content de-duplication. Possibly: ScummVM backend, MS-DOS hard-drive image support, additional platforms.

## Resolved

- **License.** Code is [MPL-2.0](../LICENSE); catalog content is [CC-BY-SA-4.0](../LICENSE-CONTENT).
- **Default tap contents.** The bundled tap is `reliquaint-core` — a small, quality-focused starter set (the Quest for Glory, King's Quest, and Amiga starter entries).

## Open questions

- **Tap discovery.** How do users find good taps? Possibilities: maintained `awesome-reliquaint-taps` repo, in-launcher search backed by a small registry, or just documentation pointers.
- **Companion content rendering security.** Tauri webview + arbitrary Markdown from third-party taps. Decision needed on Markdown renderer, sanitization policy, and remote resource loading policy.
