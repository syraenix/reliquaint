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
    background: #252538;
    border: 1px solid #3a3a55;
    color: #888;
    padding: 5px 14px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.82rem;
    transition: background 0.15s, color 0.15s;
  }

  button:hover {
    background: #2e2e4a;
    color: #ccc;
  }

  button.active {
    background: #3a3a7a;
    border-color: #5555aa;
    color: #c0c8ff;
  }
</style>
