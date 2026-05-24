<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  export let game;
  const dispatch = createEventDispatcher();

  let launching = false;
  let launchError = null;
  let launchExitedMessage = null;

  async function handleLaunch() {
    launching = true;
    launchError = null;
    launchExitedMessage = null;
    try {
      await invoke("launch_game", { id: game.id });
      launchExitedMessage = "Session ended.";
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
    <div class="header platform-{game.platform}">
      <span class="platform-label">{game.platform.toUpperCase()}</span>
      {#if game.installed}
        <span class="installed-badge">INSTALLED</span>
      {/if}
    </div>

    <div class="info">
      <h2>{game.title}</h2>

      <dl>
        {#if game.year}
          <dt>Year</dt><dd>{game.year}</dd>
        {/if}
        {#if game.developer}
          <dt>Developer</dt><dd>{game.developer}</dd>
        {/if}
        {#if game.publisher && game.publisher !== game.developer}
          <dt>Publisher</dt><dd>{game.publisher}</dd>
        {/if}
        {#if game.genre && game.genre.length > 0}
          <dt>Genre</dt><dd>{game.genre.join(", ")}</dd>
        {/if}
        {#if game.collection}
          <dt>Collection</dt><dd>{game.collection}</dd>
        {/if}
        <dt>ID</dt><dd class="id">{game.id}</dd>
        <dt>Tap</dt><dd class="id">{game.tap_id}</dd>
        {#if game.install_path}
          <dt>Install path</dt><dd class="id">{game.install_path}</dd>
        {/if}
      </dl>

      {#if game.description}
        <p class="description">{game.description}</p>
      {/if}

      <div class="actions">
        {#if game.installed}
          <button class="primary" on:click={handleLaunch} disabled={launching}>
            {launching ? "Launching…" : "Launch"}
          </button>
        {:else}
          <button class="primary" disabled title="Install workflow lands in Task 5.3">
            Install (coming in Task 5.3)
          </button>
        {/if}
      </div>

      {#if launchExitedMessage}
        <p class="msg success">{launchExitedMessage}</p>
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

  .back:hover {
    color: #ccc;
  }

  .layout {
    display: flex;
    gap: 32px;
    align-items: flex-start;
    max-width: 900px;
  }

  .header {
    width: 240px;
    flex-shrink: 0;
    aspect-ratio: 4 / 3;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }

  .platform-label {
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    opacity: 0.7;
  }

  .installed-badge {
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    padding: 5px 12px;
    border-radius: 4px;
    background: rgba(106, 191, 106, 0.2);
    color: #6abf6a;
    border: 1px solid #6abf6a;
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
    flex: 1;
    min-width: 0;
  }

  h2 {
    font-size: 1.5rem;
    font-weight: 600;
    color: #e0e0f0;
    margin-bottom: 16px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin-bottom: 16px;
  }

  dt {
    color: #666;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    align-self: center;
  }

  dd {
    color: #bbb;
    font-size: 0.9rem;
    word-break: break-word;
  }

  .id {
    font-family: monospace;
    color: #888;
    font-size: 0.85rem;
  }

  .description {
    color: #bbb;
    font-size: 0.92rem;
    line-height: 1.6;
    margin-bottom: 20px;
    max-width: 60ch;
  }

  .actions {
    display: flex;
    gap: 12px;
    margin-top: 4px;
  }

  .primary {
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

  .primary:hover:not(:disabled) {
    background: #4a4a9a;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .msg {
    margin-top: 12px;
    font-size: 0.9rem;
    border-radius: 5px;
    padding: 8px 12px;
  }

  .success {
    background: #1a2e1a;
    color: #6abf6a;
  }

  .error {
    background: #2e1a1a;
    color: #ff8080;
  }
</style>
