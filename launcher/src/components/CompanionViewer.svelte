<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  // Sanitized HTML from the backend (companion_render::render_markdown) and the
  // companion item it came from (for the header / attribution).
  export let html = "";
  export let item = null;

  const dispatch = createEventDispatcher();

  $: tapLabel =
    item?.tap_id === "local" ? "your local tap" : item?.tap_id ?? "";

  // Single delegated handler for the rendered content: intercept link clicks
  // (internal nav vs external browser) and image clicks (open the lightbox).
  async function onContentClick(event) {
    const anchor = event.target.closest("a");
    if (anchor) {
      event.preventDefault();
      const href = anchor.getAttribute("href") || "";
      if (href.startsWith("reliquaint-nav://")) {
        try {
          const url = new URL(href);
          const [game_id, ...rest] = url.pathname.replace(/^\//, "").split("/");
          dispatch("navigate", {
            tap_id: url.host,
            game_id,
            rel_path: rest.join("/"),
          });
        } catch (e) {
          // Malformed internal link — ignore rather than navigate anywhere.
        }
      } else if (href.startsWith("http://") || href.startsWith("https://")) {
        try {
          await invoke("open_url", { url: href });
        } catch (e) {
          // Non-fatal: opening the system browser failed.
        }
      }
      return;
    }

    const img = event.target.closest("img");
    if (img && img.getAttribute("src")) {
      dispatch("openImage", { src: img.getAttribute("src") });
    }
  }
</script>

<div class="guide-pane">
  <div class="guide-header">
    <button class="back" on:click={() => dispatch("back")}>← Back to guides</button>
    {#if item}
      <span class="guide-title">{item.title}</span>
      <span class="guide-attr">from {tapLabel}</span>
    {/if}
  </div>

  <!-- HTML is sanitized server-side (ammonia, strict whitelist). -->
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
  <div class="guide-content" on:click={onContentClick}>
    {@html html}
  </div>
</div>

<style>
  .guide-pane {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    border-radius: 8px;
    padding: 20px 24px;
    color: var(--text-primary);
  }

  .guide-header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--border);
    padding-bottom: 12px;
    margin-bottom: 16px;
  }

  .back {
    background: none;
    border: none;
    color: var(--gold-bright);
    cursor: pointer;
    font-size: 0.85rem;
    padding: 0;
  }

  .back:hover {
    color: var(--gold);
    text-decoration: underline;
  }

  .guide-title {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .guide-attr {
    font-size: 0.78rem;
    color: var(--text-muted);
    font-style: italic;
  }

  /* The rendered Markdown — these target the injected (sanitized) elements. */
  .guide-content {
    font-size: 0.92rem;
    line-height: 1.65;
    color: var(--text-primary);
    max-width: 72ch;
  }

  .guide-content :global(h1),
  .guide-content :global(h2),
  .guide-content :global(h3),
  .guide-content :global(h4),
  .guide-content :global(h5),
  .guide-content :global(h6) {
    color: var(--gold);
    line-height: 1.3;
    margin: 1.4em 0 0.5em;
  }

  .guide-content :global(h1) {
    font-size: 1.4rem;
  }
  .guide-content :global(h2) {
    font-size: 1.2rem;
  }
  .guide-content :global(h3) {
    font-size: 1.05rem;
  }

  .guide-content :global(p) {
    margin: 0 0 0.9em;
  }

  .guide-content :global(a) {
    color: var(--gold-bright);
    cursor: pointer;
  }
  .guide-content :global(a:hover) {
    text-decoration: underline;
  }

  .guide-content :global(ul),
  .guide-content :global(ol) {
    margin: 0 0 0.9em 1.4em;
  }
  .guide-content :global(li) {
    margin: 0.2em 0;
  }

  .guide-content :global(code) {
    background: var(--bg-deep);
    color: var(--text-primary);
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 0.85em;
  }

  .guide-content :global(pre) {
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px;
    overflow-x: auto;
    margin: 0 0 1em;
  }
  .guide-content :global(pre code) {
    background: none;
    padding: 0;
  }

  .guide-content :global(blockquote) {
    border-left: 3px solid var(--burgundy);
    margin: 0 0 1em;
    padding: 0.2em 0 0.2em 14px;
    color: var(--text-secondary);
  }

  .guide-content :global(table) {
    border-collapse: collapse;
    margin: 0 0 1em;
    font-size: 0.88rem;
  }
  .guide-content :global(th),
  .guide-content :global(td) {
    border: 1px solid var(--border-light);
    padding: 5px 10px;
    text-align: left;
  }
  .guide-content :global(th) {
    background: var(--bg-hover);
    color: var(--gold);
  }

  .guide-content :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: 6px;
    cursor: zoom-in;
    display: block;
    margin: 0.6em 0;
  }

  .guide-content :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1.4em 0;
  }

  .guide-content :global(del) {
    color: var(--text-muted);
  }
</style>
