<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { buildCommands } from "./command-registry.svelte";
  import { matchCommands, type Command, type CommandGroup } from "./commands";

  const GROUP_LABEL_KEYS: Record<CommandGroup, MessageKey> = {
    navigation: "commands.groups.navigation",
    tabs: "commands.groups.tabs",
    layouts: "commands.groups.layouts",
    appearance: "commands.groups.appearance",
    git: "commands.groups.git",
    aiMode: "commands.groups.aiMode",
    agents: "commands.groups.agents",
    extensions: "commands.groups.extensions",
  };

  let query = $state("");
  let highlighted = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  // Nothing is listed until the user types: the palette suggests as you go
  // instead of dumping every command the moment it opens.
  const results = $derived(query.trim() === "" ? [] : matchCommands(buildCommands(), query));

  $effect(() => {
    inputEl?.focus();
  });

  // Typing restarts the selection at the top, so Enter always runs the best
  // match rather than whatever row the previous query left highlighted.
  $effect(() => {
    void query;
    highlighted = 0;
    if (listEl) listEl.scrollTop = 0;
  });

  // The arrow keys move the highlight, not the viewport: bring the row into
  // view ourselves. Only for the keyboard — with the mouse the row is already
  // under the pointer, and scrolling would slide the list away from it.
  function reveal() {
    listEl?.querySelector(`[data-index="${highlighted}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function close() {
    uiState.closeCommandPalette();
    // The input took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  function run(command: Command) {
    close();
    command.run();
  }

  function onKeydown(event: KeyboardEvent) {
    // The palette owns the keyboard while it's open (MainWindow's window-level
    // handler bails out), so nothing here should reach the terminal either.
    event.stopPropagation();
    if (event.key === "Tab") {
      // A modal keeps focus inside it. The input is the only thing to focus
      // here (the options are driven from it), so Tab stays put instead of
      // wandering off to the app behind the backdrop.
      event.preventDefault();
      return;
    }
    if (event.key === "Escape") {
      close();
    } else if (event.key === "ArrowDown") {
      highlighted = results.length === 0 ? 0 : (highlighted + 1) % results.length;
      reveal();
    } else if (event.key === "ArrowUp") {
      highlighted = results.length === 0 ? 0 : (highlighted - 1 + results.length) % results.length;
      reveal();
    } else if (event.key === "Enter") {
      const command = results[highlighted];
      if (command) run(command);
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="palette-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="palette-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette" onpointerdown={(e) => e.stopPropagation()} transition:fly={{ y: -16, duration: 140 }}>
    <input
      bind:this={inputEl}
      bind:value={query}
      class="palette-input"
      placeholder={t("commands.placeholder")}
      onkeydown={onKeydown}
      aria-label={t("commands.placeholder")}
      role="combobox"
      aria-expanded={results.length > 0}
      aria-controls="palette-listbox"
      aria-activedescendant={results.length > 0 ? `palette-option-${highlighted}` : undefined}
      aria-autocomplete="list"
    />
    <div class="palette-list" id="palette-listbox" role="listbox" bind:this={listEl}>
      {#each results as command, index (command.id)}
        {#if index === 0 || results[index - 1].group !== command.group}
          <div class="palette-group" role="presentation">{t(GROUP_LABEL_KEYS[command.group])}</div>
        {/if}
        <button
          class="palette-item"
          class:highlighted={index === highlighted}
          class:opens-group={index === 0 || results[index - 1].group !== command.group}
          id="palette-option-{index}"
          role="option"
          tabindex="-1"
          data-index={index}
          aria-selected={index === highlighted}
          onpointermove={() => (highlighted = index)}
          onclick={() => run(command)}
        >
          <span class="palette-label">{command.label}</span>
          {#if command.shortcut}
            <kbd class="palette-shortcut">
              {formatKeybindingLabel(appConfigStore.getEffectiveShortcut(command.shortcut), isMac)}
            </kbd>
          {/if}
        </button>
      {:else}
        <p class="palette-empty">{query.trim() === "" ? t("commands.hint") : t("commands.empty")}</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .palette-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.35);
  }

  /* The panel itself carries the fly transition, which animates transform —
     so the centering lives on this wrapper instead of on the panel. */
  .palette-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .palette {
    pointer-events: auto;
    width: min(560px, 90%);
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }

  .palette-input {
    padding: 10px 12px;
    border: none;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 13px;
  }

  .palette-input:focus {
    outline: none;
  }

  .palette-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    min-height: 0;
    max-height: 320px;
    overflow-y: auto;
  }

  /* Without this, flex shrinks every row to fit max-height instead of
     scrolling, and the whole list renders squashed. */
  .palette-group {
    flex-shrink: 0;
    padding: 6px 8px 2px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .palette-item {
    flex-shrink: 0;
    padding: 7px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .palette-label {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .palette-shortcut {
    flex-shrink: 0;
    font: inherit;
    font-size: 11px;
    opacity: 0.55;
  }

  /* Scrolling to the first row of a group keeps its title in view too. */
  .palette-item.opens-group {
    scroll-margin-top: 28px;
  }

  .palette-item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    color: var(--pane-active-border, #3a6df0);
  }

  .palette-empty {
    margin: 0;
    padding: 10px 8px;
    font-size: 12px;
    opacity: 0.6;
    color: var(--titlebar-fg, #d4d4d4);
  }
</style>
