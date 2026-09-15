<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t, type LanguagePreference } from "$lib/i18n/i18n.svelte";

  // "Español"/"English" stay in their own language so they're recognizable
  // whichever UI language is active.
  const OPTIONS: { value: LanguagePreference; label: () => string }[] = [
    { value: "system", label: () => t("preferences.general.system") },
    { value: "es", label: () => "Español" },
    { value: "en", label: () => "English" },
  ];

  let open = $state(false);
  let highlighted = $state(0);
  let triggerEl: HTMLButtonElement | undefined = $state();

  const current = $derived(OPTIONS.find((option) => option.value === appConfigStore.language) ?? OPTIONS[0]);

  function toggle() {
    open = !open;
    if (open) highlighted = Math.max(0, OPTIONS.indexOf(current));
  }

  function close() {
    open = false;
    triggerEl?.focus();
  }

  function pick(value: LanguagePreference) {
    if (value !== appConfigStore.language) appConfigStore.setLanguage(value);
    close();
  }

  function onKeydown(event: KeyboardEvent) {
    if (!open) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        toggle();
      }
      return;
    }
    if (event.key === "Escape") {
      close();
    } else if (event.key === "ArrowDown") {
      highlighted = (highlighted + 1) % OPTIONS.length;
    } else if (event.key === "ArrowUp") {
      highlighted = (highlighted - 1 + OPTIONS.length) % OPTIONS.length;
    } else if (event.key === "Enter" || event.key === " ") {
      pick(OPTIONS[highlighted].value);
    } else if (event.key === "Tab") {
      open = false;
      return;
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<section>
  <h3>{t("preferences.general.language")}</h3>
  <div class="row">
    <span>{t("preferences.general.languageLabel")}</span>
    <div class="dropdown">
      <button
        bind:this={triggerEl}
        class="dropdown-trigger"
        class:open
        onclick={toggle}
        onkeydown={onKeydown}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span>{current.label()}</span>
        <svg viewBox="0 0 10 6" width="9" height="6">
          <path d="M0 0l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>

      {#if open}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="dropdown-backdrop" onpointerdown={() => (open = false)}></div>
        <div class="dropdown-panel" role="listbox">
          {#each OPTIONS as option, index (option.value)}
            <button
              class="dropdown-item"
              class:selected={option.value === current.value}
              class:highlighted={index === highlighted}
              role="option"
              aria-selected={option.value === current.value}
              onmouseenter={() => (highlighted = index)}
              onclick={() => pick(option.value)}
            >
              <span>{option.label()}</span>
              {#if option.value === current.value}
                <svg viewBox="0 0 12 10" width="11" height="9">
                  <path d="M1 5l3.5 3.5L11 1" fill="none" stroke="currentColor" stroke-width="1.4" />
                </svg>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
  <p class="setting-hint">{t("preferences.general.languageHint")}</p>
</section>

<style>
  section {
    margin-bottom: 20px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  .dropdown {
    position: relative;
    width: 180px;
    flex-shrink: 0;
  }

  /* Same look as FontPicker's trigger and panel, so both combos match. */
  .dropdown-trigger {
    width: 100%;
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

  .dropdown-trigger:hover,
  .dropdown-trigger.open,
  .dropdown-trigger:focus-visible {
    border-color: var(--pane-active-border, #3a6df0);
    outline: none;
  }

  .dropdown-trigger svg {
    flex-shrink: 0;
    opacity: 0.7;
    transition: transform 120ms ease;
  }

  .dropdown-trigger.open svg {
    transform: rotate(180deg);
  }

  .dropdown-backdrop {
    position: fixed;
    inset: 0;
    z-index: 199;
  }

  .dropdown-panel {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 4px;
    z-index: 200;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .dropdown-item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dropdown-item.selected {
    color: var(--pane-active-border, #3a6df0);
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }
</style>
