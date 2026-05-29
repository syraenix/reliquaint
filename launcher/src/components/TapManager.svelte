<script>
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher, onMount } from "svelte";

  const dispatch = createEventDispatcher();
  let taps = [];
  let addInput = "";
  let adding = false;
  let error = null;
  let syncing = null;

  onMount(loadTaps);

  async function loadTaps() {
    try {
      taps = await invoke("list_taps");
    } catch (e) {
      error = String(e);
    }
  }

  async function addTap() {
    if (!addInput.trim()) return;
    adding = true;
    error = null;
    try {
      await invoke("add_tap", { nameOrUrl: addInput.trim(), priority: null });
      addInput = "";
      await loadTaps();
      dispatch("changed");
    } catch (e) {
      error = String(e);
    } finally {
      adding = false;
    }
  }

  async function removeTap(id) {
    if (!confirm(`Unsubscribe from "${id}"? This will remove its cached catalog.`)) return;
    error = null;
    try {
      await invoke("remove_tap", { id });
      await loadTaps();
      dispatch("changed");
    } catch (e) {
      error = String(e);
    }
  }

  async function syncTap(id) {
    syncing = id;
    error = null;
    try {
      const messages = await invoke("sync_tap", { id });
      await loadTaps();
    } catch (e) {
      error = String(e);
    } finally {
      syncing = null;
    }
  }

  async function syncAll() {
    syncing = "__all__";
    error = null;
    try {
      await invoke("sync_tap", { id: null });
      await loadTaps();
    } catch (e) {
      error = String(e);
    } finally {
      syncing = null;
    }
  }

  function handleKeydown(e) {
    if (e.key === "Enter") addTap();
  }
</script>

<div class="tap-manager">
  <div class="tap-manager-header">
    <h2>Taps</h2>
    <button
      class="sync-all-btn"
      on:click={syncAll}
      disabled={syncing !== null}
      title="Sync all subscribed taps"
    >
      {syncing === "__all__" ? "Syncing…" : "↻ Sync all"}
    </button>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <table class="tap-table">
    <thead>
      <tr>
        <th>ID</th>
        <th>Entries</th>
        <th>Status</th>
        <th>Source</th>
        <th>Actions</th>
      </tr>
    </thead>
    <tbody>
      {#each taps as tap}
        <tr class:local={tap.is_local}>
          <td class="tap-id">{tap.id}{tap.is_local ? " ★" : ""}</td>
          <td>{tap.entry_count}</td>
          <td class:ok={tap.cache_ok} class:bad={!tap.cache_ok}>
            {tap.is_local ? "—" : tap.cache_ok ? "ok" : "missing cache"}
          </td>
          <td class="tap-source">{tap.source}</td>
          <td class="tap-actions">
            {#if !tap.is_local}
              <button
                on:click={() => syncTap(tap.id)}
                disabled={syncing !== null}
                title="Pull latest commits"
              >
                {syncing === tap.id ? "…" : "↻"}
              </button>
              <button
                class="remove-btn"
                on:click={() => removeTap(tap.id)}
                title="Unsubscribe"
              >
                ✕
              </button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  <div class="add-tap">
    <input
      bind:value={addInput}
      placeholder="Short name (e.g. reliquaint-core) or URL"
      on:keydown={handleKeydown}
      disabled={adding}
    />
    <button on:click={addTap} disabled={adding || !addInput.trim()}>
      {adding ? "Adding…" : "+ Add tap"}
    </button>
  </div>
</div>

<style>
  .tap-manager {
    padding: 20px;
    max-width: 800px;
  }

  .tap-manager-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  h2 {
    font-size: 1.1rem;
    color: var(--gold);
    margin: 0;
  }

  .error {
    color: var(--status-error-ink);
    font-size: 0.85rem;
    margin-bottom: 12px;
  }

  .tap-table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 16px;
    font-size: 0.85rem;
  }

  .tap-table th {
    text-align: left;
    padding: 6px 10px;
    color: var(--ink-muted);
    border-bottom: 1px solid var(--border);
    font-weight: 500;
  }

  .tap-table td {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    color: var(--ink-secondary);
    vertical-align: middle;
  }

  .tap-table tr.local td {
    color: var(--ink-primary);
  }

  .tap-id {
    font-weight: 500;
    color: var(--ink-primary);
  }

  .tap-source {
    font-size: 0.8rem;
    color: var(--ink-muted);
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ok {
    color: var(--status-ok-ink, #7cbb6e);
  }

  .bad {
    color: var(--status-error-ink);
  }

  .tap-actions {
    display: flex;
    gap: 6px;
  }

  .tap-actions button {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--ink-secondary);
    padding: 3px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .tap-actions button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .tap-actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .remove-btn:hover:not(:disabled) {
    color: var(--status-error-ink);
    border-color: var(--status-error-ink);
  }

  .sync-all-btn {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--gold);
    padding: 5px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .sync-all-btn:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .sync-all-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .add-tap {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .add-tap input {
    flex: 1;
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--ink-primary);
    padding: 7px 12px;
    border-radius: 6px;
    font-size: 0.85rem;
  }

  .add-tap input:focus {
    outline: none;
    border-color: var(--gold);
  }

  .add-tap input:disabled {
    opacity: 0.6;
  }

  .add-tap button {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--gold);
    padding: 7px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .add-tap button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .add-tap button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
