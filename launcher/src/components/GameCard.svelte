<script>
  import { createEventDispatcher } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";

  export let game;
  const dispatch = createEventDispatcher();

  $: artworkUrl = game.artwork_path ? convertFileSrc(game.artwork_path) : null;
</script>

<button class="card" on:click={() => dispatch("click")}>
  <div class="artwork platform-{game.platform}">
    {#if artworkUrl}
      <img src={artworkUrl} alt="{game.title} artwork" />
    {:else}
      <span class="platform-label">{game.platform.toUpperCase()}</span>
    {/if}
  </div>
  <div class="info">
    <span class="title">{game.title}</span>
  </div>
</button>

<style>
  .card {
    background: #1e1e30;
    border: 1px solid #2a2a40;
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    padding: 0;
    overflow: hidden;
    transition: border-color 0.15s, transform 0.1s;
    width: 100%;
  }

  .card:hover {
    border-color: #5555aa;
    transform: translateY(-2px);
  }

  .artwork {
    width: 100%;
    aspect-ratio: 4 / 3;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .platform-label {
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    opacity: 0.7;
  }

  .platform-dos {
    background: #1a2e1a;
    color: #6abf6a;
  }

  .platform-amiga {
    background: #2e1a1a;
    color: #bf6a6a;
  }

  .info {
    padding: 10px 12px;
    border-top: 1px solid #2a2a40;
  }

  .title {
    display: block;
    font-size: 0.82rem;
    color: #ccc;
    line-height: 1.35;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
