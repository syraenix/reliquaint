# Reliquaint v0.3 release notes

## Highlights

v0.3 makes the launcher **catalog-source-agnostic**. Instead of shipping a fixed
set of games inside the binary, Reliquaint now reads its catalog from *taps* —
versioned TOML catalog repositories you subscribe to (ADR-0003).

- **Tap subscriptions.** `reliquaint tap add/remove/list/sync/reorder/validate`
  manage the taps you read from; the GUI has an equivalent Taps panel.
- **Multiple sources with priority.** When two taps provide the same game id,
  the lower-priority tap wins; `reliquaint list --show-conflicts` enumerates the
  duplicates, and the detail view's "Make this version the default" button (or
  `reliquaint tap reorder`) lets you change which one wins. Your local user-tap
  entries always win.
- **First-run prompt.** A clean install with no subscriptions offers to
  subscribe to the official `reliquaint-core` tap.
- **`reliquaint upgrade`.** Detects install records whose tap you're not
  subscribed to and tells you exactly what to add (see migration below).
- **Quality-of-life:** `git clone` of a tap now times out after 5 minutes;
  `reliquaint tap list --check-remote` flags taps with newer commits upstream;
  `reliquaint tap reorder --interactive` edits the whole ordering in `$EDITOR`;
  game cards show a tap-of-origin badge.

## ⚠️ Breaking change: the bundled catalog has moved

The `reliquaint-core` catalog that used to ship **inside** the launcher now
lives in its own repository: <https://github.com/syraenix/reliquaint-core>. The
launcher no longer bundles any catalog content (the `tap/` directory and the
Tauri resource that shipped it are gone).

## Migrating from v0.2

Your installed games are untouched on disk. Each install record references the
`(tap, game)` it was installed against, so after upgrading the only thing
missing is the subscription that provides those catalog entries.

```bash
reliquaint upgrade            # reports installs whose tap is no longer subscribed
reliquaint tap add reliquaint-core
reliquaint list              # your games are back, sourced from the subscribed tap
```

In the GUI this surfaces as the first-run/empty-catalog prompt offering to
subscribe to `reliquaint-core`, and `⚕ Doctor` lists any orphaned installs with
the suggested `tap add` command.

The launcher v0.3.0 release and the `reliquaint-core` v0.1.0 release are
intended to land within a short window of each other so there is always
something to subscribe to.
