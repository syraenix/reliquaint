<script>
  import { createEventDispatcher } from "svelte";
  import GameCard from "./GameCard.svelte";

  export let games = [];
  const dispatch = createEventDispatcher();

  const STORAGE_KEY = "reliquaint:collection:expanded";

  let expanded = (() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : {};
      return (typeof parsed === "object" && parsed !== null) ? parsed : {};
    } catch {
      return {};
    }
  })();

  function saveExpanded() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(expanded));
    } catch { /* storage unavailable */ }
  }

  function autoFormatCollection(id) {
    return id.split("-").map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
  }

  $: collected = (() => {
    const map = new Map();
    for (const g of games) {
      if (!g.collection) continue;
      if (!map.has(g.collection)) {
        const displayName = g.collection_name ?? autoFormatCollection(g.collection);
        map.set(g.collection, { displayName, games: [] });
      }
      map.get(g.collection).games.push(g);
    }
    const sorted = Array.from(map.entries()).sort((a, b) =>
      a[1].displayName.localeCompare(b[1].displayName)
    );
    for (const [, group] of sorted) {
      group.games.sort((a, b) => a.id.localeCompare(b.id));
    }
    return sorted;
  })();

  $: standalone = [...games]
    .filter((g) => !g.collection)
    .sort((a, b) => a.id.localeCompare(b.id));

  $: allExpanded =
    collected.length > 0 && collected.every(([id]) => expanded[id]);

  function toggleCollection(id) {
    const next = { ...expanded };
    if (next[id]) {
      delete next[id];
    } else {
      next[id] = true;
    }
    expanded = next;
    saveExpanded();
  }

  function expandAll() {
    const next = {};
    for (const [id] of collected) next[id] = true;
    expanded = next;
    saveExpanded();
  }

  function collapseAll() {
    expanded = {};
    saveExpanded();
  }
</script>

<div class="grid-container">
  {#if games.length === 0}
    <div class="empty">No games match this filter.</div>
  {:else}
    {#if collected.length > 0}
      <div class="collection-toolbar">
        <span class="toolbar-label">Collections</span>
        <button class="toolbar-btn" on:click={allExpanded ? collapseAll : expandAll}>
          {allExpanded ? "collapse all" : "expand all"}
        </button>
      </div>

      {#each collected as [id, group] (id)}
        <section class="collection">
          <button class="collection-header" on:click={() => toggleCollection(id)}>
            <span class="toggle-icon">{expanded[id] ? "▼" : "▶"}</span>
            <span class="collection-name">{group.displayName}</span>
            <span class="game-count"
              >{group.games.length}
              {group.games.length === 1 ? "game" : "games"}</span
            >
          </button>
          {#if expanded[id]}
            <div class="grid">
              {#each group.games as game (game.id)}
                <GameCard {game} on:click={() => dispatch("select", game)} />
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    {/if}

    {#if standalone.length > 0}
      <section class="other-section">
        <div class="other-label">Other</div>
        <div class="grid">
          {#each standalone as game (game.id)}
            <GameCard {game} on:click={() => dispatch("select", game)} />
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .collection-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 2px;
    margin-bottom: 4px;
    border-bottom: 1px solid #2a2a40;
  }

  .toolbar-label {
    font-size: 0.7rem;
    font-weight: 600;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .toolbar-btn {
    background: none;
    border: 1px solid #3a3a55;
    color: #888;
    padding: 2px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.75rem;
    transition: color 0.15s, border-color 0.15s;
  }

  .toolbar-btn:hover {
    color: #ccc;
    border-color: #5555aa;
  }

  .collection {
    margin-bottom: 4px;
  }

  .collection-header {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 10px 2px;
    background: none;
    border: none;
    border-bottom: 1px solid #2a2a40;
    cursor: pointer;
    text-align: left;
    gap: 8px;
  }

  .collection-header:hover .collection-name {
    color: #c0c8ff;
  }

  .toggle-icon {
    font-size: 0.65rem;
    color: #a0a8ff;
    width: 12px;
    flex-shrink: 0;
  }

  .collection-name {
    flex: 1;
    font-size: 0.9rem;
    font-weight: 600;
    color: #a0a8ff;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    transition: color 0.15s;
  }

  .game-count {
    font-size: 0.75rem;
    color: #555;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 14px;
    padding: 12px 0 16px;
  }

  .other-section {
    margin-top: 20px;
  }

  .other-label {
    font-size: 0.75rem;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 10px;
    padding-bottom: 6px;
    border-bottom: 1px solid #2a2a40;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: #555;
    font-size: 0.95rem;
  }
</style>
