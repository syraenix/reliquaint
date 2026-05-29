<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  const dispatch = createEventDispatcher();

  // Wizard state. `step` advances 'idle' -> 'detected' -> 'saving'/'saved'.
  let step = "idle";
  let detecting = false;
  let saving = false;
  let error = null;
  let report = null;
  let sourcePath = null;

  // Form fields populated from the report and editable by the user.
  let id = "";
  let title = "";
  let platform = "dos";
  let entry = "";
  let floppies = [];
  let year = "";
  let developer = "";
  let publisher = "";
  let description = "";

  // Mirror is_valid_id from src-tauri/src/catalog.rs.
  $: idValid = isValidId(id);
  $: titleValid = title.trim().length > 0;
  $: entryValid =
    platform === "dos"
      ? entry.trim().length > 0
      : platform === "amiga"
      ? floppies.length > 0
      : false;
  $: canSave = idValid && titleValid && entryValid && !saving;

  function isValidId(s) {
    if (!s) return false;
    if (s.length < 2 || s.length > 64) return false;
    if (!/^[a-z]/.test(s)) return false;
    if (!/[a-z0-9]$/.test(s)) return false;
    if (!/^[a-z0-9-]+$/.test(s)) return false;
    if (s.includes("--")) return false;
    return true;
  }

  async function pickAndDetect() {
    error = null;
    detecting = true;
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose the game directory",
      });
      if (!picked) {
        detecting = false;
        return;
      }
      sourcePath = picked;
      report = await invoke("detect_game", { path: picked });
      seedFormFromReport();
      step = "detected";
    } catch (e) {
      error = String(e);
    } finally {
      detecting = false;
    }
  }

  function seedFormFromReport() {
    id = report.metadata.id || "";
    title = report.metadata.title || "";
    if (report.platform) {
      platform = report.platform;
    } else if (report.dos_candidates.length > 0) {
      platform = "dos";
    } else if (report.amiga && report.amiga.kind === "floppies") {
      platform = "amiga";
    }
    if (platform === "dos" && report.dos_candidates.length > 0) {
      entry = report.dos_candidates[0].file_name;
    } else {
      entry = "";
    }
    if (
      platform === "amiga" &&
      report.amiga &&
      report.amiga.kind === "floppies"
    ) {
      floppies = [...report.amiga.files];
    } else {
      floppies = [];
    }
  }

  function onPlatformChange() {
    if (platform === "dos" && report?.dos_candidates?.length > 0 && !entry) {
      entry = report.dos_candidates[0].file_name;
    }
    if (
      platform === "amiga" &&
      report?.amiga?.kind === "floppies" &&
      floppies.length === 0
    ) {
      floppies = [...report.amiga.files];
    }
  }

  async function save() {
    if (!canSave) return;
    saving = true;
    error = null;
    try {
      const payload = {
        id,
        title,
        platform,
        install_path: sourcePath,
        entry: platform === "dos" ? entry : null,
        floppies: platform === "amiga" ? floppies : null,
        year: year ? Number(year) : null,
        developer: developer || null,
        publisher: publisher || null,
        description: description || null,
      };
      await invoke("save_user_manifest", { req: payload });
      step = "saved";
      dispatch("saved", { id });
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function cancel() {
    dispatch("close");
  }
</script>

<div class="modal-overlay" on:click={cancel}>
  <div class="modal" on:click|stopPropagation>
    <h3>Add a game</h3>

    {#if step === "idle"}
      <p>
        Choose a directory that contains a DOS or Amiga game you already
        have on disk. Reliquaint will inspect it, propose a manifest,
        and let you tweak before saving to your local tap.
      </p>
      <div class="modal-actions">
        <button class="secondary" on:click={cancel}>Cancel</button>
        <button class="primary" on:click={pickAndDetect} disabled={detecting}>
          {detecting ? "Detecting…" : "Choose directory…"}
        </button>
      </div>
    {/if}

    {#if step === "detected"}
      <p class="hint">
        Source: <code>{sourcePath}</code>
      </p>

      <div class="field">
        <label for="title">Title</label>
        <input id="title" type="text" bind:value={title} />
        {#if !titleValid}
          <span class="warn">required</span>
        {/if}
      </div>

      <div class="field">
        <label for="id">ID (catalog identifier)</label>
        <input id="id" type="text" bind:value={id} />
        {#if id && !idValid}
          <span class="warn">
            must match <code>^[a-z][a-z0-9-]*[a-z0-9]$</code>, 2–64
            chars, no consecutive dashes
          </span>
        {/if}
      </div>

      <div class="field">
        <label for="platform">Platform</label>
        <select id="platform" bind:value={platform} on:change={onPlatformChange}>
          <option value="dos">DOS</option>
          <option value="amiga">Amiga</option>
        </select>
        {#if report?.confidence}
          <span class="hint-inline">detected: {report.confidence}</span>
        {/if}
      </div>

      {#if platform === "dos"}
        <div class="field">
          <label for="entry">Entry point</label>
          {#if report?.dos_candidates?.length > 0}
            <select id="entry" bind:value={entry}>
              {#each report.dos_candidates as candidate}
                <option value={candidate.file_name}>
                  {candidate.file_name} — {candidate.reason}
                </option>
              {/each}
            </select>
          {:else}
            <input
              id="entry"
              type="text"
              bind:value={entry}
              placeholder="GAME.EXE"
            />
          {/if}
          {#if !entryValid}
            <span class="warn">required</span>
          {/if}
        </div>
      {:else if platform === "amiga"}
        <div class="field">
          <label>Floppy disks (mount order)</label>
          {#if floppies.length === 0}
            <p class="warn">No .ADF files detected — add via hand-edit.</p>
          {:else}
            <ul class="floppy-list">
              {#each floppies as f}
                <li><code>{f}</code></li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}

      <details class="optional">
        <summary>Optional metadata</summary>
        <div class="field">
          <label for="year">Year</label>
          <input id="year" type="number" bind:value={year} min="1970" max="2100" />
        </div>
        <div class="field">
          <label for="developer">Developer</label>
          <input id="developer" type="text" bind:value={developer} />
        </div>
        <div class="field">
          <label for="publisher">Publisher</label>
          <input id="publisher" type="text" bind:value={publisher} />
        </div>
        <div class="field">
          <label for="description">Description</label>
          <textarea id="description" rows="3" bind:value={description}></textarea>
        </div>
      </details>

      {#if error}
        <div class="error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="secondary" on:click={cancel} disabled={saving}>
          Cancel
        </button>
        <button class="primary" on:click={save} disabled={!canSave}>
          {saving ? "Saving…" : "Save manifest"}
        </button>
      </div>
    {/if}

    {#if step === "saved"}
      <p>Saved <code>{id}</code> to your local tap.</p>
      <div class="modal-actions">
        <button class="primary" on:click={cancel}>Close</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 60;
  }

  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    border-radius: 8px;
    padding: 24px;
    max-width: 560px;
    width: 90%;
    max-height: 85vh;
    overflow-y: auto;
    color: var(--text-primary);
  }

  h3 {
    font-size: 1.05rem;
    color: var(--gold);
    margin: 0 0 14px;
  }

  p {
    font-size: 0.9rem;
    line-height: 1.5;
    margin-bottom: 12px;
    color: var(--text-secondary);
  }

  .hint code {
    color: var(--gold-bright);
    word-break: break-all;
  }

  .hint-inline {
    margin-left: 8px;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 14px;
  }

  .field label {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-muted);
  }

  .field input,
  .field select,
  .field textarea {
    background: var(--bg-deep);
    border: 1px solid var(--border);
    color: var(--text-primary);
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 0.9rem;
    font-family: inherit;
  }

  .field input:focus,
  .field select:focus,
  .field textarea:focus {
    outline: none;
    border-color: var(--gold);
  }

  .warn {
    font-size: 0.75rem;
    color: var(--status-warn, #c87a3a);
  }

  .warn code {
    color: inherit;
  }

  .error {
    background: var(--status-error-bg, #4a2020);
    color: var(--status-error-ink, #f3b4b4);
    padding: 8px 10px;
    border-radius: 4px;
    font-size: 0.85rem;
    margin: 8px 0;
  }

  .floppy-list {
    margin: 0 0 0 18px;
    padding: 0;
    font-family: monospace;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .optional {
    margin-bottom: 14px;
  }

  .optional summary {
    cursor: pointer;
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 10px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
  }

  .primary {
    background: var(--gold);
    border: 1px solid var(--gold-bright);
    color: var(--bg-deep);
    padding: 8px 18px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
    font-weight: 600;
  }

  .primary:hover:not(:disabled) {
    background: var(--gold-bright);
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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
</style>
