# ADR-0004: Logging Strategy

## Status

Proposed.

## Context

Reliquaint is a Rust CLI + Tauri GUI sharing a single backend crate. Both surfaces need visibility into what the launcher is doing — for development debugging, for diagnosing user-reported issues, and for the user's own understanding when something goes wrong trying to run a 30-year-old game on modern hardware.

The launcher's operations span multiple processes (emulator, sidecars), multiple async boundaries (Tauri IPC, process supervision), and multiple subsystems (catalog loading, config composition, install validation). Scattered `println!` calls will not scale; nor will scattered usage of `log` without a coherent subscriber strategy.

Logging is one of those decisions that is cheap to make early and expensive to retrofit. Choosing the framework before code accretes also gives Claude Code clear guidance for instrumentation as it implements v0.1 tasks.

## Decision

Use the `tracing` ecosystem as the logging framework.

- **`tracing`** as the instrumentation facade in library code.
- **`tracing-subscriber`** for formatting, filtering, and routing.
- A custom subscriber layer for the Tauri GUI to receive events and surface them in the diagnostic panel.
- Standard `EnvFilter` for runtime verbosity control via `RUST_LOG`.

Instrument key operations with spans, not just events. The launcher's value as a debugging tool depends on being able to follow a single "launch game X" operation through config composition, sidecar startup, emulator invocation, and shutdown.

### Log levels

Used consistently across the codebase:

- **`ERROR`** — Operations the user cannot proceed past: failed launch, missing required binary, unparseable catalog entry, corrupt installation record. Always visible regardless of verbosity setting.
- **`WARN`** — Things the user should know about that do not stop work: orphaned installation records, missing optional sidecar, unrecognized fields in a catalog entry, missing optional metadata.
- **`INFO`** — High-level operations the user might want to see: "launching qfg1-ega", "loaded N catalog entries from reliquaint-core", "installation record created". The default verbosity for the CLI.
- **`DEBUG`** — Detailed step information useful for diagnosis: config composition steps, individual file existence checks, process spawn arguments. Enabled with `-v` or `RUST_LOG=reliquaint=debug`.
- **`TRACE`** — Very fine-grained, on by request only: every TOML field, every path resolution.

### Spans

At minimum, wrap these operations in spans:

- `launch_game(id)` — covers the entire launch lifecycle from CLI/GUI command to emulator exit.
- `sidecar.<name>` — child span under `launch_game`, covering the sidecar's lifetime.
- `load_tap(id)` — covers tap discovery and catalog parsing.
- `install_game(id)` — covers install record creation.
- `doctor.<check>` — one span per `doctor` check.

Spans carry the game id, tap id, or other relevant context as structured fields so log output can be filtered and correlated.

### Output destinations

- **CLI default**: human-formatted output to stderr at `INFO`. `-v` raises to `DEBUG`; `-vv` to `TRACE`. `RUST_LOG` overrides if set.
- **GUI default**: events captured by a custom subscriber layer and forwarded to the Svelte frontend via Tauri events for display in the diagnostic panel.
- **Log file** (opt-in for v0.1): `${XDG_STATE_HOME:-$HOME/.local/state}/reliquaint/logs/reliquaint.log` when enabled via user config. Plain text, one event per line, ISO 8601 timestamps. Rotation deferred; v0.1 can truncate on startup or simply append.

### Configuration in user launcher config

```toml
[logging]
level        = "info"   # one of: error, warn, info, debug, trace
file_enabled = false    # if true, also write to log file
```

`RUST_LOG` overrides this for the session. CLI `-v` flags override both.

## Consequences

**Positive**

- Single instrumentation API across CLI and GUI.
- Structured fields make diagnostic reports — and user-submitted bug reports — significantly more useful. A user can paste a `-v` run rather than describing what happened.
- The GUI's diagnostic panel is a near-free feature once the subscriber layer exists.
- Span-based correlation makes async and multi-process operations actually debuggable.
- Idiomatic for modern Rust; contributors who know the ecosystem recognize it immediately.

**Negative**

- One more dependency tree to carry. Acceptable; `tracing` is widely-used and well-maintained.
- A small upfront cost setting up the subscriber stack and the Tauri-routing layer. One-time.
- Risk of over-instrumentation if every function gets a span. Mitigated by guidance: spans for user-meaningful operations, events for state changes within them.

## Alternatives considered

**`log` + `env_logger`.** Simpler. Rejected because it lacks structured fields and spans, both of which materially help when debugging a multi-process launcher. The complexity delta from `log` to `tracing` is small and pays for itself by the second async operation.

**`println!` / `eprintln!` directly.** Rejected as the default approach. Has its place in throwaway prototypes; not suitable for a tool that will accept user bug reports.

**Custom logging.** Rejected on principle. The Rust ecosystem solved this problem; there is no project-specific reason to reinvent.

## Open follow-ups

- Should bug reports include an automatic log capture from the last launch? Useful but raises privacy questions (log entries may contain user paths). Defer to post-v0.1.
- Log rotation strategy if file logging sees broad use.
- "Copy diagnostic output to clipboard" button on GUI errors. Likely yes; folded into Milestone 5's diagnostic panel work.
