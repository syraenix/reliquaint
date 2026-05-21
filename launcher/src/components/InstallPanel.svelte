<script>
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  const KQ_GAMES = [
    { id: "kq1sci", label: "King's Quest 1 (SCI)" },
    { id: "kq2", label: "King's Quest 2" },
    { id: "kq3", label: "King's Quest 3" },
    { id: "kq4", label: "King's Quest 4 (SCI)" },
    { id: "kq5", label: "King's Quest 5" },
    { id: "kq6", label: "King's Quest 6" },
  ];

  let activeTab = "quest-for-glory";

  // QFG state
  let qfgDir = "";
  let qfgEntries = [];
  let qfgSelected = {};
  let qfgDiscoverError = null;

  // KQ state — { kq1sci: { entry, error } }
  let kqState = {};

  // Shared install state
  let installing = false;
  let log = [];
  let entryStatus = {};
  let installError = null;
  let listenerError = null;

  let unlistenStarted = null;
  let unlistenOutput = null;
  let unlistenFinished = null;
  let unlistenAborted = null;
  let logEl;

  async function loadDefaultQfgDir() {
    try {
      const dir = await invoke("default_installers_dir", {
        collection: "quest-for-glory",
      });
      if (dir) {
        qfgDir = dir;
        await discoverQfg();
      }
    } catch (e) {
      qfgDiscoverError = String(e);
    }
  }

  async function pickQfgDir() {
    const picked = await openDialog({ directory: true, defaultPath: qfgDir || undefined });
    if (typeof picked === "string") {
      qfgDir = picked;
      await discoverQfg();
    }
  }

  async function discoverQfg() {
    qfgDiscoverError = null;
    qfgEntries = [];
    try {
      const entries = await invoke("discover_qfg_installers", { directory: qfgDir });
      qfgEntries = entries;
      const next = {};
      for (const e of entries) next[e.game_id] = true;
      qfgSelected = next;
    } catch (e) {
      qfgDiscoverError = String(e);
    }
  }

  async function pickKqSource(gameId) {
    const picked = await openDialog({ directory: true });
    if (typeof picked !== "string") return;
    try {
      const entry = await invoke("build_kq_entry", { gameId, directory: picked });
      kqState[gameId] = { entry, error: null };
      kqState = { ...kqState };
    } catch (e) {
      kqState[gameId] = { entry: null, error: String(e) };
      kqState = { ...kqState };
    }
  }

  function clearKq(gameId) {
    delete kqState[gameId];
    kqState = { ...kqState };
  }

  function appendLog(line, stream, gameId) {
    log = [...log, { line, stream, gameId }];
    queueScrollToBottom();
  }

  function queueScrollToBottom() {
    requestAnimationFrame(() => {
      if (logEl) logEl.scrollTop = logEl.scrollHeight;
    });
  }

  async function installQfg() {
    const entries = qfgEntries.filter((e) => qfgSelected[e.game_id]);
    if (entries.length === 0) return;
    await runInstall("quest-for-glory", entries);
  }

  async function installKq() {
    const entries = Object.values(kqState)
      .map((s) => s.entry)
      .filter(Boolean);
    if (entries.length === 0) return;
    await runInstall("kings-quest", entries);
  }

  async function runInstall(collection, entries) {
    installing = true;
    installError = null;
    log = [];
    entryStatus = {};
    for (const e of entries) entryStatus[e.game_id] = "pending";
    entryStatus = { ...entryStatus };
    try {
      await invoke("install_games", { collection, entries });
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
    }
  }

  onMount(async () => {
    await loadDefaultQfgDir();
    try {
      unlistenStarted = await listen("game-install-started", (e) => {
        entryStatus[e.payload.game_id] = "installing";
        entryStatus = { ...entryStatus };
      });
      unlistenOutput = await listen("game-install-output", (e) => {
        appendLog(e.payload.line, e.payload.stream, e.payload.game_id);
      });
      unlistenFinished = await listen("game-install-finished", (e) => {
        const { game_id, exit_code } = e.payload;
        entryStatus[game_id] = exit_code === 0 ? "ok" : "failed";
        entryStatus = { ...entryStatus };
      });
      unlistenAborted = await listen("game-install-aborted", (e) => {
        const { game_id, exit_code } = e.payload;
        appendLog(`[aborted] ${game_id} exited with ${exit_code}`, "stderr", game_id);
      });
    } catch (e) {
      listenerError = String(e);
    }
  });

  onDestroy(() => {
    for (const u of [unlistenStarted, unlistenOutput, unlistenFinished, unlistenAborted]) {
      if (u) u();
    }
  });

  function statusBadge(status) {
    switch (status) {
      case "installing": return { label: "installing…", cls: "badge-installing" };
      case "ok": return { label: "ok", cls: "badge-ok" };
      case "failed": return { label: "failed", cls: "badge-failed" };
      case "pending": return { label: "pending", cls: "badge-pending" };
      default: return null;
    }
  }

  $: qfgSelectedCount = qfgEntries.filter((e) => qfgSelected[e.game_id]).length;
  $: kqEntryCount = Object.values(kqState).filter((s) => s.entry).length;
</script>

<div class="install">
  <h2>Install Games</h2>

  {#if listenerError}
    <p class="status warn">Live install output unavailable: {listenerError}</p>
  {/if}

  <div class="tabs">
    <button class:active={activeTab === "quest-for-glory"} on:click={() => (activeTab = "quest-for-glory")}>
      Quest for Glory
    </button>
    <button class:active={activeTab === "kings-quest"} on:click={() => (activeTab = "kings-quest")}>
      King's Quest
    </button>
  </div>

  {#if activeTab === "quest-for-glory"}
    <section class="panel">
      <div class="dir-row">
        <div class="dir-label">Installers directory</div>
        <div class="dir-path">{qfgDir || "(not set)"}</div>
        <button class="secondary" on:click={pickQfgDir} disabled={installing}>Change…</button>
      </div>

      {#if qfgDiscoverError}
        <p class="status error">{qfgDiscoverError}</p>
      {/if}

      {#if qfgEntries.length === 0 && !qfgDiscoverError}
        <p class="status">No GOG installers detected. Drop `qfg1.exe`–`qfg4.exe` into the installers directory, then click <strong>Change…</strong> and pick it again.</p>
      {:else}
        <ul class="entries">
          {#each qfgEntries as entry}
            {@const badge = statusBadge(entryStatus[entry.game_id])}
            <li>
              <label>
                <input
                  type="checkbox"
                  bind:checked={qfgSelected[entry.game_id]}
                  disabled={installing}
                />
                <span class="entry-label">{entry.label}</span>
                <span class="entry-target">→ {entry.target}</span>
              </label>
              {#if badge}
                <span class="badge {badge.cls}">{badge.label}</span>
              {/if}
            </li>
          {/each}
        </ul>
        <button
          class="primary"
          on:click={installQfg}
          disabled={installing || qfgSelectedCount === 0}
        >
          {installing ? "Installing…" : `Install ${qfgSelectedCount} selected`}
        </button>
      {/if}
    </section>
  {:else}
    <section class="panel">
      <p class="hint">
        Pick the Steam folder for each game that contains its DOS files (the folder with <code>*.EXE</code> and resource files at the top level). The Steam bundles are <code>Kings Quest 1+2+3</code> and <code>Kings Quest 4+5+6</code> — the per-game subfolder inside each is what you want.
      </p>
      <ul class="entries">
        {#each KQ_GAMES as g}
          {@const state = kqState[g.id]}
          {@const badge = statusBadge(entryStatus[g.id])}
          <li class="kq-row">
            <span class="entry-label">{g.label}</span>
            <span class="kq-path">
              {#if state?.entry}
                {state.entry.source}
              {:else if state?.error}
                <span class="error-text">{state.error}</span>
              {:else}
                <span class="muted">(not picked)</span>
              {/if}
            </span>
            <span class="kq-actions">
              <button class="secondary" on:click={() => pickKqSource(g.id)} disabled={installing}>
                {state?.entry ? "Change…" : "Pick folder…"}
              </button>
              {#if state?.entry}
                <button class="link" on:click={() => clearKq(g.id)} disabled={installing}>clear</button>
              {/if}
              {#if badge}
                <span class="badge {badge.cls}">{badge.label}</span>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
      <button
        class="primary"
        on:click={installKq}
        disabled={installing || kqEntryCount === 0}
      >
        {installing ? "Installing…" : `Install ${kqEntryCount} game${kqEntryCount === 1 ? "" : "s"}`}
      </button>
    </section>
  {/if}

  {#if installError}
    <p class="status error">{installError}</p>
  {/if}

  {#if log.length > 0 || installing}
    <pre class="log" bind:this={logEl}>
{#each log as entry}<span class="log-line {entry.stream}">[{entry.gameId}] {entry.line}
</span>{/each}{#if installing && log.length === 0}<span class="log-line stdout">starting…
</span>{/if}</pre>
  {/if}
</div>

<style>
  .install {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    max-width: 900px;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    color: #a0a8ff;
    margin-bottom: 16px;
  }

  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 18px;
    border-bottom: 1px solid #1e1e30;
  }

  .tabs button {
    background: transparent;
    border: none;
    color: #888;
    padding: 8px 16px;
    cursor: pointer;
    font-size: 0.9rem;
    border-bottom: 2px solid transparent;
  }

  .tabs button.active {
    color: #cfd2ff;
    border-bottom-color: #5555aa;
  }

  .tabs button:hover:not(.active) { color: #ccc; }

  .panel { margin-bottom: 20px; }

  .dir-row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid #1e1e30;
    margin-bottom: 14px;
  }

  .dir-label {
    font-size: 0.85rem;
    color: #888;
  }

  .dir-path {
    font-family: monospace;
    font-size: 0.82rem;
    color: #bbb;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    font-size: 0.85rem;
    color: #888;
    margin-bottom: 14px;
    line-height: 1.5;
  }

  .hint code {
    background: #1a1a2e;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 0.82rem;
    color: #cfd2ff;
  }

  .entries {
    list-style: none;
    margin-bottom: 16px;
  }

  .entries li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
    border-bottom: 1px solid #1e1e30;
    font-size: 0.88rem;
  }

  .entries label {
    display: flex;
    align-items: baseline;
    gap: 10px;
    cursor: pointer;
    flex: 1;
    min-width: 0;
  }

  .entry-label { color: #ccc; }
  .entry-target {
    font-family: monospace;
    font-size: 0.78rem;
    color: #666;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kq-row {
    display: grid;
    grid-template-columns: 220px 1fr auto;
    gap: 12px;
    align-items: center;
  }

  .kq-path {
    font-family: monospace;
    font-size: 0.78rem;
    color: #bbb;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kq-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .muted { color: #555; font-family: sans-serif; }
  .error-text { color: #ff8080; }

  button.primary {
    background: #3a3a7a;
    border: 1px solid #5555aa;
    color: #c0c8ff;
    padding: 8px 20px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  button.primary:hover:not(:disabled) { background: #4a4a9a; }
  button.primary:disabled { opacity: 0.5; cursor: default; }

  button.secondary {
    background: #2a2a45;
    border: 1px solid #4a4a70;
    color: #cfd2ff;
    padding: 4px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
  }
  button.secondary:hover:not(:disabled) { background: #34345a; }
  button.secondary:disabled { opacity: 0.5; cursor: default; }

  button.link {
    background: transparent;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 0.78rem;
    padding: 0;
    text-decoration: underline;
  }
  button.link:hover:not(:disabled) { color: #ccc; }
  button.link:disabled { opacity: 0.5; cursor: default; }

  .badge {
    font-size: 0.72rem;
    padding: 2px 8px;
    border-radius: 10px;
    border: 1px solid currentColor;
    flex-shrink: 0;
  }
  .badge-pending { color: #888; }
  .badge-installing { color: #e0c060; }
  .badge-ok { color: #4caf50; }
  .badge-failed { color: #ff8080; }

  .log {
    margin-top: 14px;
    padding: 10px 12px;
    background: #0d0d18;
    border: 1px solid #1e1e30;
    border-radius: 4px;
    max-height: 320px;
    overflow-y: auto;
    font-family: ui-monospace, monospace;
    font-size: 0.78rem;
    line-height: 1.4;
    white-space: pre-wrap;
    color: #b8b8c8;
  }

  .log-line.stderr { color: #ff9090; }

  .status { color: #888; font-size: 0.88rem; padding: 8px 0; }
  .status.error { color: #ff8080; }
  .status.warn { color: #e0c060; font-size: 0.8rem; margin-bottom: 8px; }
</style>
