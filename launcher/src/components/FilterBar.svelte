<script>
  export let filter = "all";
  const options = [
    { value: "all", label: "All" },
    { value: "dos", label: "DOS" },
    { value: "amiga", label: "Amiga" },
  ];
</script>

<nav class="filter-bar">
  {#each options as opt}
    <button
      class:active={filter === opt.value}
      on:click={() => (filter = opt.value)}
    >
      {opt.label}
    </button>
  {/each}
</nav>

<style>
  .filter-bar {
    display: flex;
    gap: 4px;
  }

  button {
    background: var(--bg-elevated);
    border: 1px solid var(--border-light);
    color: var(--text-secondary);
    padding: 5px 14px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.82rem;
    transition: background 0.15s, color 0.15s;
  }

  button:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  button.active {
    background: var(--burgundy);
    border-color: var(--burgundy-soft);
    color: var(--text-primary);
  }
</style>
