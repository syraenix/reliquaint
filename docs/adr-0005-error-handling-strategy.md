# ADR-0005: Error Handling Strategy

## Status

Proposed.

## Context

Reliquaint is a Rust workspace where the same backend library is consumed by two binaries — a CLI and the Tauri-hosted GUI. The library does TOML parsing, filesystem walking, process spawning, and config composition, all of which can fail in distinct ways that matter to the user.

Without a coherent error strategy, the codebase will accrete inconsistent error types: some functions returning `String`, some `Box<dyn Error>`, some custom enums per file. Error chains will break or duplicate. Surfacing errors to the user — in CLI stderr output, in the GUI diagnostic panel, in log lines — becomes ad-hoc.

This is the same shape of decision as ADR-0004 (logging): cross-cutting, cheap to decide now, expensive to retrofit. The library/binary structure also gives the decision a natural seam.

## Decision

Use a two-layer error strategy split along the library/binary boundary.

### Library code: `thiserror`

Parsers, launcher logic, install flow, catalog assembly — anything inside the `reliquaint` library — uses `thiserror`-derived enum errors.

- One error enum per module that owns a distinct failure domain (`CatalogError`, `InstallError`, `LaunchError`, `TapError`, `ConfigError`).
- Each enum variant carries structured context as fields (file path, game id, expected vs. found values), not free-form strings interpolated into message templates.
- Foreign errors (`io::Error`, `toml::de::Error`, etc.) wrap as enum variants with `#[source]` for chain preservation.
- A top-level `ReliquaintError` enum unifies module errors via `#[from]` at points where the library exposes a single surface (e.g., a high-level `launch_game()` facade).

Library code never exposes `anyhow::Error` in its public API. `anyhow` lives in the binaries.

### Binary code: `anyhow`

The CLI `main`, the Tauri command handlers, and any other binary-layer code use `anyhow::Result`.

- Receives library errors and adds binary-layer context via `.context()` where the call site has information the library does not.
- The CLI entry point formats the full error chain to stderr.
- The Tauri command layer translates library errors into structured payloads the frontend can render (severity, summary, detail, suggested action).

### Display, Debug, and the chain

Every library error type:

- Implements `Display` via `thiserror`'s `#[error("…")]` attribute, with a clear user-readable message that includes key context (which file, which game, which field).
- Has a useful `Debug` representation (the derive is fine).
- Preserves the `source()` chain so `anyhow`'s chain printing produces meaningful output through the boundary.

### Relationship to logging

Library code does **not** log errors on the way up. It returns them. The binary layer is the single place that decides "this error → log at `error!`, format for stderr, dispatch to GUI." This prevents the double-logging anti-pattern where the same error appears multiple times in different contexts.

### Panic policy

- Library code never panics on user data, malformed input, or recoverable conditions. It returns errors.
- `unwrap()` and `expect()` are reserved for invariants statically guaranteed (e.g., accessing capture groups after a confirmed regex match).
- Where `expect()` is used, the message describes the invariant being asserted, not the operation: `expect("schema_version present after validation")` rather than `expect("get failed")`.
- A custom panic hook installed at both binary entry points prints a brief "please file a bug" message and an instruction for capturing trace-level logs before re-panicking. For the rare case where a panic does escape.

### Validation aggregation

Where multiple validation failures can be detected in a single pass — loading all catalog entries from a tap, running `doctor` checks, validating an installation record's `expects_files` — aggregate errors rather than failing on the first. The user benefits from seeing the full picture in one run.

## Consequences

**Positive**

- Consistent error handling across the codebase. Every parser, walker, and launcher feels like part of the same project.
- Library callers can `match` on specific error variants where useful. The Tauri command layer can render "catalog entry not found" differently from "emulator binary missing" without string-matching error messages.
- Error messages include real context (paths, game ids, expected values) rather than generic strings.
- The library / binary seam matches the natural ownership of context: the library knows what failed and why; the binary knows how to present it.
- Test assertions can match specific variants rather than substring-matching error messages.

**Negative**

- Per-module error enums require more upfront design than throwing everything in one big enum. Mitigated by the modules already being natural failure domains, and by errors being additive — new variants don't break callers that match exhaustively, since `#[non_exhaustive]` is part of the recipe.
- `thiserror`'s derive syntax is slightly less obvious than a hand-written `impl Error`. Trade-off worth making for the boilerplate it removes.

## Alternatives considered

**`anyhow` everywhere.** Simpler. Rejected because the GUI needs to discriminate error categories programmatically (different UI treatment for "game not found" vs. "emulator crashed"), and `anyhow`'s dynamic typing makes this awkward.

**`thiserror` everywhere, no `anyhow`.** Pure but verbose at binary boundaries where every `?` requires explicit conversion. Rejected; `anyhow` exists for exactly the binary-layer use case where the only thing the code does with an error is propagate, log, and exit.

**`eyre` / `color-eyre` instead of `anyhow`.** Nicer terminal output, especially for the CLI. Worth considering as a follow-up; not adopted up-front because `anyhow` is more conventional and the marginal value is small for v0.1. Migration is mechanical.

**`miette` for rich parser diagnostics.** Tempting for catalog TOML errors with source spans. Rejected for v0.1 as over-engineering; the file path plus the line/column from `toml::de::Error` is sufficient. May revisit if catalog parsing errors become a community-support burden.

**`Box<dyn Error>` returns.** Works but loses type information and complicates the chain. Rejected as the worst of both worlds.

## Open follow-ups

- Whether to adopt `eyre` for nicer CLI error reporting after v0.1 ships.
- A standardized way for the Tauri command layer to translate library errors into frontend-friendly payloads (severity, summary, detail, suggested action). Likely a small `ToUserError` trait or similar; design deferred to GUI implementation in Milestone 5.
