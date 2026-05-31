<script>
  import { invoke } from "@tauri-apps/api/core";
  import CompanionViewer from "./CompanionViewer.svelte";

  export let game;

  let items = [];
  let loading = false;
  let error = null;

  // Open guide (markdown viewer) state.
  let openItem = null;
  let openHtml = null;
  let openError = null;

  // Lightbox state.
  let lightboxSrc = null;

  // Reload whenever the viewed game changes; reset any open guide/lightbox.
  let loadedFor = null;
  $: if (game && game.id !== loadedFor) {
    loadedFor = game.id;
    openItem = null;
    openHtml = null;
    lightboxSrc = null;
    loadCompanion(game.id);
  }

  async function loadCompanion(gameId) {
    loading = true;
    error = null;
    try {
      items = await invoke("list_companion", { gameId });
    } catch (e) {
      error = String(e);
      items = [];
    } finally {
      loading = false;
    }
  }

  function tapLabel(tapId) {
    return tapId === "local" ? "your local tap" : tapId;
  }

  function imageUrl(item) {
    return `reliquaint-content://${item.tap_id}/${item.game_id}/${item.rel_path}`;
  }

  // Group items by tap_id, preserving the backend's priority-then-file order.
  $: groups = (() => {
    const out = [];
    for (const item of items) {
      let g = out.find((x) => x.tap_id === item.tap_id);
      if (!g) {
        g = { tap_id: item.tap_id, label: tapLabel(item.tap_id), items: [] };
        out.push(g);
      }
      g.items.push(item);
    }
    return out;
  })();

  async function openItem_(item) {
    if (item.kind === "image") {
      lightboxSrc = imageUrl(item);
      return;
    }
    await renderGuide(item.tap_id, item.game_id, item.rel_path);
  }

  async function renderGuide(tapId, gameId, relPath) {
    openError = null;
    try {
      const html = await invoke("render_companion", {
        tapId,
        gameId,
        relPath,
      });
      openHtml = html;
      // Prefer the indexed item (for its title); fall back to a minimal one.
      openItem =
        items.find((i) => i.tap_id === tapId && i.rel_path === relPath) || {
          tap_id: tapId,
          game_id: gameId,
          rel_path: relPath,
          title: relPath,
        };
    } catch (e) {
      openError = String(e);
    }
  }

  function onNavigate(e) {
    const { tap_id, game_id, rel_path } = e.detail;
    renderGuide(tap_id, game_id, rel_path);
  }

  function closeViewer() {
    openItem = null;
    openHtml = null;
    openError = null;
  }

  function onKeydown(e) {
    if (e.key === "Escape" && lightboxSrc) lightboxSrc = null;
  }
</script>

<svelte:window on:keydown={onKeydown} />

<section class="guides">
  <h3>Guides</h3>

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if error}
    <p class="msg-error">Couldn't load guides: {error}</p>
  {:else if openHtml !== null}
    <CompanionViewer
      html={openHtml}
      item={openItem}
      on:back={closeViewer}
      on:navigate={onNavigate}
      on:openImage={(e) => (lightboxSrc = e.detail.src)}
    />
  {:else if items.length === 0}
    <p class="muted">No guides available for this game yet.</p>
  {:else}
    {#if openError}
      <p class="msg-error">{openError}</p>
    {/if}
    {#each groups as group (group.tap_id)}
      <div class="tap-group">
        <span class="tap-head">From {group.label}</span>
        <ul>
          {#each group.items as item (item.tap_id + "/" + item.rel_path)}
            <li>
              <button class="guide-item" on:click={() => openItem_(item)}>
                <span class="glyph">{item.kind === "image" ? "🖼" : "📄"}</span>
                <span class="item-title">{item.title}</span>
                {#if item.section}
                  <span class="item-section">{item.section}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  {/if}
</section>

{#if lightboxSrc}
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
  <div class="lightbox-overlay" on:click={() => (lightboxSrc = null)}>
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
    <img class="lightbox-img" src={lightboxSrc} alt="" on:click|stopPropagation />
    <button class="lightbox-close" on:click={() => (lightboxSrc = null)}>✕</button>
  </div>
{/if}

<style>
  .guides {
    margin-top: 28px;
    max-width: 860px;
  }

  .guides h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--gold-ink);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 12px;
  }

  .muted {
    color: var(--ink-muted);
    font-size: 0.88rem;
    font-style: italic;
  }

  .msg-error {
    color: var(--status-error-ink);
    font-size: 0.88rem;
  }

  .tap-group {
    margin-bottom: 16px;
  }

  .tap-head {
    display: block;
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-muted);
    margin-bottom: 6px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    margin: 0;
  }

  .guide-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 7px 10px;
    cursor: pointer;
    text-align: left;
    color: var(--ink-primary);
    font-size: 0.9rem;
  }

  .guide-item:hover {
    background: var(--bg-base);
    border-color: var(--border-on-light);
  }

  .glyph {
    font-size: 1rem;
    flex-shrink: 0;
  }

  .item-title {
    color: var(--ink-primary);
  }

  .item-section {
    font-size: 0.72rem;
    color: var(--ink-muted);
    background: var(--bg-base);
    border: 1px solid var(--border-on-light);
    border-radius: 4px;
    padding: 1px 6px;
    text-transform: lowercase;
  }

  .lightbox-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.82);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 60;
    overflow: auto;
    padding: 24px;
  }

  .lightbox-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    cursor: default;
    border-radius: 4px;
  }

  .lightbox-close {
    position: fixed;
    top: 16px;
    right: 20px;
    background: rgba(0, 0, 0, 0.5);
    border: 1px solid var(--border-light);
    color: var(--text-primary);
    font-size: 1.1rem;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    cursor: pointer;
  }

  .lightbox-close:hover {
    background: rgba(0, 0, 0, 0.8);
  }
</style>
