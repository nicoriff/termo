<script lang="ts">
  import { onMount } from "svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { installGoogleFont, injectPreviewStylesheet, listMonospaceGoogleFonts } from "$lib/theme/google-fonts";

  let googleFonts = $state<string[]>([]);
  let fontsLoadFailed = $state(false);
  let open = $state(false);
  let query = $state("");
  let previewsRequested = false;
  let installState = $state<"idle" | "installing" | "error">("idle");
  let installError = $state("");
  let searchInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    listMonospaceGoogleFonts()
      .then((list) => (googleFonts = list))
      .catch(() => (fontsLoadFailed = true));
  });

  const filtered = $derived(
    query.trim() ? googleFonts.filter((f) => f.toLowerCase().includes(query.trim().toLowerCase())) : googleFonts,
  );

  $effect(() => {
    if (open && searchInput) searchInput.focus();
  });

  function toggleOpen() {
    if (installState === "installing") return;
    open = !open;
    if (open && !previewsRequested && googleFonts.length > 0) {
      injectPreviewStylesheet(googleFonts);
      previewsRequested = true;
    }
  }

  function closeDropdown() {
    open = false;
    query = "";
  }

  async function pickFont(family: string) {
    closeDropdown();
    installState = "installing";
    installError = "";
    try {
      const localPath = await installGoogleFont(family);
      themeStore.setFontFamily(family);
      appConfigStore.setInstalledFont(localPath);
      installState = "idle";
    } catch (err) {
      installState = "error";
      installError = err instanceof Error ? err.message : String(err);
    }
  }

  function resetFont() {
    themeStore.setFontFamily(null);
    appConfigStore.setInstalledFont(null);
    installState = "idle";
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeDropdown();
    else if (event.key === "Enter" && filtered.length > 0) void pickFont(filtered[0]);
  }
</script>

<div class="font-picker">
  <button
    class="font-picker-trigger"
    onclick={toggleOpen}
    disabled={installState === "installing"}
    aria-haspopup="listbox"
    aria-expanded={open}
  >
    <span class="font-picker-value" style="font-family: '{themeStore.effectiveFontFamily}'">
      {themeStore.effectiveFontFamily}
    </span>
    <svg viewBox="0 0 10 6" width="9" height="6">
      <path d="M0 0l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
  </button>
  {#if themeStore.fontFamilyOverride !== null}
    <button class="reset-btn" onclick={resetFont}>restablecer</button>
  {/if}

  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="font-picker-backdrop" onpointerdown={closeDropdown}></div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="font-picker-panel" onpointerdown={(e) => e.stopPropagation()}>
      <input
        class="font-picker-search"
        placeholder="Buscar fuente..."
        bind:value={query}
        bind:this={searchInput}
        onkeydown={onSearchKeydown}
      />
      <div class="font-picker-list" role="listbox">
        {#each filtered as family (family)}
          <button
            class="font-picker-item"
            class:selected={themeStore.effectiveFontFamily === family}
            style="font-family: '{family}'"
            onclick={() => pickFont(family)}
          >
            {family}
          </button>
        {/each}
        {#if filtered.length === 0}
          <p class="font-picker-empty">Sin resultados</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if installState === "installing"}
  <p class="font-status">Instalando fuente...</p>
{:else if installState === "error"}
  <p class="font-status error">No se pudo instalar: {installError}</p>
{:else if fontsLoadFailed}
  <p class="font-status error">No se pudo obtener el listado de Google Fonts</p>
{/if}

<style>
  .font-picker {
    display: flex;
    align-items: center;
    gap: 6px;
    position: relative;
  }

  .font-picker-trigger {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 12px;
    padding: 6px 8px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
    text-align: left;
  }

  .font-picker-trigger:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .font-picker-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .font-picker-trigger svg {
    flex-shrink: 0;
    opacity: 0.7;
  }

  .font-picker-backdrop {
    position: fixed;
    inset: 0;
    z-index: 199;
  }

  .font-picker-panel {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 4px;
    z-index: 200;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    max-height: 280px;
    overflow: hidden;
  }

  .font-picker-search {
    margin: 6px;
    padding: 6px 8px;
    font-size: 12px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
  }

  .font-picker-search:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .font-picker-list {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
  }

  .font-picker-item {
    display: flex;
    align-items: center;
    text-align: left;
    padding: 0 8px;
    height: 34px;
    min-height: 34px;
    line-height: normal;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 14px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .font-picker-item:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .font-picker-item.selected {
    color: var(--pane-active-border, #3a6df0);
  }

  .font-picker-empty {
    padding: 10px 8px;
    font-size: 12px;
    opacity: 0.6;
    margin: 0;
  }

  .font-status {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .font-status.error {
    color: #f14c4c;
    opacity: 0.9;
  }
</style>
