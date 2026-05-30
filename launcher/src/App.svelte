<script>
    import { onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import iconSrc from "./assets/icon.png";
    import FilterBar from "./components/FilterBar.svelte";
    import GameGrid from "./components/GameGrid.svelte";
    import GameDetail from "./components/GameDetail.svelte";
    import DoctorPanel from "./components/DoctorPanel.svelte";
    import AddGameWizard from "./components/AddGameWizard.svelte";
    import FirstRunPrompt from "./components/FirstRunPrompt.svelte";
    import TapManager from "./components/TapManager.svelte";

    let catalog = [];
    let filter = "all";
    let tapFilter = "all";
    let selectedId = null;
    // When viewing a conflict alternate, pins the specific tap to show.
    let selectedTapId = null;
    let loading = true;
    let error = null;
    let doctorOpen = false;
    let tapsOpen = false;
    let addOpen = false;
    let showFirstRun = false;
    // True when at least one non-local tap is subscribed.
    let hasSubs = false;
    let addingCore = false;
    let addCoreError = null;

    const FIRST_RUN_DISMISSED_KEY = "reliquaint:first-run-dismissed";
    let firstRunDismissed = (() => {
        try {
            return localStorage.getItem(FIRST_RUN_DISMISSED_KEY) === "1";
        } catch {
            return false;
        }
    })();

    function toggleDoctor() {
        doctorOpen = !doctorOpen;
        if (doctorOpen) tapsOpen = false;
    }

    function toggleTaps() {
        tapsOpen = !tapsOpen;
        if (tapsOpen) doctorOpen = false;
    }

    function openAdd() {
        addOpen = true;
    }

    function closeAdd() {
        addOpen = false;
    }

    function onWizardSaved(e) {
        addOpen = false;
        selectedId = e.detail?.id ?? null;
        loadCatalog();
    }

    async function loadCatalog() {
        loading = true;
        error = null;
        try {
            catalog = await invoke("list_catalog");
            // With no subscribed (non-local) tap there is no catalog source beyond
            // the user's own games, so guide the user to subscribe — regardless of
            // whether local entries exist. The welcome screen takes over until the
            // user dismisses it; after that a banner keeps the suggestion visible.
            const taps = await invoke("list_taps");
            hasSubs = taps.some((t) => !t.is_local);
            showFirstRun = !hasSubs && !firstRunDismissed;
        } catch (e) {
            error = String(e);
        } finally {
            loading = false;
        }
    }

    // Subscribe to the official core tap (shared by the welcome screen's button
    // and the no-subscriptions banner).
    async function addCore() {
        addingCore = true;
        addCoreError = null;
        try {
            await invoke("add_tap", {
                nameOrUrl: "reliquaint-core",
                priority: null,
            });
            await loadCatalog();
        } catch (e) {
            addCoreError = String(e);
        } finally {
            addingCore = false;
        }
    }

    function onFirstRunSubscribed() {
        showFirstRun = false;
        loadCatalog();
    }

    function onFirstRunDismissed() {
        showFirstRun = false;
        firstRunDismissed = true;
        try {
            localStorage.setItem(FIRST_RUN_DISMISSED_KEY, "1");
        } catch {
            /* storage unavailable — banner still shows this session */
        }
    }

    function onTapChanged() {
        loadCatalog();
    }

    onMount(() => {
        loadCatalog();
    });

    $: availableTaps = [...new Set(catalog.map((g) => g.tap_id))];
    $: filtered = (() => {
        const base = catalog.filter(
            (g) =>
                (filter === "all" || g.platform === filter) &&
                (tapFilter === "all" || g.tap_id === tapFilter),
        );
        // Viewing a single tap: every id is unique within it. Viewing "all
        // taps": collapse conflicts to the priority winner (catalog arrives
        // sorted by priority, so the first occurrence per id wins).
        if (tapFilter !== "all") return base;
        const seen = new Set();
        return base.filter((g) =>
            seen.has(g.id) ? false : (seen.add(g.id), true),
        );
    })();
    $: selectedGame = selectedId
        ? ((selectedTapId
              ? catalog.find(
                    (g) => g.id === selectedId && g.tap_id === selectedTapId,
                )
              : null) ??
          catalog.find((g) => g.id === selectedId) ??
          null)
        : null;
    $: alternates = selectedGame
        ? catalog.filter(
              (g) =>
                  g.id === selectedGame.id && g.tap_id !== selectedGame.tap_id,
          )
        : [];

    function selectGame(detail) {
        selectedId = detail.id;
        selectedTapId = detail.tap_id ?? null;
    }

    function viewAlternate(tapId) {
        selectedTapId = tapId;
    }

    function onMadeDefault() {
        // Priorities changed; reload and snap back to the (new) winner.
        selectedTapId = null;
        loadCatalog();
    }
</script>

<div class="app">
    <header>
        <div class="app-title">
            <img src={iconSrc} alt="" aria-hidden="true" class="app-icon" />
            <h1>Reliquaint</h1>
        </div>
        <div class="header-actions">
            <FilterBar bind:filter bind:tapFilter {availableTaps} />
            <button
                class="header-btn"
                on:click={loadCatalog}
                title="Refresh catalog"
            >
                ↻
            </button>
            <button
                class="header-btn"
                on:click={openAdd}
                title="Add a game you own"
            >
                + Add game
            </button>
            <button class="header-btn" on:click={toggleTaps}>
                {tapsOpen ? "✕ Taps" : "⊞ Taps"}
            </button>
            <button class="header-btn" on:click={toggleDoctor}>
                {doctorOpen ? "✕ Doctor" : "⚕ Doctor"}
            </button>
        </div>
    </header>

    {#if addOpen}
        <AddGameWizard on:close={closeAdd} on:saved={onWizardSaved} />
    {/if}

    {#if tapsOpen}
        <TapManager on:changed={onTapChanged} />
    {:else if doctorOpen}
        <DoctorPanel />
    {:else if showFirstRun}
        <FirstRunPrompt
            on:subscribed={onFirstRunSubscribed}
            on:dismiss={onFirstRunDismissed}
        />
    {:else if selectedGame}
        <GameDetail
            game={selectedGame}
            {alternates}
            on:back={() => {
                selectedId = null;
                selectedTapId = null;
            }}
            on:installed={loadCatalog}
            on:viewAlternate={(e) => viewAlternate(e.detail.tap_id)}
            on:madeDefault={onMadeDefault}
        />
    {:else if loading}
        <div class="status">Loading catalog…</div>
    {:else if error}
        <div class="status error">Error loading catalog: {error}</div>
    {:else}
        {#if !hasSubs}
            <div class="subscribe-banner">
                <span class="banner-text">
                    No taps subscribed — add the official <strong
                        >Reliquaint Core</strong
                    >
                    tap to get the starter catalog.
                </span>
                <span class="banner-actions">
                    <button
                        class="banner-btn primary"
                        on:click={addCore}
                        disabled={addingCore}
                    >
                        {addingCore ? "Adding…" : "Add Reliquaint Core"}
                    </button>
                    <button class="banner-btn" on:click={toggleTaps}
                        >Manage taps</button
                    >
                </span>
            </div>
            {#if addCoreError}
                <div class="banner-error">{addCoreError}</div>
            {/if}
        {/if}
        {#if catalog.length === 0}
            <div class="status">
                Catalog is empty.
                <br />
                <small
                    >Subscribe to a tap to get started — click <strong
                        >⊞ Taps</strong
                    > above.</small
                >
            </div>
        {:else}
            <GameGrid
                games={filtered}
                on:select={(e) => selectGame(e.detail)}
            />
        {/if}
    {/if}
</div>

<style>
    .app {
        display: flex;
        flex-direction: column;
        height: 100vh;
        overflow: hidden;
        background: var(--bg-base);
    }

    .subscribe-banner {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        flex-wrap: wrap;
        padding: 10px 20px;
        background: var(--bg-elevated);
        border-bottom: 1px solid var(--border-light);
        flex-shrink: 0;
    }

    .banner-text {
        color: var(--text-secondary);
        font-size: 0.88rem;
    }

    .banner-text strong {
        color: var(--gold);
    }

    .banner-actions {
        display: flex;
        gap: 8px;
        flex-shrink: 0;
    }

    .banner-btn {
        background: var(--bg-elevated);
        border: 1px solid var(--border-light);
        color: var(--text-secondary);
        padding: 6px 14px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 0.82rem;
    }

    .banner-btn:hover:not(:disabled) {
        background: var(--bg-hover);
        color: var(--text-secondary);
    }

    .banner-btn.primary {
        background: var(--gold);
        color: var(--bg-deep);
        font-weight: 600;
        border-color: var(--gold);
    }

    .banner-btn.primary:hover:not(:disabled) {
        opacity: 0.9;
        color: var(--text-secondary);
    }

    .banner-btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .banner-error {
        padding: 8px 20px;
        color: var(--status-error-ink);
        font-size: 0.82rem;
        background: var(--bg-elevated);
        border-bottom: 1px solid var(--border-light);
        flex-shrink: 0;
    }

    header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 12px 20px;
        background: var(--bg-deep);
        border-bottom: 1px solid var(--border);
        flex-shrink: 0;
    }

    .app-title {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .app-icon {
        width: 24px;
        height: 24px;
    }

    h1 {
        font-size: 1.2rem;
        font-weight: 600;
        color: var(--gold);
        letter-spacing: 0.03em;
    }

    .header-actions {
        display: flex;
        align-items: center;
        gap: 12px;
    }

    .header-btn {
        background: var(--bg-elevated);
        border: 1px solid var(--border-light);
        color: var(--gold);
        padding: 6px 14px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 0.85rem;
    }

    .header-btn:hover {
        background: var(--bg-hover);
    }

    .status {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        color: var(--ink-secondary);
        font-size: 1rem;
        text-align: center;
        padding: 20px;
    }

    .status small {
        color: var(--ink-muted);
        font-size: 0.85rem;
        margin-top: 8px;
    }

    .error {
        color: var(--status-error-ink);
    }
</style>
