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
    background: #15151f;
    border: 1px solid #2a2a40;
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
    background: #1a1a2e;
    border-bottom: 1px solid #2a2a40;
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .title {
    font-size: 0.75rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-family: system-ui, sans-serif;
  }

  .clear {
    background: transparent;
    border: 1px solid #3a3a55;
    color: #888;
    padding: 3px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.72rem;
    font-family: system-ui, sans-serif;
  }

  .clear:hover:not(:disabled) {
    background: #252538;
    color: #ccc;
  }

  .clear:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .empty {
    padding: 14px;
    color: #555;
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
    color: #666;
    font-size: 0.7rem;
    padding-top: 1px;
  }

  .log-error .tag { color: #ff6b6b; }
  .log-warn  .tag { color: #ffaa44; }
  .log-info  .tag { color: #6abf6a; }
  .log-debug .tag { color: #5588ff; }
  .log-trace .tag { color: #888; }

  .tag-emu { color: #aaa; }
  .emu-stderr .tag { color: #ff8080; }

  .text {
    color: #ccc;
    word-break: break-word;
    white-space: pre-wrap;
  }

  .log-error .text { color: #ffb0b0; }
  .log-warn .text { color: #ffd080; }
</style>
