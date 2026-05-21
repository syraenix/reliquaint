<script>
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  // catalog entries from list_catalog
  let catalog = [];
  let loading = true;
  let error = null;

  // which collection install is open: "quest-for-glory" | "kings-quest" | null
  let activeCollectionInstall = null;

  // QFG sub-install state
  let qfgDir = "";
  let qfgEntries = [];
  let qfgSelected = {};
  let qfgDiscoverError = null;

  // KQ sub-install state (game_id -> { entry, error })
  let kqState = {};

  // Shared install progress
  let installing = false;
  let log = [];
  let entryStatus = {};
  let installError = null;
  let logEl;

  let unlistenStarted, unlistenOutput, unlistenFinished, unlistenAborted;

  async function loadCatalog() {
    loading = true;
    error = null;
    try {
      catalog = await invoke("list_catalog");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $: uninstalledQfg = catalog.filter(g => !g.installed && g.collection === "quest-for-glory");
  $: uninstalledKq  = catalog.filter(g => !g.installed && g.collection === "kings-quest");
  $: uninstalledAmiga = catalog.filter(g => !g.installed && g.platform === "amiga");
  $: hasUninstalled = uninstalledQfg.length > 0 || uninstalledKq.length > 0 || uninstalledAmiga.length > 0;

  // ─── QFG helpers ─────────────────────────────────────────────────
  async function loadDefaultQfgDir() {
    try {
      const dir = await invoke("default_installers_dir", { collection: "quest-for-glory" });
      if (dir) { qfgDir = dir; await discoverQfg(); }
    } catch (_) {}
  }

  async function pickQfgDir() {
    const picked = await openDialog({ directory: true, defaultPath: qfgDir || undefined });
    if (typeof picked === "string") { qfgDir = picked; await discoverQfg(); }
  }

  async function discoverQfg() {
    qfgDiscoverError = null;
    qfgEntries = [];
    try {
      qfgEntries = await invoke("discover_qfg_installers", { directory: qfgDir });
      const next = {};
      for (const e of qfgEntries) next[e.game_id] = true;
      qfgSelected = next;
    } catch (e) { qfgDiscoverError = String(e); }
  }

  // ─── KQ helpers ──────────────────────────────────────────────────
  async function pickKqSource(gameId) {
    const picked = await openDialog({ directory: true });
    if (typeof picked !== "string") return;
    try {
      const entry = await invoke("build_kq_entry", { gameId, directory: picked });
      kqState = { ...kqState, [gameId]: { entry, error: null } };
    } catch (e) {
      kqState = { ...kqState, [gameId]: { entry: null, error: String(e) } };
    }
  }

  function clearKq(gameId) {
    const next = { ...kqState };
    delete next[gameId];
    kqState = next;
  }

  // ─── Amiga install ───────────────────────────────────────────────
  async function installAmiga(gameId) {
    const picked = await openDialog({
      filters: [{ name: "Amiga files", extensions: ["adf", "hdf", "rp9"] }]
    });
    if (!picked) return;
    const src = typeof picked === "string" ? picked : picked[0];
    installing = true;
    installError = null;
    log = [];
    entryStatus = { [gameId]: "pending" };
    try {
      await invoke("install_amiga_game", { gameId, sourcePath: src });
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
      await loadCatalog();
    }
  }

  // ─── DOS collection install ──────────────────────────────────────
  async function installQfg() {
    const entries = qfgEntries.filter(e => qfgSelected[e.game_id]);
    if (entries.length === 0) return;
    installing = true;
    installError = null;
    log = [];
    entryStatus = {};
    for (const e of entries) entryStatus[e.game_id] = "pending";
    entryStatus = { ...entryStatus };
    try {
      await invoke("install_games", { collection: "quest-for-glory", entries });
    } catch (e) { installError = String(e); }
    finally { installing = false; await loadCatalog(); }
  }

  async function installKq() {
    const entries = Object.values(kqState).map(s => s.entry).filter(Boolean);
    if (entries.length === 0) return;
    installing = true;
    installError = null;
    log = [];
    entryStatus = {};
    for (const e of entries) entryStatus[e.game_id] = "pending";
    entryStatus = { ...entryStatus };
    try {
      await invoke("install_games", { collection: "kings-quest", entries });
    } catch (e) { installError = String(e); }
    finally { installing = false; await loadCatalog(); }
  }

  function appendLog(line, stream, gameId) {
    log = [...log, { line, stream, gameId }];
    requestAnimationFrame(() => { if (logEl) logEl.scrollTop = logEl.scrollHeight; });
  }

  function statusBadge(status) {
    switch (status) {
      case "installing": return { label: "installing…", cls: "badge-installing" };
      case "ok":         return { label: "ok",           cls: "badge-ok" };
      case "failed":     return { label: "failed",       cls: "badge-failed" };
      case "pending":    return { label: "pending",      cls: "badge-pending" };
      default:           return null;
    }
  }

  $: qfgSelectedCount = qfgEntries.filter(e => qfgSelected[e.game_id]).length;
  $: kqEntryCount = Object.values(kqState).filter(s => s.entry).length;

  onMount(async () => {
    await loadCatalog();
    await loadDefaultQfgDir();
    unlistenStarted  = await listen("game-install-started",  e => { entryStatus = { ...entryStatus, [e.payload.game_id]: "installing" }; });
    unlistenOutput   = await listen("game-install-output",   e => appendLog(e.payload.line, e.payload.stream, e.payload.game_id));
    unlistenFinished = await listen("game-install-finished", e => {
      const { game_id, exit_code } = e.payload;
      entryStatus = { ...entryStatus, [game_id]: exit_code === 0 ? "ok" : "failed" };
    });
    unlistenAborted  = await listen("game-install-aborted",  e => appendLog(`[aborted] ${e.payload.game_id} exited ${e.payload.exit_code}`, "stderr", e.payload.game_id));
  });

  onDestroy(() => {
    for (const u of [unlistenStarted, unlistenOutput, unlistenFinished, unlistenAborted]) {
      if (u) u();
    }
  });
</script>

<div class="catalog">
  <h2>Add Game</h2>

  {#if loading}
    <p class="status">Loading catalog…</p>
  {:else if error}
    <p class="status error">{error}</p>
  {:else if !hasUninstalled}
    <p class="status">All known games are already installed.</p>
  {:else}

    {#if uninstalledQfg.length > 0}
      <section>
        <div class="collection-header">
          <span>Quest for Glory ({uninstalledQfg.length} uninstalled)</span>
          <button class="secondary" on:click={() => activeCollectionInstall = activeCollectionInstall === "quest-for-glory" ? null : "quest-for-glory"} disabled={installing}>
            {activeCollectionInstall === "quest-for-glory" ? "Hide" : "Install collection…"}
          </button>
        </div>
        {#if activeCollectionInstall === "quest-for-glory"}
          <div class="sub-install">
            <div class="dir-row">
              <span class="dir-label">Installers directory</span>
              <span class="dir-path">{qfgDir || "(not set)"}</span>
              <button class="secondary" on:click={pickQfgDir} disabled={installing}>Change…</button>
            </div>
            {#if qfgDiscoverError}<p class="status error">{qfgDiscoverError}</p>{/if}
            {#if qfgEntries.length === 0 && !qfgDiscoverError}
              <p class="status">No installers found. Drop qfg1.exe–qfg4.exe into the directory then click Change…</p>
            {:else}
              <ul class="entries">
                {#each qfgEntries as entry}
                  {@const badge = statusBadge(entryStatus[entry.game_id])}
                  <li>
                    <label>
                      <input type="checkbox" bind:checked={qfgSelected[entry.game_id]} disabled={installing} />
                      <span class="entry-label">{entry.label}</span>
                    </label>
                    {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
                  </li>
                {/each}
              </ul>
              <button class="primary" on:click={installQfg} disabled={installing || qfgSelectedCount === 0}>
                {installing ? "Installing…" : `Install ${qfgSelectedCount} selected`}
              </button>
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    {#if uninstalledKq.length > 0}
      <section>
        <div class="collection-header">
          <span>King's Quest ({uninstalledKq.length} uninstalled)</span>
          <button class="secondary" on:click={() => activeCollectionInstall = activeCollectionInstall === "kings-quest" ? null : "kings-quest"} disabled={installing}>
            {activeCollectionInstall === "kings-quest" ? "Hide" : "Install collection…"}
          </button>
        </div>
        {#if activeCollectionInstall === "kings-quest"}
          <div class="sub-install">
            <p class="hint">Pick the Steam subfolder for each game (the folder with .EXE files at the top level).</p>
            <ul class="entries">
              {#each uninstalledKq as g}
                {@const state = kqState[g.id]}
                {@const badge = statusBadge(entryStatus[g.id])}
                <li class="kq-row">
                  <span class="entry-label">{g.title}</span>
                  <span class="kq-path">
                    {#if state?.entry}{state.entry.source}
                    {:else if state?.error}<span class="error-text">{state.error}</span>
                    {:else}<span class="muted">(not picked)</span>{/if}
                  </span>
                  <span class="kq-actions">
                    <button class="secondary" on:click={() => pickKqSource(g.id)} disabled={installing}>
                      {state?.entry ? "Change…" : "Pick folder…"}
                    </button>
                    {#if state?.entry}
                      <button class="link" on:click={() => clearKq(g.id)} disabled={installing}>clear</button>
                    {/if}
                    {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
                  </span>
                </li>
              {/each}
            </ul>
            <button class="primary" on:click={installKq} disabled={installing || kqEntryCount === 0}>
              {installing ? "Installing…" : `Install ${kqEntryCount} game${kqEntryCount === 1 ? "" : "s"}`}
            </button>
          </div>
        {/if}
      </section>
    {/if}

    {#if uninstalledAmiga.length > 0}
      <section>
        <div class="collection-header"><span>Amiga</span></div>
        <ul class="entries">
          {#each uninstalledAmiga as g}
            {@const badge = statusBadge(entryStatus[g.id])}
            <li>
              <span class="entry-label">{g.title}</span>
              <span class="amiga-actions">
                <button class="secondary" on:click={() => installAmiga(g.id)} disabled={installing}>
                  Install (.adf / .hdf / .rp9)…
                </button>
                {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

  {/if}

  {#if installError}
    <p class="status error">{installError}</p>
  {/if}

  {#if log.length > 0 || installing}
    <pre class="log" bind:this={logEl}>{#each log as entry}<span class="log-line {entry.stream}">[{entry.gameId}] {entry.line}
</span>{/each}{#if installing && log.length === 0}<span class="log-line stdout">starting…
</span>{/if}</pre>
  {/if}
</div>

<style>
  .catalog { flex: 1; overflow-y: auto; padding: 20px; max-width: 900px; }
  h2 { font-size: 1.1rem; font-weight: 600; color: #a0a8ff; margin-bottom: 16px; }
  section { margin-bottom: 24px; }
  .collection-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 8px 0; border-bottom: 1px solid #2a2a40; margin-bottom: 12px;
    color: #cfd2ff; font-size: 0.9rem;
  }
  .sub-install { padding-left: 12px; border-left: 2px solid #2a2a40; margin-top: 10px; }
  .dir-row {
    display: grid; grid-template-columns: auto 1fr auto;
    align-items: center; gap: 12px; padding: 8px 0;
    border-bottom: 1px solid #1e1e30; margin-bottom: 12px;
  }
  .dir-label { font-size: 0.85rem; color: #888; }
  .dir-path { font-family: monospace; font-size: 0.82rem; color: #bbb; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { font-size: 0.85rem; color: #888; margin-bottom: 12px; line-height: 1.5; }
  .entries { list-style: none; margin-bottom: 14px; }
  .entries li {
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px; padding: 8px 0; border-bottom: 1px solid #1e1e30; font-size: 0.88rem;
  }
  .entries label { display: flex; align-items: baseline; gap: 10px; cursor: pointer; flex: 1; }
  .entry-label { color: #ccc; }
  .kq-row { display: grid; grid-template-columns: 220px 1fr auto; gap: 12px; align-items: center; }
  .kq-path { font-family: monospace; font-size: 0.78rem; color: #bbb; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kq-actions, .amiga-actions { display: flex; align-items: center; gap: 8px; }
  .muted { color: #555; font-family: sans-serif; }
  .error-text { color: #ff8080; }
  button.primary { background: #3a3a7a; border: 1px solid #5555aa; color: #c0c8ff; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-size: 0.9rem; }
  button.primary:hover:not(:disabled) { background: #4a4a9a; }
  button.primary:disabled { opacity: 0.5; cursor: default; }
  button.secondary { background: #2a2a45; border: 1px solid #4a4a70; color: #cfd2ff; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 0.8rem; }
  button.secondary:hover:not(:disabled) { background: #34345a; }
  button.secondary:disabled { opacity: 0.5; cursor: default; }
  button.link { background: transparent; border: none; color: #888; cursor: pointer; font-size: 0.78rem; padding: 0; text-decoration: underline; }
  button.link:disabled { opacity: 0.5; cursor: default; }
  .badge { font-size: 0.72rem; padding: 2px 8px; border-radius: 10px; border: 1px solid currentColor; flex-shrink: 0; }
  .badge-pending { color: #888; } .badge-installing { color: #e0c060; }
  .badge-ok { color: #4caf50; } .badge-failed { color: #ff8080; }
  .log { margin-top: 14px; padding: 10px 12px; background: #0d0d18; border: 1px solid #1e1e30; border-radius: 4px; max-height: 280px; overflow-y: auto; font-family: ui-monospace, monospace; font-size: 0.78rem; line-height: 1.4; white-space: pre-wrap; color: #b8b8c8; }
  .log-line.stderr { color: #ff9090; }
  .status { color: #888; font-size: 0.88rem; padding: 8px 0; }
  .status.error { color: #ff8080; }
</style>
