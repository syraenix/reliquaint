<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let results = [];
  let loading = true;
  let error = null;

  onMount(async () => {
    try {
      results = await invoke("run_doctor");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });
</script>

<div class="doctor">
  <h2>System Health</h2>

  {#if loading}
    <p class="status">Checking…</p>
  {:else if error}
    <p class="status error">{error}</p>
  {:else}
    <ul>
      {#each results as r}
        <li class="probe status-{r.status}">
          <span class="dot"></span>
          <span class="name">{r.name}</span>
          {#if r.detail}
            <span class="detail">{r.detail}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .doctor {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    max-width: 600px;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    color: #a0a8ff;
    margin-bottom: 16px;
  }

  ul {
    list-style: none;
  }

  .probe {
    display: grid;
    grid-template-columns: 14px 1fr;
    column-gap: 10px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid #1e1e30;
    font-size: 0.88rem;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    margin: auto;
  }

  .status-ok .dot { background: #4caf50; }
  .status-missing .dot { background: #f44336; }
  .status-unknown .dot { background: #888; }

  .name { color: #ccc; }

  .detail {
    grid-column: 2;
    color: #666;
    font-size: 0.8rem;
    font-family: monospace;
    margin-top: 2px;
  }

  .status { color: #888; font-size: 0.9rem; }
  .error { color: #ff8080; }
</style>
