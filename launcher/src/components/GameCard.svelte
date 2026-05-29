<script>
  import { createEventDispatcher } from "svelte";

  export let game;
  const dispatch = createEventDispatcher();
</script>

<button class="card" on:click={() => dispatch("click")}>
  <div class="header platform-{game.platform}">
    <span class="platform-label">{game.platform.toUpperCase()}</span>
    <span class="badges">
      {#if game.tap_id === "local"}
        <span class="custom-badge" title="Added via the manifest wizard">CUSTOM</span>
      {/if}
      {#if game.installed}
        <span class="installed-badge">INSTALLED</span>
      {/if}
    </span>
  </div>
  <div class="info">
    <span class="title">{game.title}</span>
    <span class="meta">
      {#if game.year}{game.year}{/if}
      {#if game.year && game.developer} · {/if}
      {#if game.developer}{game.developer}{/if}
    </span>
  </div>
</button>

<style>
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    padding: 0;
    overflow: hidden;
    transition: border-color 0.15s, transform 0.1s;
    width: 100%;
    display: flex;
    flex-direction: column;
  }

  .card:hover {
    border-color: var(--burgundy-soft);
    transform: translateY(-2px);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px;
    aspect-ratio: 16 / 5;
  }

  .platform-label {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    opacity: 0.7;
  }

  .badges {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .installed-badge {
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 3px 8px;
    border-radius: 3px;
    background: var(--status-installed-bg);
    color: var(--status-installed);
    border: 1px solid var(--status-installed-border);
  }

  .custom-badge {
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    padding: 2px 6px;
    border-radius: 3px;
    background: transparent;
    color: currentColor;
    border: 1px dashed currentColor;
    opacity: 0.55;
  }

  .platform-dos {
    background: var(--platform-dos-bg);
    color: var(--platform-dos);
  }

  .platform-amiga {
    background: var(--platform-amiga-bg);
    color: var(--platform-amiga);
  }

  .info {
    padding: 10px 12px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .title {
    font-size: 0.88rem;
    color: var(--text-primary);
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
