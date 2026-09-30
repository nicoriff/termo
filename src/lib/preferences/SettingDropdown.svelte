<script lang="ts">
  import { tick } from "svelte";
  import { placePanel, type PanelPlacement } from "./dropdown-placement";

  /**
   * The combo used by Preferences rows. A native <select> can't be styled to
   * match the rest of the app, so this reproduces one: same look as
   * FontPicker's trigger and panel, and the keyboard behaviour a select has
   * (arrows open and move, Enter/Space pick, Escape closes, Tab leaves).
   */
  interface DropdownOption {
    value: string;
    label: string;
  }

  let {
    options,
    value,
    onpick,
    onopen,
  }: {
    options: DropdownOption[];
    value: string;
    onpick: (value: string) => void;
    /** Called when the panel opens — for lists that may have changed since. */
    onopen?: () => void;
  } = $props();

  let open = $state(false);
  let highlighted = $state(0);
  let triggerEl: HTMLButtonElement | undefined = $state();
  let panelEl: HTMLDivElement | undefined = $state();
  let placement = $state<PanelPlacement | null>(null);

  const current = $derived(options.find((option) => option.value === value) ?? options[0]);

  /** The panel's own cap, and roughly what one row takes (padding, text,
   * the gap between rows) — to know whether it fits below before it's drawn. */
  const MAX_PANEL_HEIGHT = 240;
  const ROW_HEIGHT = 31;

  function toggle() {
    open = !open;
    if (open) {
      onopen?.();
      highlighted = Math.max(
        0,
        options.findIndex((option) => option.value === value),
      );
      if (triggerEl) {
        const wanted = Math.min(MAX_PANEL_HEIGHT, options.length * ROW_HEIGHT + 8);
        placement = placePanel(triggerEl.getBoundingClientRect(), window.innerHeight, wanted);
      }
      // The current pick in view, not scrolled off the bottom of a long list.
      void tick().then(() => scrollToRow(highlighted));
    }
  }

  function scrollToRow(index: number) {
    panelEl?.children[index]?.scrollIntoView({ block: "nearest" });
  }

  // The panel is placed against the window, not the page: once the page
  // scrolls or the window resizes it would float away from its trigger, so
  // it closes instead — as a native select does. Scrolling the panel's own
  // list doesn't count.
  $effect(() => {
    if (!open) return;
    const dismiss = (event: Event) => {
      if (event.type === "scroll" && panelEl?.contains(event.target as Node)) return;
      open = false;
    };
    window.addEventListener("scroll", dismiss, true);
    window.addEventListener("resize", dismiss);
    return () => {
      window.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("resize", dismiss);
    };
  });

  function close() {
    open = false;
    triggerEl?.focus();
  }

  function pick(next: string) {
    if (next !== value) onpick(next);
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
      highlighted = (highlighted + 1) % options.length;
      scrollToRow(highlighted);
    } else if (event.key === "ArrowUp") {
      highlighted = (highlighted - 1 + options.length) % options.length;
      scrollToRow(highlighted);
    } else if (event.key === "Enter" || event.key === " ") {
      if (options[highlighted]) pick(options[highlighted].value);
    } else if (event.key === "Tab") {
      open = false;
      return;
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

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
    <span>{current?.label ?? ""}</span>
    <svg viewBox="0 0 10 6" width="9" height="6">
      <path d="M0 0l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
  </button>

  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dropdown-backdrop" onpointerdown={() => (open = false)}></div>
    <div
      class="dropdown-panel"
      role="listbox"
      bind:this={panelEl}
      style:left={placement ? `${placement.left}px` : undefined}
      style:width={placement ? `${placement.width}px` : undefined}
      style:top={placement?.top !== undefined ? `${placement.top}px` : undefined}
      style:bottom={placement?.bottom !== undefined ? `${placement.bottom}px` : undefined}
      style:max-height={placement ? `${placement.maxHeight}px` : undefined}
    >
      {#each options as option, index (option.value)}
        <button
          class="dropdown-item"
          class:selected={option.value === value}
          class:highlighted={index === highlighted}
          role="option"
          aria-selected={option.value === value}
          onmouseenter={() => (highlighted = index)}
          onclick={() => pick(option.value)}
        >
          <span>{option.label}</span>
          {#if option.value === value}
            <svg viewBox="0 0 12 10" width="11" height="9">
              <path d="M1 5l3.5 3.5L11 1" fill="none" stroke="currentColor" stroke-width="1.4" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dropdown {
    position: relative;
    width: 180px;
    flex-shrink: 0;
  }

  /* Same look as FontPicker's trigger and panel, so every combo matches. */
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

  .dropdown-trigger span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  /* Fixed, placed by placePanel: a scrolling section can't clip it, and it
     opens upwards when the trigger is near the window's bottom. */
  .dropdown-panel {
    position: fixed;
    z-index: 200;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    overflow-y: auto;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .dropdown-item {
    flex-shrink: 0;
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

  .dropdown-item span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dropdown-item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dropdown-item.selected {
    color: var(--pane-active-border, #3a6df0);
  }
</style>
