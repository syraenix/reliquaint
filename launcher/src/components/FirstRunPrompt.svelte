<script>
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher } from "svelte";

  const dispatch = createEventDispatcher();
  const CORE_URL = "https://github.com/syraenix/reliquaint-core";
  let adding = false;
  let error = null;

  async function addCoreTap() {
    adding = true;
    error = null;
    try {
      await invoke("add_tap", { nameOrUrl: "reliquaint-core", priority: null });
      dispatch("subscribed");
    } catch (e) {
      error = String(e);
    } finally {
      adding = false;
    }
  }
</script>

<div class="first-run">
  <div class="first-run-inner">
    <h2>Welcome to Reliquaint.</h2>
    <p>
      To get started, subscribe to a tap — a curated catalog of games. The
      official Reliquaint Core tap covers a starter set of classic DOS and Amiga
      titles.
    </p>
    <p class="url-hint">Will fetch: <code>{CORE_URL}</code></p>
    {#if error}
      <p class="error">{error}</p>
    {/if}
    <div class="actions">
      <button class="primary" on:click={addCoreTap} disabled={adding}>
        {adding ? "Adding…" : "Add Reliquaint Core"}
      </button>
      <button class="secondary" on:click={() => dispatch("dismiss")}>
        I'll add a tap later
      </button>
    </div>
  </div>
</div>

<style>
  .first-run {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px 20px;
    background: var(--bg-base);
  }

  .first-run-inner {
    max-width: 480px;
    text-align: center;
  }

  h2 {
    font-size: 1.4rem;
    color: var(--gold);
    margin-bottom: 16px;
  }

  p {
    color: var(--ink-secondary);
    line-height: 1.6;
    margin-bottom: 12px;
  }

  .url-hint {
    font-size: 0.8rem;
    color: var(--ink-muted);
  }

  .url-hint code {
    color: var(--ink-secondary);
  }

  .error {
    color: var(--status-error-ink);
    font-size: 0.85rem;
  }

  .actions {
    display: flex;
    gap: 12px;
    justify-content: center;
    margin-top: 24px;
  }

  button {
    padding: 8px 20px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
    border: 1px solid var(--border-light);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .primary {
    background: var(--gold);
    color: var(--bg-deep);
    font-weight: 600;
    border-color: var(--gold);
  }

  .primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .secondary {
    background: var(--bg-elevated);
    color: var(--ink-secondary);
  }

  .secondary:hover {
    background: var(--bg-hover);
  }
</style>
