<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import FilterBar from "./components/FilterBar.svelte";
  import GameGrid from "./components/GameGrid.svelte";
  import GameDetail from "./components/GameDetail.svelte";
  import DoctorPanel from "./components/DoctorPanel.svelte";
  import InstallPanel from "./components/InstallPanel.svelte";

  let games = [];
  let filter = "all";
  let selectedGame = null;
  let loading = true;
  let error = null;
  let doctorOpen = false;
  let installOpen = false;

  function toggleDoctor() {
    doctorOpen = !doctorOpen;
    if (doctorOpen) installOpen = false;
  }

  function toggleInstall() {
    installOpen = !installOpen;
    if (installOpen) doctorOpen = false;
  }

  async function loadGames() {
    loading = true;
    error = null;
    try {
      games = await invoke("list_games");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadGames();
    window.addEventListener("focus", loadGames);
    return () => window.removeEventListener("focus", loadGames);
  });

  $: filtered = games.filter((g) => filter === "all" || g.platform === filter);
</script>

<div class="app">
  <header>
    <h1>Classic Launcher</h1>
    <div class="header-actions">
      <FilterBar bind:filter />
      <button class="doctor-btn" on:click={toggleInstall}>
        {installOpen ? "✕ Install" : "↓ Install"}
      </button>
      <button class="doctor-btn" on:click={toggleDoctor}>
        {doctorOpen ? "✕ Doctor" : "⚕ Doctor"}
      </button>
    </div>
  </header>

  {#if doctorOpen}
    <DoctorPanel />
  {:else if installOpen}
    <InstallPanel />
  {:else if selectedGame}
    <GameDetail
      game={selectedGame}
      on:back={() => (selectedGame = null)}
    />
  {:else if loading}
    <div class="status">Loading games…</div>
  {:else if error}
    <div class="status error">Error loading games: {error}</div>
  {:else}
    <GameGrid
      games={filtered}
      on:select={(e) => (selectedGame = e.detail)}
    />
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

  .doctor-btn {
    background: #252538;
    border: 1px solid #3a3a55;
    color: #a0a8ff;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .doctor-btn:hover {
    background: #2e2e4a;
  }

  .status {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #888;
    font-size: 1rem;
  }

  .error {
    color: #ff6b6b;
  }
</style>
