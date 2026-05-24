# ADR-0003: Tap-Based Catalog and Companion Content Distribution

## Status

Proposed.

## Context

Two facts about the project, taken together, force a distribution decision:

1. The catalog is intended to grow well beyond what one maintainer can curate. The pool of legitimate classic DOS and Amiga titles is in the thousands. Community contribution is on the critical path for breadth.
2. The project's preservation goals extend beyond launch metadata to companion content — walkthroughs, maps, hint files, install notes, scans of original materials. This content is bulky, varies wildly in editorial style, and has different legal considerations than catalog metadata.

A single bundled catalog living in the launcher repository handles neither well:

- Every catalog addition becomes a PR against the launcher repo, with the maintainer as the bottleneck.
- The repo accumulates content that has nothing to do with launcher code.
- Specialized curation (e.g., adventure-games-only, demoscene-focused, AGA-Amiga-only) is impossible without forking everything.
- Companion content with arguable copyright status (e.g., scanned original manuals) cannot be carried in the launcher's own repo without taking on the legal exposure.

Package managers have solved a structurally identical problem with the "tap" or "repository" pattern. Homebrew has taps; Linux distributions have third-party repositories; Nix has flakes. The pattern is mature and well-understood by the kind of user this project is built for.

## Decision

Distribute the catalog and companion content via **taps**: independently maintained sources of catalog entries and companion content that the launcher can subscribe to.

A tap is a git repository (or release tarball) with a known structure:

```
some-tap/
  tap.toml                  # tap metadata: name, maintainer, version, license, schema version
  catalog/
    dos/<id>.toml           # catalog entries
    dos/<id>.conf           # shipped DOSBox config (see ADR-0002)
    amiga/<id>.toml
  companion/
    <id>/
      install.md
      walkthrough.md
      hints/
      maps/
      ...
```

The game id is the cross-cutting key. The launcher reads catalog entries to know what is runnable; it reads `companion/<id>/` directories across all subscribed taps to know what supplementary content exists for a given game.

**The launcher ships with a default core tap** containing a small, quality-focused starter catalog and corresponding companion content. This is the v0.1 catalog and provides the baseline experience for a user who installs the launcher and adds nothing else. Whether this default tap lives inside the launcher repo or as a separate sibling repo is an implementation choice; the data model is the same either way.

**Additional taps are opt-in.** Users add them via `launcher tap add <url>` (or GUI equivalent). The launcher tracks the list of subscribed taps and the last-synced state of each. `launcher tap sync` updates them all.

**Multiple taps may contain content for the same game.** Two taps both shipping a `qfg1-ega` catalog entry is a conflict resolved by tap priority (subscription order, with explicit override available). Two taps both shipping companion content for the same game is not a conflict — both render, with per-tap attribution shown to the user.

**Tap contents are operated by tap maintainers.** The launcher project takes no editorial or legal position on what an arbitrary tap contains. The launcher renders what it is given. This is the browser/website model: the engine is neutral, the content is the maintainer's responsibility.

## Consequences

**Positive**

- The launcher repo stays small, code-focused, and reviewable.
- Catalog and companion content scale with community capacity, not maintainer capacity.
- Specialized curation is possible without forking the launcher.
- Users self-select into the content they want. A user uninterested in scanned manuals does not download them.
- Legal exposure for content with edge-case copyright status lives with the tap operator, not the launcher project.
- The model is familiar to the technical audience this project is aimed at.

**Negative**

- Adds a tap-management surface area to the launcher (CLI commands, GUI screens, sync logic, version pinning).
- "How do users find good taps?" is a new discovery problem. Solvable (curated list, in-launcher registry, docs), but real.
- Companion content rendering requires sandboxing untrusted Markdown — see open question in PRD.
- Tap protocol versioning needs thought up front to avoid painful migrations later.

## Alternatives considered

**Monorepo: all catalog and companion content in the launcher repo.** Simplest. Rejected as the structural mismatch becomes worse the more the project succeeds — a successful catalog drowns the launcher code in PRs about game metadata.

**Per-game CDN with manifest references.** Catalog entries carry URLs to companion content hosted elsewhere. Lightweight repo, content fetched on demand. Rejected because it requires centralized hosting infrastructure, complicates offline use, and trades the tap model's clarity (one source of trust per tap) for a per-URL trust problem.

**Federated/distributed (IPFS or similar).** Considered briefly. Right answer in principle for a preservation project. Rejected for v0.1 as premature; can be reintroduced as an alternative tap fetch transport later without changing the data model.

**Single bundled catalog forever.** Ship a curated set as v1, accept that catalog growth tracks maintainer capacity. Rejected because it caps the project's preservation reach to one person's time.

## Open follow-ups

- Tap protocol versioning scheme (`schema_version` in `tap.toml`?) — needs to be defined before v0.3 ships, because by then external taps may exist.
- Whether the default tap should live in the launcher repo (simpler v0.1 story, requires extraction later) or as a separate repo from the start (more setup work up front, cleaner conceptual story).
- Conflict resolution UI for the multi-tap case — exposed as priority list in GUI, in-CLI, or both?
- Markdown rendering / sandbox policy. Likely a follow-on ADR.
