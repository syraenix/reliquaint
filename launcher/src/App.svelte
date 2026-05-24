<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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
    <h1>Reliquaint</h1>
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
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    background: #1a1a2e;
    border-bottom: 1px solid #2a2a40;
    flex-shrink: 0;
  }

  h1 {
    font-size: 1.2rem;
    font-weight: 600;
    color: #a0a8ff;
    letter-spacing: 0.03em;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .header-btn {
    background: #252538;
    border: 1px solid #3a3a55;
    color: #a0a8ff;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .header-btn:hover {
    background: #2e2e4a;
  }

  .status {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #888;
    font-size: 1rem;
    text-align: center;
    padding: 20px;
  }

  .status small {
    color: #555;
    font-size: 0.85rem;
    margin-top: 8px;
  }

  .error {
    color: #ff6b6b;
  }
</style>
