<script>
  import { createEventDispatcher } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";

  export let game;
  const dispatch = createEventDispatcher();

  // Display icon: the backend resolves an absolute path (install-dir art, with
  // tap-provided art as fallback); convertFileSrc serves it over the asset
  // protocol. Null → fall back to the colored platform header.
  $: iconUrl = game.icon_path ? convertFileSrc(game.icon_path) : null;

  // Short, glanceable tap-of-origin label. reliquaint-core → "core";
  // anything else is shown as its id, truncated if long.
  function tapLabel(tapId) {
    if (tapId === "reliquaint-core") return "core";
    return tapId.length > 10 ? `${tapId.slice(0, 9)}…` : tapId;
  }
</script>

<button class="card" on:click={() => dispatch("click")}>
  <div class="header platform-{game.platform}" class:has-art={iconUrl}>
    {#if iconUrl}
      <img class="art" src={iconUrl} alt="" />
    {:else}
      <span class="platform-label">{game.platform.toUpperCase()}</span>
    {/if}
    <span class="badges">
      {#if game.tap_id === "local"}
        <span class="custom-badge" title="Added via the manifest wizard">CUSTOM</span>
      {:else}
        <span class="tap-badge" title="From the {game.tap_id} tap">{tapLabel(game.tap_id)}</span>
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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px;
    aspect-ratio: 16 / 5;
  }

  .header.has-art {
    aspect-ratio: 4 / 3;
    padding: 8px;
  }

  .art {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
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
    /* Pin top-right and keep above the artwork when art fills the header. */
    margin-left: auto;
    align-self: flex-start;
    position: relative;
    z-index: 1;
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

  .tap-badge {
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 2px 6px;
    border-radius: 3px;
    background: transparent;
    color: currentColor;
    border: 1px solid currentColor;
    opacity: 0.45;
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
