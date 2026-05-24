# ADR-0002: Split DOSBox Config Model

## Status

Proposed.

## Context

DOSBox-Staging `.conf` files for the current collections do two distinct jobs in a single file:

1. **Shipped tuning.** Cycles, scalers, MIDI routing (e.g., `midiconfig=128:0` for FluidSynth), sound card emulation choices. These are the per-game accumulated knowledge that makes a specific game run correctly and feel right. They are identical across machines.
2. **User-specific mounting.** The `[autoexec]` block contains literal `MOUNT C "~/games/qfg1-ega"`-style commands referencing the user's filesystem. These cannot ship as-is.

Today both live in the same `.conf` file, which means the file cannot be redistributed without modification. The current repo papers over this by assuming every user mirrors a specific directory layout (`~/games/<id>`), which is fine for a one-author docs repo but does not survive contact with multiple users.

This decision is closely tied to ADR-0001's installation record concept — the launcher needs a source of truth for the install path before it can generate user-specific autoexec sections.

## Decision

Split per-game DOSBox configuration into two parts handled separately:

**Shipped settings (`<tap>/catalog/dos/<id>.conf`)**
- Lives in the tap alongside the catalog entry.
- Contains everything that is per-game tuning: cycles, scalers, MIDI routing, sound cards, machine type.
- Contains no `[autoexec]` block. Contains no filesystem paths.

**Generated autoexec**
- Built at launch time by the launcher from data in the installation record (per ADR-0001) and the catalog entry's runtime block.
- Contains the `MOUNT` lines (driven by the installation record's install path), the working-directory change, and the game's entry-point command (driven by the catalog entry).
- Composed with the shipped settings into a final config that DOSBox-Staging receives.

The composition can happen via two reasonable mechanisms; the choice is an implementation detail to be settled when implementing:

- Write a temporary merged `.conf` to a runtime directory and pass it to DOSBox-Staging.
- Pass the shipped `.conf` plus an `-c` series of inline autoexec commands.

The launcher should prefer whichever produces clearer diagnostics when something goes wrong.

The catalog entry's runtime block grows the small amount of declarative information needed to drive autoexec generation — entry-point command, expected mount letter, anything game-specific. The shape of that block is a schema concern, not an ADR concern.

User-side overrides of shipped configs (e.g., the user wants `cycles=fixed 10000` instead of the shipped `cycles=fixed 8000` for a slower machine) layer on top via a per-user override file in the installation record's directory. Override semantics are deferred to implementation; the simple form is "user override file is appended after shipped config so its values win."

## Consequences

**Positive**

- Shipped `.conf` files become portable. Same file works for every user.
- User paths never appear in version-controlled files.
- The launcher gains a single point of responsibility for resolving "where is the game on this machine" into the actual DOSBox invocation.
- Configuration changes for a game (a better scaler, a MIDI fix) propagate via tap updates without touching user data.
- The override layer gives advanced users a clean way to tweak without forking the catalog.

**Negative**

- Per-game `.conf` files in the current repo need editing to remove their `[autoexec]` blocks. One-time migration.
- The launcher has to learn to compose configs. Modest implementation cost.
- The override layer adds a small amount of UI/UX surface (where do overrides live, how does the user create one?).

## Alternatives considered

**Templated `.conf` files with placeholders.** Ship `.conf` files with `{{game_dir}}` substitution markers. Simpler to implement than full composition. Rejected because it bakes user paths into the same file that ships, which loses the cleanliness of separation and complicates the override story.

**Fully launcher-generated `.conf`.** The manifest declares game requirements at a high level (e.g., `vga = true`, `cpu_cycles = 8000`, `midi = fluidsynth`) and the launcher generates the entire `.conf`. Most elegant in principle. Rejected as too invasive for v0.1 — the current per-game `.conf` files encode hard-won tuning, and re-expressing all of it in a higher-level schema is a significant project that should not block public release. Worth revisiting later.

**Status quo with documented assumptions.** Keep the current single-file `.conf` and document the path assumption (`~/games/<id>`). Rejected; punts the problem and constrains every user to the author's directory layout.

## Open follow-ups

- FS-UAE config files have an analogous problem (model templates plus per-game disk mounts). The same split applies but the mechanics differ. A follow-on ADR or schema document should specify the Amiga side.
