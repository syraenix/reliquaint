<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  export let game;
  const dispatch = createEventDispatcher();

  let launching = false;
  let launchError = null;
  let launchExitedMessage = null;

  let installing = false;
  let installError = null;
  let installSuccessMessage = null;
  // When the backend reports MissingFiles, we hold the picked path and
  // the list here so the modal can show them and re-invoke with force.
  let pendingInstall = null; // { path, missing } | null

  const ACQUISITION_LABELS = [
    ["gog", "Get on GOG"],
    ["steam", "Get on Steam"],
    ["developer_site", "Developer's site"],
    ["archive", "Internet Archive"],
  ];

  $: acquisitionButtons = ACQUISITION_LABELS
    .map(([key, label]) => ({ key, label, url: game.acquisition?.[key] }))
    .filter((b) => !!b.url);

  async function handleOpenUrl(url) {
    try {
      await invoke("open_url", { url });
    } catch (e) {
      launchError = String(e);
    }
  }

  async function attemptInstall(path, force) {
    installing = true;
    installError = null;
    installSuccessMessage = null;
    try {
      const outcome = await invoke("install_game", {
        id: game.id,
        path,
        force,
      });
      if (outcome.status === "installed") {
        installSuccessMessage = `Installed (record at ${outcome.record_path}).`;
        pendingInstall = null;
        dispatch("installed");
      } else if (outcome.status === "missing_files") {
        pendingInstall = { path, missing: outcome.missing };
      }
    } catch (e) {
      installError = String(e);
      pendingInstall = null;
    } finally {
      installing = false;
    }
  }

  async function handleInstallClick() {
    installError = null;
    let picked;
    try {
      picked = await openDialog({
        directory: true,
        multiple: false,
        title: `Select directory containing ${game.title} files`,
      });
    } catch (e) {
      installError = String(e);
      return;
    }
    if (!picked) return;
    await attemptInstall(picked, false);
  }

  function confirmInstallAnyway() {
    if (pendingInstall) attemptInstall(pendingInstall.path, true);
  }

  function cancelInstall() {
    pendingInstall = null;
  }

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
          <button class="primary" on:click={handleInstallClick} disabled={installing}>
            {installing ? "Installing…" : "Install"}
          </button>
        {/if}
      </div>

      {#if installSuccessMessage}
        <p class="msg success">{installSuccessMessage}</p>
      {/if}
      {#if installError}
        <p class="msg error">{installError}</p>
      {/if}
      {#if launchExitedMessage}
        <p class="msg success">{launchExitedMessage}</p>
      {/if}
      {#if launchError}
        <p class="msg error">{launchError}</p>
      {/if}
    </div>
  </div>

  {#if pendingInstall}
    <div class="modal-overlay" on:click={cancelInstall}>
      <div class="modal" on:click|stopPropagation>
        <h3>Expected files not found</h3>
        <p>The directory you selected is missing these files the catalog expects:</p>
        <ul>
          {#each pendingInstall.missing as f}
            <li>{f}</li>
          {/each}
        </ul>
        <p class="modal-path">Directory: <code>{pendingInstall.path}</code></p>
        <div class="modal-actions">
          <button class="secondary" on:click={cancelInstall}>Cancel</button>
          <button class="primary" on:click={confirmInstallAnyway} disabled={installing}>
            Install anyway
          </button>
        </div>
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
    max-width: 520px;
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

  .modal-path {
    font-size: 0.82rem;
    color: #888;
    word-break: break-all;
  }

  .modal-path code {
    color: #aaa;
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

  .secondary:hover {
    background: #252538;
    color: #ddd;
  }
</style>
