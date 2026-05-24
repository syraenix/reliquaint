<script>
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import DiagnosticPanel from "./DiagnosticPanel.svelte";

  export let game;
  const dispatch = createEventDispatcher();

  let launching = false;
  let launchError = null;
  let launchExitedMessage = null;
  let showDiagnostics = false;
  let unlistenExit = null;
  let unlistenInstallOutput = null;

  // Install modal state.
  let showInstallModal = false;
  let installing = false;
  let installError = null;
  let installSuccessMessage = null;
  let installSource = null; // selected source path (folder, .exe, or disk image)
  let customDest = null; // user-chosen library dir; null => default (~/games)
  let defaultDest = ""; // ~/games/<id>, shown when no custom dest is picked
  let installLog = []; // streamed copy/extract output lines
  // After the backend reports MissingFiles: { install_path, missing }.
  let pendingInstall = null;

  // File-picker filters per platform — DOS installs from a GOG .exe, Amiga
  // from a disk image. Folders are pickable on either platform.
  const FILE_FILTERS = {
    dos: [{ name: "DOS installer", extensions: ["exe"] }],
    amiga: [{ name: "Amiga disk image", extensions: ["adf", "hdf", "rp9"] }],
  };

  const ACQUISITION_LABELS = [
    ["gog", "Get on GOG"],
    ["steam", "Get on Steam"],
    ["developer_site", "Developer's site"],
    ["archive", "Internet Archive"],
  ];

  $: acquisitionButtons = ACQUISITION_LABELS
    .map(([key, label]) => ({ key, label, url: game.acquisition?.[key] }))
    .filter((b) => !!b.url);

  $: destDisplay = customDest ? `${customDest}/${game.id}` : defaultDest;

  async function handleOpenUrl(url) {
    try {
      await invoke("open_url", { url });
    } catch (e) {
      launchError = String(e);
    }
  }

  async function openInstallModal() {
    installError = null;
    installSuccessMessage = null;
    installSource = null;
    customDest = null;
    installLog = [];
    pendingInstall = null;
    try {
      defaultDest = await invoke("default_install_dest", { id: game.id });
    } catch (e) {
      defaultDest = "";
    }
    showInstallModal = true;
  }

  async function closeInstallModal() {
    if (installing) return;
    // A pending install has files staged but not yet committed — drop them.
    if (pendingInstall) await discardStaged();
    pendingInstall = null;
    showInstallModal = false;
  }

  // Best-effort removal of the staged-but-uncommitted copy.
  async function discardStaged() {
    try {
      await invoke("discard_install", { id: game.id, dest: customDest });
    } catch (e) {
      // Non-fatal: a stale staging dir is cleared on the next attempt anyway.
    }
  }

  async function chooseSourceFolder() {
    installError = null;
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: `Select ${game.title} game folder`,
      });
      if (picked) installSource = picked;
    } catch (e) {
      installError = String(e);
    }
  }

  async function chooseSourceFile() {
    installError = null;
    try {
      const picked = await openDialog({
        directory: false,
        multiple: false,
        filters: FILE_FILTERS[game.platform],
        title: `Select ${game.title} installer or disk image`,
      });
      if (picked) installSource = picked;
    } catch (e) {
      installError = String(e);
    }
  }

  async function chooseDest() {
    installError = null;
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose library folder",
      });
      if (picked) customDest = picked;
    } catch (e) {
      installError = String(e);
    }
  }

  async function startInstall() {
    if (!installSource) return;
    installing = true;
    installError = null;
    installSuccessMessage = null;
    installLog = [];
    pendingInstall = null;
    try {
      const outcome = await invoke("install_game", {
        id: game.id,
        source: installSource,
        dest: customDest,
      });
      if (outcome.status === "installed") {
        installSuccessMessage = `Installed to ${outcome.install_path}.`;
        showInstallModal = false;
        dispatch("installed");
      } else if (outcome.status === "missing_files") {
        pendingInstall = {
          install_path: outcome.install_path,
          missing: outcome.missing,
        };
      }
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
    }
  }

  async function confirmInstallAnyway() {
    if (!pendingInstall) return;
    installing = true;
    installError = null;
    try {
      // Commit the staged copy into place and write the record.
      await invoke("commit_install", { id: game.id, dest: customDest });
      installSuccessMessage = `Installed to ${pendingInstall.install_path}.`;
      pendingInstall = null;
      showInstallModal = false;
      dispatch("installed");
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
    }
  }

  async function cancelPending() {
    await discardStaged();
    pendingInstall = null;
  }

  async function handleLaunch() {
    launching = true;
    launchError = null;
    launchExitedMessage = null;
    showDiagnostics = true;
    try {
      // launch_game returns immediately after spawning a background
      // thread. The frontend tracks lifecycle via "emulator-exit"
      // events (wired up below).
      await invoke("launch_game", { id: game.id });
    } catch (e) {
      launchError = String(e);
      launching = false;
    }
  }

  onMount(async () => {
    unlistenExit = await listen("emulator-exit", (e) => {
      const payload = e.payload || {};
      if (payload.id !== game.id) return;
      launching = false;
      if (payload.error) {
        launchError = payload.error;
      } else if (payload.code === 0) {
        launchExitedMessage = "Session ended.";
      } else {
        launchError = `emulator exited with code ${payload.code}`;
      }
    });
    unlistenInstallOutput = await listen("install-output", (e) => {
      const payload = e.payload || {};
      if (payload.id !== game.id) return;
      installLog = [...installLog, payload.line];
    });
  });

  onDestroy(() => {
    unlistenExit?.();
    unlistenInstallOutput?.();
  });
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

      {#if acquisitionButtons.length > 0 || game.acquisition?.notes}
        <section class="acquisition">
          <h3>How to obtain</h3>
          {#if acquisitionButtons.length > 0}
            <div class="acquisition-buttons">
              {#each acquisitionButtons as btn (btn.key)}
                <button class="acq-btn" on:click={() => handleOpenUrl(btn.url)}>
                  {btn.label}
                </button>
              {/each}
            </div>
          {/if}
          {#if game.acquisition?.notes}
            <p class="acquisition-notes">{game.acquisition.notes}</p>
          {/if}
        </section>
      {/if}

      <div class="actions">
        {#if game.installed}
          <button class="primary" on:click={handleLaunch} disabled={launching}>
            {launching ? "Launching…" : "Launch"}
          </button>
        {:else}
          <button class="primary" on:click={openInstallModal}>Install</button>
        {/if}
      </div>

      {#if installSuccessMessage}
        <p class="msg success">{installSuccessMessage}</p>
      {/if}
      {#if launchExitedMessage}
        <p class="msg success">{launchExitedMessage}</p>
      {/if}
      {#if launchError}
        <p class="msg error">{launchError}</p>
      {/if}

      {#if showDiagnostics}
        <div class="diagnostics">
          <DiagnosticPanel />
        </div>
      {/if}
    </div>
  </div>

  {#if showInstallModal}
    <div class="modal-overlay" on:click={closeInstallModal}>
      <div class="modal" on:click|stopPropagation>
        {#if pendingInstall}
          <h3>Expected files not found</h3>
          <p>
            {game.title} is staged for
            <code>{pendingInstall.install_path}</code>, but these files the
            catalog expects are missing:
          </p>
          <ul>
            {#each pendingInstall.missing as f}
              <li>{f}</li>
            {/each}
          </ul>
          <p>Install anyway, or cancel to discard the staged copy?</p>
          {#if installError}
            <p class="msg error">{installError}</p>
          {/if}
          <div class="modal-actions">
            <button class="secondary" on:click={cancelPending} disabled={installing}>
              Cancel
            </button>
            <button class="primary" on:click={confirmInstallAnyway} disabled={installing}>
              {installing ? "Working…" : "Install anyway"}
            </button>
          </div>
        {:else}
          <h3>Install {game.title}</h3>

          <div class="install-field">
            <span class="field-label">Source</span>
            <div class="source-buttons">
              <button class="secondary" on:click={chooseSourceFolder} disabled={installing}>
                Choose folder…
              </button>
              <button class="secondary" on:click={chooseSourceFile} disabled={installing}>
                {game.platform === "dos" ? "Choose .exe…" : "Choose disk image…"}
              </button>
            </div>
            {#if installSource}
              <p class="chosen"><code>{installSource}</code></p>
            {/if}
          </div>

          <div class="install-field">
            <span class="field-label">Install to</span>
            <p class="chosen">
              <code>{destDisplay}</code>
              <button class="link" on:click={chooseDest} disabled={installing}>
                Change…
              </button>
            </p>
          </div>

          {#if installLog.length > 0}
            <pre class="install-log">{installLog.join("\n")}</pre>
          {/if}
          {#if installError}
            <p class="msg error">{installError}</p>
          {/if}

          <div class="modal-actions">
            <button class="secondary" on:click={closeInstallModal} disabled={installing}>
              Cancel
            </button>
            <button
              class="primary"
              on:click={startInstall}
              disabled={installing || !installSource}
            >
              {installing ? "Installing…" : "Install"}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
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

  .acquisition {
    margin-bottom: 22px;
    max-width: 60ch;
  }

  .acquisition h3 {
    font-size: 0.78rem;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 10px;
  }

  .acquisition-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 10px;
  }

  .acq-btn {
    background: #252538;
    border: 1px solid #3a3a55;
    color: #c0c8ff;
    padding: 7px 14px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .acq-btn:hover {
    background: #2e2e4a;
    border-color: #5555aa;
  }

  .acquisition-notes {
    color: #999;
    font-size: 0.85rem;
    line-height: 1.5;
    font-style: italic;
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

  .diagnostics {
    margin-top: 20px;
    max-width: 100%;
  }

  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }

  .modal {
    background: #1e1e30;
    border: 1px solid #3a3a55;
    border-radius: 8px;
    padding: 24px;
    max-width: 560px;
    width: 90%;
    max-height: 80vh;
    overflow-y: auto;
    color: #ddd;
  }

  .modal h3 {
    font-size: 1.05rem;
    color: #ffaa44;
    margin-bottom: 14px;
  }

  .modal p {
    font-size: 0.9rem;
    line-height: 1.5;
    margin-bottom: 12px;
  }

  .modal ul {
    margin: 0 0 16px 20px;
    color: #bbb;
    font-family: monospace;
    font-size: 0.88rem;
  }

  .install-field {
    margin-bottom: 18px;
  }

  .field-label {
    display: block;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: #888;
    margin-bottom: 8px;
  }

  .source-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .chosen {
    margin: 8px 0 0;
    font-size: 0.82rem;
    color: #aaa;
    word-break: break-all;
  }

  .chosen code {
    color: #c0c8ff;
  }

  .link {
    background: none;
    border: none;
    color: #7a7add;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0 0 0 8px;
  }

  .link:hover:not(:disabled) {
    color: #9a9aff;
    text-decoration: underline;
  }

  .link:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .install-log {
    background: #12121e;
    border: 1px solid #2a2a40;
    border-radius: 5px;
    padding: 10px;
    margin: 0 0 12px;
    max-height: 180px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 0.78rem;
    color: #9a9ab0;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
  }

  .secondary {
    background: transparent;
    border: 1px solid #3a3a55;
    color: #aaa;
    padding: 8px 18px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .secondary:hover:not(:disabled) {
    background: #252538;
    color: #ddd;
  }

  .secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
