<script>
  import { createEventDispatcher } from "svelte";
  import GameCard from "./GameCard.svelte";

  export let games = [];
  const dispatch = createEventDispatcher();

  // Group by collection ("(no collection)" bucket for entries without one).
  // Stable order: collections alphabetical, games alphabetical within.
  $: grouped = (() => {
    const buckets = new Map();
    for (const g of games) {
      const key = g.collection ?? "(no collection)";
      if (!buckets.has(key)) buckets.set(key, []);
      buckets.get(key).push(g);
    }
    const sorted = Array.from(buckets.entries()).sort((a, b) =>
      a[0].localeCompare(b[0])
    );
    for (const [, list] of sorted) list.sort((a, b) => a.id.localeCompare(b.id));
    return sorted;
  })();
</script>

<div class="grid-container">
  {#if games.length === 0}
    <div class="empty">No games match this filter.</div>
  {:else}
    {#each grouped as [collection, items] (collection)}
      <section class="group">
        <h2>{collection}</h2>
        <div class="grid">
          {#each items as game (game.id)}
            <GameCard {game} on:click={() => dispatch("select", game)} />
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .group {
    margin-bottom: 28px;
  }

  .group:last-child {
    margin-bottom: 0;
  }

  h2 {
    font-size: 0.9rem;
    font-weight: 600;
    color: #a0a8ff;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid #2a2a40;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 14px;
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
