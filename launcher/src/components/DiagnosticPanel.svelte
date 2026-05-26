<script>
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  // entries: [{ kind: "log"|"emulator", level?, stream?, text, ts }]
  let entries = [];
  let unlistenLog = null;
  let unlistenEmu = null;
  let unlistenExit = null;
  let panelEl;

  function push(entry) {
    entries = [...entries, { ...entry, ts: Date.now() }];
    // Cap size to avoid runaway memory.
    if (entries.length > 2000) {
      entries = entries.slice(-1500);
    }
    queueScroll();
  }

  function queueScroll() {
    setTimeout(() => {
      if (panelEl) panelEl.scrollTop = panelEl.scrollHeight;
    }, 0);
  }

  function clear() {
    entries = [];
  }

  onMount(async () => {
    unlistenLog = await listen("log", (e) => {
      const { level, target, message, fields } = e.payload || {};
      push({
        kind: "log",
        level: level ?? "INFO",
        target,
        text: message + (fields ? "  " + fields : ""),
      });
    });
    unlistenEmu = await listen("emulator-output", (e) => {
      const { stream, line } = e.payload || {};
      push({
        kind: "emulator",
        stream: stream ?? "stdout",
        text: line ?? "",
      });
    });
    unlistenExit = await listen("emulator-exit", (e) => {
      const { id, code, error } = e.payload || {};
      const tail = error
        ? `error: ${error}`
        : `exit code ${code}`;
      push({
        kind: "log",
        level: code === 0 ? "INFO" : "ERROR",
        target: "emulator",
        text: `[${id}] ${tail}`,
      });
    });
  });

  onDestroy(() => {
    unlistenLog?.();
    unlistenEmu?.();
    unlistenExit?.();
  });
</script>

<div class="panel" bind:this={panelEl}>
  <div class="toolbar">
    <span class="title">Diagnostic output</span>
    <button class="clear" on:click={clear} disabled={entries.length === 0}>
      Clear
    </button>
  </div>
  {#if entries.length === 0}
    <div class="empty">No output yet. Launcher events and emulator output appear here.</div>
  {:else}
    <div class="lines">
      {#each entries as e}
        <div
          class="line"
          class:log-error={e.kind === "log" && e.level === "ERROR"}
          class:log-warn={e.kind === "log" && e.level === "WARN"}
          class:log-info={e.kind === "log" && e.level === "INFO"}
          class:log-debug={e.kind === "log" && e.level === "DEBUG"}
          class:log-trace={e.kind === "log" && e.level === "TRACE"}
          class:emu-stdout={e.kind === "emulator" && e.stream === "stdout"}
          class:emu-stderr={e.kind === "emulator" && e.stream === "stderr"}
        >
          {#if e.kind === "log"}
            <span class="tag">{e.level}</span>
          {:else}
            <span class="tag tag-emu">{e.stream}</span>
          {/if}
          <span class="text">{e.text}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .panel {
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 6px;
    max-height: 260px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 0.8rem;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--bg-base);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .title {
    font-size: 0.75rem;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-family: system-ui, sans-serif;
  }

  .clear {
    background: transparent;
    border: 1px solid var(--border-light);
    color: var(--text-secondary);
    padding: 3px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.72rem;
    font-family: system-ui, sans-serif;
  }

  .clear:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .clear:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .empty {
    padding: 14px;
    color: var(--text-muted);
    text-align: center;
    font-family: system-ui, sans-serif;
    font-size: 0.85rem;
  }

  .lines {
    padding: 6px 0;
  }

  .line {
    display: flex;
    gap: 8px;
    padding: 2px 12px;
    line-height: 1.4;
  }

  .tag {
    flex-shrink: 0;
    width: 60px;
    color: var(--text-muted);
    font-size: 0.7rem;
    padding-top: 1px;
  }

  .log-error .tag { color: var(--status-error); }
  .log-warn  .tag { color: var(--status-warn); }
  .log-info  .tag { color: var(--status-installed); }
  .log-debug .tag { color: var(--text-secondary); }
  .log-trace .tag { color: var(--text-muted); }

  .tag-emu { color: var(--text-secondary); }
  .emu-stderr .tag { color: var(--status-error); }

  .text {
    color: var(--text-primary);
    word-break: break-word;
    white-space: pre-wrap;
  }

  .log-error .text { color: var(--status-error); }
  .log-warn .text { color: var(--status-warn); }
</style>
