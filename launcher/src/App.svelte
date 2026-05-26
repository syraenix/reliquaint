<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import iconSrc from "./assets/icon.png";
  import FilterBar from "./components/FilterBar.svelte";
  import GameGrid from "./components/GameGrid.svelte";
  import GameDetail from "./components/GameDetail.svelte";
  import DoctorPanel from "./components/DoctorPanel.svelte";

  let catalog = [];
  let filter = "all";
  let selectedId = null;
  let loading = true;
  let error = null;
  let doctorOpen = false;

  function toggleDoctor() {
    doctorOpen = !doctorOpen;
  }

  async function loadCatalog() {
    loading = true;
    error = null;
    try {
      catalog = await invoke("list_catalog");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadCatalog();
  });

  $: filtered = catalog.filter((g) => filter === "all" || g.platform === filter);
  $: selectedGame = selectedId
    ? catalog.find((g) => g.id === selectedId) ?? null
    : null;
</script>

<div class="app">
  <header>
    <div class="app-title">
      <img src={iconSrc} alt="" aria-hidden="true" class="app-icon" />
      <h1>Reliquaint</h1>
    </div>
    <div class="header-actions">
      <FilterBar bind:filter />
      <button class="header-btn" on:click={loadCatalog} title="Refresh catalog">
        ↻
      </button>
      <button class="header-btn" on:click={toggleDoctor}>
        {doctorOpen ? "✕ Doctor" : "⚕ Doctor"}
      </button>
    </div>
  </header>

  {#if doctorOpen}
    <DoctorPanel />
  {:else if selectedGame}
    <GameDetail
      game={selectedGame}
      on:back={() => (selectedId = null)}
      on:installed={loadCatalog}
    />
  {:else if loading}
    <div class="status">Loading catalog…</div>
  {:else if error}
    <div class="status error">Error loading catalog: {error}</div>
  {:else if catalog.length === 0}
    <div class="status">
      Catalog is empty.
      <br />
      <small>The bundled tap may not be present at this repo root.</small>
    </div>
  {:else}
    <GameGrid games={filtered} on:select={(e) => (selectedId = e.detail.id)} />
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--bg-base);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    background: var(--bg-deep);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .app-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .app-icon {
    width: 24px;
    height: 24px;
  }

  h1 {
    font-size: 1.2rem;
    font-weight: 600;
    color: var(--gold);
    letter-spacing: 0.03em;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .header-btn {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--gold);
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .header-btn:hover {
    background: var(--bg-hover);
  }

  .status {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--ink-secondary);
    font-size: 1rem;
    text-align: center;
    padding: 20px;
  }

  .status small {
    color: var(--ink-muted);
    font-size: 0.85rem;
    margin-top: 8px;
  }

  .error {
    color: var(--status-error-ink);
  }
</style>
