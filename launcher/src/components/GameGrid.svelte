<script>
  import { createEventDispatcher } from "svelte";
  import GameCard from "./GameCard.svelte";

  export let games = [];
  const dispatch = createEventDispatcher();
</script>

<div class="grid-container">
  {#if games.length === 0}
    <div class="empty">No games found for this platform.</div>
  {:else}
    <div class="grid">
      {#each games as game (game.id)}
        <GameCard {game} on:click={() => dispatch("select", game)} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 16px;
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
