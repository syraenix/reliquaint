<script>
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  let results = [];
  let loading = true;
  let error = null;
  let listenerError = null;

  // kind → { installing, expanded, log: [{line, stream}], lastExit, rowError }
  let rowState = {};

  const FIXABLE = new Set([
    "dosbox-flatpak",
    "fluidsynth",
    "soundfont",
    "innoextract",
    "fs-uae",
    "unzip",
  ]);

  const INFO_HINTS = {
    "game-install-dir":
      "Acquire the game per the collection guide and extract it to the path shown.",
  };

  let unlistenOutput = null;
  let unlistenFinished = null;

  async function refresh() {
    loading = true;
    error = null;
    try {
      results = await invoke("run_doctor");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function ensureRowState(kind) {
    if (!rowState[kind]) {
      rowState[kind] = {
        installing: false,
        expanded: false,
        log: [],
        lastExit: null,
        rowError: null,
      };
    }
    return rowState[kind];
  }

  function isFixable(kind) {
    return FIXABLE.has(kind);
  }

  function infoHint(kind) {
    if (kind.startsWith("game-install-dir:")) return INFO_HINTS["game-install-dir"];
    return null;
  }

  async function fix(kind) {
    const row = ensureRowState(kind);
    row.installing = true;
    row.expanded = true;
    row.log = [];
    row.lastExit = null;
    row.rowError = null;
    rowState = { ...rowState };
    try {
      const exit = await invoke("install_dependency", { kind });
      row.lastExit = exit;
    } catch (e) {
      row.rowError = String(e);
    } finally {
      row.installing = false;
      rowState = { ...rowState };
      await refresh();
    }
  }

  function toggleExpanded(kind) {
    const row = ensureRowState(kind);
    row.expanded = !row.expanded;
    rowState = { ...rowState };
  }

  onMount(async () => {
    await refresh();
    try {
      unlistenOutput = await listen("install-output", (e) => {
        const { kind, line, stream } = e.payload;
        const row = ensureRowState(kind);
        row.log = [...row.log, { line, stream }];
        rowState = { ...rowState };
      });
      unlistenFinished = await listen("install-finished", (e) => {
        const { kind, exit_code } = e.payload;
        const row = ensureRowState(kind);
        row.lastExit = exit_code;
        rowState = { ...rowState };
      });
    } catch (e) {
      listenerError = String(e);
    }
  });

  onDestroy(() => {
    if (unlistenOutput) unlistenOutput();
    if (unlistenFinished) unlistenFinished();
  });
</script>

<div class="doctor">
  <h2>Setup</h2>

  {#if listenerError}
    <p class="status warn">Live install output unavailable: {listenerError}</p>
  {/if}

  {#if loading}
    <p class="status">Checking…</p>
  {:else if error}
    <p class="status error">{error}</p>
  {:else}
    <ul>
      {#each results as r}
        {@const row = rowState[r.kind] || { installing: false, expanded: false, log: [], lastExit: null, rowError: null }}
        {@const hint = infoHint(r.kind)}
        {@const fixable = isFixable(r.kind) && r.status === "missing"}
        <li class="probe status-{r.status}">
          <div class="row">
            <span class="dot"></span>
            <div class="text">
              <div class="name">{r.name}</div>
              {#if r.detail}
                <div class="detail">{r.detail}</div>
              {/if}
              {#if r.status === "missing" && hint && !fixable}
                <div class="hint">{hint}</div>
              {/if}
              {#if row.lastExit !== null && row.lastExit !== 0}
                <div class="hint error">Install exited with code {row.lastExit}.</div>
              {/if}
              {#if row.rowError}
                <div class="hint error">{row.rowError}</div>
              {/if}
            </div>
            <div class="actions">
              {#if fixable}
                <button
                  class="fix-btn"
                  disabled={row.installing}
                  on:click={() => fix(r.kind)}
                >
                  {row.installing ? "Installing…" : "Fix this"}
                </button>
              {/if}
              {#if row.log.length > 0 || row.installing}
                <button class="toggle-btn" on:click={() => toggleExpanded(r.kind)}>
                  {row.expanded ? "▾" : "▸"}
                </button>
              {/if}
            </div>
          </div>
          {#if row.expanded && (row.log.length > 0 || row.installing)}
            <pre class="log">
{#each row.log as entry}<span class="log-line {entry.stream}">{entry.line}
</span>{/each}{#if row.installing && row.log.length === 0}<span class="log-line stdout">starting…
</span>{/if}</pre>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .doctor {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    max-width: 800px;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--gold-ink);
    margin-bottom: 16px;
  }

  ul {
    list-style: none;
  }

  .probe {
    padding: 8px 0;
    border-bottom: 1px solid var(--border-on-light);
    font-size: 0.88rem;
  }

  .row {
    display: grid;
    grid-template-columns: 14px 1fr auto;
    column-gap: 10px;
    align-items: start;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    margin: 6px auto 0;
  }

  .status-ok .dot { background: var(--status-installed); }
  .status-missing .dot { background: var(--status-error); }
  .status-unknown .dot { background: var(--ink-muted); }

  .text { min-width: 0; }
  .name { color: var(--ink-primary); }

  .detail {
    color: var(--ink-muted);
    font-size: 0.8rem;
    font-family: monospace;
    margin-top: 2px;
    word-break: break-all;
  }

  .hint {
    color: var(--ink-secondary);
    font-size: 0.78rem;
    margin-top: 4px;
  }

  .hint.error { color: var(--status-error-ink); }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .fix-btn {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--text-primary);
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .fix-btn:hover:not(:disabled) { background: var(--bg-hover); }
  .fix-btn:disabled { opacity: 0.6; cursor: default; }

  .toggle-btn {
    background: transparent;
    border: none;
    color: var(--ink-secondary);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 4px 6px;
  }

  .log {
    margin: 8px 0 0 24px;
    padding: 8px 10px;
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 4px;
    max-height: 240px;
    overflow-y: auto;
    font-family: ui-monospace, monospace;
    font-size: 0.78rem;
    line-height: 1.35;
    white-space: pre-wrap;
    color: var(--text-secondary);
  }

  .log-line.stderr { color: var(--status-error); }

  .status { color: var(--ink-secondary); font-size: 0.9rem; }
  .error { color: var(--status-error-ink); }
  .warn { color: var(--status-warn-ink); font-size: 0.8rem; margin-bottom: 8px; }
</style>
