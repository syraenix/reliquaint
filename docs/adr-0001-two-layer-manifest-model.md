# ADR-0001: Two-Layer Manifest Model

## Status

Proposed.

## Context

The current manifest format combines two kinds of information that have different lifecycles and audiences:

1. **What the game is and how to run it.** Identity (id, title, platform), runtime requirements (emulator, model, sidecars), and recommended configuration. This data is identical across every user who has the game.
2. **Where the user's copy lives.** Filesystem paths like `expects_dir = "~/games/qfg1-ega"` and relative config references like `config = "../config/qfg1-ega.conf"`. This data is specific to one user on one machine.

Today both live in a single TOML file in the repository. This works for a single-user docs-and-scripts project, but it breaks down as soon as the manifest needs to be shippable to other people:

- The repo's directory structure leaks into the manifest (relative `config` paths).
- The user's filesystem leaks into the manifest (`expects_dir`).
- Manifests cannot be meaningfully shared between users without modification.
- There is no place to record "I have this game installed, and it is at this path" separate from the catalog identity of the game.

The catalog is intended to grow into a discovery surface — including for games the user does not own — which makes the conflation more acute. A catalog entry for a game the user has not installed should not carry an `expects_dir`.

## Decision

Split the current manifest into two distinct concepts with distinct storage:

**Catalog entry** (`<tap>/catalog/<platform>/<id>.toml`)
- Immutable as far as the launcher is concerned. Lives in a tap (initially the repo's bundled tap).
- Carries identity, runtime requirements, recommended config reference, metadata for discovery, and legal acquisition links.
- Carries no user paths, no filesystem assumptions, no host-specific data.
- Versioned in the tap's git history.

**Installation record** (`${XDG_DATA_HOME}/reliquaint/installs/<id>.toml` or equivalent)
- Per-user, mutable. Written by the install flow, read at launch time.
- Carries a reference to the catalog entry id, the game's install path on this machine, and any user-specific overrides (e.g., per-machine config tweaks, user-chosen MIDI routing).
- Never appears in any repository.

The launcher loads catalog entries and installation records as separate datasets and joins them by id at runtime. A catalog entry without a matching installation record renders as "not installed." An installation record without a catalog entry (legacy or user-created) renders with whatever metadata is available locally.

## Consequences

**Positive**

- Catalog entries become shippable. The same file works on every machine.
- Community contribution becomes a clean PR flow: add a TOML file under `catalog/<platform>/`.
- Discovery UI can show games the user does not have without needing fake or null install paths.
- Schema evolution is per-layer. Adding a discovery field to catalog entries does not touch any installation record.
- The launcher's data model maps cleanly onto the tap model in ADR-0003.

**Negative**

- Two files to manage where there was one. Mitigated by the fact that users only ever touch one file directly (catalog entries via PR, installation records via the install command).
- Migration cost for the existing manifests. One-time, scriptable.
- The launcher needs slightly more logic at startup to assemble the joined view.

## Alternatives considered

**Single-file manifest, all paths user-relative.** Keeps the current shape but rewrites all paths relative to a `$LAUNCHER_HOME` variable. Does not solve the discovery problem (catalog entries for games the user does not own would still need null/placeholder paths). Rejected.

**Embedded SQLite database holding both catalog and installation data.** Considered briefly. Rejected because (a) the data volumes never justify a real database, (b) community contribution via PR is significantly harder on a binary file, (c) schema evolution requires migrations rather than additive TOML field changes, and (d) inspectability and debuggability suffer.

**Catalog as TOML, installation records as SQLite.** Hybrid model. Considered. Rejected for now in favor of TOML on both sides for consistency. SQLite may return later as a launch-time cache/index over the TOML files if startup scan time becomes a real problem; it would not be the source of truth.

## Open follow-ups

- The exact schema for the catalog entry (which discovery fields are required vs. optional) is left for a separate schema design document or follow-on ADR.
- The exact shape of "user-specific overrides" in installation records is deferred; v0.1 may ship with overrides limited to install path only.
