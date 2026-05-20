<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";

  export let game;
  const dispatch = createEventDispatcher();

  let launching = false;
  let launchError = null;
  let launched = false;

  $: artworkUrl = game.artwork_path ? convertFileSrc(game.artwork_path) : null;

  async function handleLaunch() {
    launching = true;
    launchError = null;
    launched = false;
    try {
      await invoke("launch_game", { id: game.id });
      launched = true;
    } catch (e) {
      launchError = String(e);
    } finally {
      launching = false;
    }
  }
</script>

<div class="detail">
  <button class="back" on:click={() => dispatch("back")}>← Back</button>

  <div class="layout">
    <div class="artwork platform-{game.platform}">
      {#if artworkUrl}
        <img src={artworkUrl} alt="{game.title} cover art" />
      {:else}
        <span class="platform-label">{game.platform.toUpperCase()}</span>
      {/if}
    </div>

    <div class="info">
      <h2>{game.title}</h2>
      <dl>
        <dt>Platform</dt>
        <dd>{game.platform.toUpperCase()}</dd>
        <dt>Collection</dt>
        <dd>{game.collection}</dd>
        <dt>ID</dt>
        <dd class="id">{game.id}</dd>
      </dl>

      <button
        class="launch"
        on:click={handleLaunch}
        disabled={launching}
      >
        {launching ? "Launching…" : "Launch"}
      </button>

      {#if launched}
        <p class="msg success">Session ended.</p>
      {/if}
      {#if launchError}
        <p class="msg error">{launchError}</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .detail {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .back {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 0.9rem;
    padding: 4px 0;
    margin-bottom: 20px;
  }

  .back:hover { color: #ccc; }

  .layout {
    display: flex;
    gap: 32px;
    align-items: flex-start;
  }

  .artwork {
    width: 280px;
    flex-shrink: 0;
    aspect-ratio: 4 / 3;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 8px;
  }

  .platform-label {
    font-size: 1rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    opacity: 0.6;
  }

  .platform-dos { background: #1a2e1a; color: #6abf6a; }
  .platform-amiga { background: #2e1a1a; color: #bf6a6a; }

  .info { flex: 1; }

  h2 {
    font-size: 1.5rem;
    font-weight: 600;
    color: #e0e0f0;
    margin-bottom: 16px;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 16px;
    margin-bottom: 24px;
  }

  dt {
    color: #666;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    align-self: center;
  }

  dd {
    color: #bbb;
    font-size: 0.9rem;
  }

  .id { font-family: monospace; color: #888; }

  .launch {
    background: #3a3a7a;
    border: 1px solid #5555aa;
    color: #c0c8ff;
    padding: 10px 28px;
    border-radius: 7px;
    cursor: pointer;
    font-size: 1rem;
    font-weight: 500;
    transition: background 0.15s;
  }

  .launch:hover:not(:disabled) { background: #4a4a9a; }
  .launch:disabled { opacity: 0.5; cursor: default; }

  .msg {
    margin-top: 12px;
    font-size: 0.9rem;
    border-radius: 5px;
    padding: 8px 12px;
  }

  .success { background: #1a2e1a; color: #6abf6a; }
  .error { background: #2e1a1a; color: #ff8080; }
</style>
