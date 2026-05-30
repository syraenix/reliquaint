<script>
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import DiagnosticPanel from "./DiagnosticPanel.svelte";

  export let game;
  // Other catalog entries sharing this game's id but from a different tap
  // (conflict alternates), supplied by App.svelte.
  export let alternates = [];
  const dispatch = createEventDispatcher();

  let makingDefault = false;
  let makeDefaultError = null;

  // The local user tap always wins, so "make default" can't override it.
  $: canMakeDefault = game.tap_id !== "local" && alternates.some((a) => a.priority < game.priority);

  async function makeDefault() {
    makingDefault = true;
    makeDefaultError = null;
    try {
      await invoke("make_tap_default", { id: game.tap_id });
      dispatch("madeDefault");
    } catch (e) {
      makeDefaultError = String(e);
    } finally {
      makingDefault = false;
    }
  }

  let launching = false;
  let launchError = null;
  let launchExitedMessage = null;
  let showDiagnostics = false;

  // Submit-upstream state.
  let submitting = false;
  let submitMessage = null;
  let submitError = null;
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
    ["amiga_forever", "Amiga Forever"],
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

  async function handleSubmit() {
    submitting = true;
    submitError = null;
    submitMessage = null;
    try {
      const resp = await invoke("submit_manifest", { id: game.id });
      try {
        await navigator.clipboard.writeText(resp.content);
        submitMessage = `Manifest copied to clipboard. Paste into ${resp.target_path} in the upstream PR.`;
      } catch (e) {
        // Clipboard may be unavailable in some webviews — fall back to a hint.
        submitMessage = `Open ${resp.target_path} on GitHub and paste the manifest. (clipboard unavailable: ${e})`;
      }
      try {
        await invoke("open_url", { url: resp.github_url });
      } catch (e) {
        submitError = `Couldn't open the browser: ${e}. URL: ${resp.github_url}`;
      }
      if (resp.warnings && resp.warnings.length > 0) {
        submitMessage += `\nReviewer warnings: ${resp.warnings.join("; ")}`;
      }
    } catch (e) {
      submitError = String(e);
    } finally {
      submitting = false;
    }
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

      {#if alternates.length > 0}
        <section class="alternates">
          <span class="alt-label">Also in:</span>
          {#each alternates as alt (alt.tap_id)}
            <button
              class="alt-link"
              title="View the {alt.tap_id} version"
              on:click={() => dispatch("viewAlternate", { tap_id: alt.tap_id })}
            >
              {alt.tap_id}
            </button>
          {/each}
          {#if canMakeDefault}
            <button class="make-default" on:click={makeDefault} disabled={makingDefault}>
              {makingDefault ? "Setting…" : "Make this version the default"}
            </button>
          {/if}
        </section>
        {#if makeDefaultError}
          <p class="msg error">{makeDefaultError}</p>
        {/if}
      {/if}

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
        {#if game.tap_id === "local"}
          <button class="secondary" on:click={handleSubmit} disabled={submitting}>
            {submitting ? "Preparing…" : "Submit upstream"}
          </button>
        {/if}
      </div>
      {#if submitMessage}
        <p class="msg success">{submitMessage}</p>
      {/if}
      {#if submitError}
        <p class="msg error">{submitError}</p>
      {/if}

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
    color: var(--ink-secondary);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 4px 0;
    margin-bottom: 20px;
  }

  .back:hover {
    color: var(--ink-primary);
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
    background: var(--status-installed-bg);
    color: var(--status-installed);
    border: 1px solid var(--status-installed-border);
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
    flex: 1;
    min-width: 0;
  }

  h2 {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--ink-primary);
    margin-bottom: 16px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin-bottom: 16px;
  }

  dt {
    color: var(--ink-muted);
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    align-self: center;
  }

  dd {
    color: var(--ink-secondary);
    font-size: 0.9rem;
    word-break: break-word;
  }

  .id {
    font-family: monospace;
    color: var(--ink-muted);
    font-size: 0.85rem;
  }

  .alternates {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 16px;
  }

  .alt-label {
    font-size: 0.78rem;
    color: var(--ink-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .alt-link {
    background: transparent;
    border: 1px solid var(--border-light);
    color: var(--gold-bright);
    padding: 3px 10px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.82rem;
    font-family: monospace;
  }

  .alt-link:hover {
    background: var(--bg-hover);
  }

  .make-default {
    background: transparent;
    border: 1px solid var(--gold-ink);
    color: var(--gold-ink);
    padding: 3px 12px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .make-default:hover:not(:disabled) {
    background: var(--gold-ink);
    color: var(--bg-base);
  }

  .make-default:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .description {
    color: var(--ink-secondary);
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
    color: var(--gold-ink);
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
    background: transparent;
    border: 1px solid var(--gold-ink);
    color: var(--gold-ink);
    padding: 7px 14px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .acq-btn:hover {
    background: var(--gold-ink);
    border-color: var(--gold-ink);
    color: var(--bg-base);
  }

  .acquisition-notes {
    color: var(--ink-secondary);
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
    background: var(--action);
    border: 1px solid var(--action-deep);
    color: var(--ink-primary);
    padding: 10px 28px;
    border-radius: 7px;
    cursor: pointer;
    font-size: 1rem;
    font-weight: 500;
    transition: background 0.15s;
  }

  .primary:hover:not(:disabled) {
    background: var(--action-deep);
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
    background: var(--status-installed-bg);
    color: var(--status-installed);
  }

  .error {
    background: var(--status-error-bg);
    color: var(--status-error);
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
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    border-radius: 8px;
    padding: 24px;
    max-width: 560px;
    width: 90%;
    max-height: 80vh;
    overflow-y: auto;
    color: var(--text-primary);
  }

  .modal h3 {
    font-size: 1.05rem;
    color: var(--status-warn);
    margin-bottom: 14px;
  }

  .modal p {
    font-size: 0.9rem;
    line-height: 1.5;
    margin-bottom: 12px;
  }

  .modal ul {
    margin: 0 0 16px 20px;
    color: var(--text-secondary);
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
    color: var(--text-muted);
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
    color: var(--text-secondary);
    word-break: break-all;
  }

  .chosen code {
    color: var(--gold-bright);
  }

  .link {
    background: none;
    border: none;
    color: var(--gold-bright);
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0 0 0 8px;
  }

  .link:hover:not(:disabled) {
    color: var(--gold);
    text-decoration: underline;
  }

  .link:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .install-log {
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 10px;
    margin: 0 0 12px;
    max-height: 180px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 0.78rem;
    color: var(--text-secondary);
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
    border: 1px solid var(--border-light);
    color: var(--text-secondary);
    padding: 8px 18px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .secondary:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
