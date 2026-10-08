<script lang="ts" generics="T extends string">
  import { tick, type Snippet } from "svelte";
  import { placePanel, type PanelPlacement } from "$lib/preferences/dropdown-placement";

  /**
   * A combo whose options carry their explanation: each one is a title plus
   * what it means, and the trigger shows the chosen one the same way. Same
   * keyboard as SettingDropdown (arrows open and move, Enter/Space pick,
   * Escape closes, Tab leaves).
   */
  interface RichOption {
    value: T;
    title: string;
    text: string;
    disabled?: boolean;
  }

  let {
    options,
    value,
    label,
    onpick,
    icon,
  }: {
    options: RichOption[];
    value: T;
    /** What is being chosen, for screen readers. */
    label: string;
    onpick: (value: T) => void;
    /** Drawn before each option's text. */
    icon?: Snippet<[T]>;
  } = $props();

  let open = $state(false);
  let highlighted = $state(0);
  let triggerEl: HTMLButtonElement | undefined = $state();
  let panelEl: HTMLDivElement | undefined = $state();
  let placement = $state<PanelPlacement | null>(null);

  const current = $derived(options.find((option) => option.value === value) ?? options[0]);

  /** The panel's own cap, and roughly what one row takes — to know whether
   * it fits below before it's drawn. */
  const MAX_PANEL_HEIGHT = 340;
  const ROW_HEIGHT = 58;

  function toggle() {
    open = !open;
    if (!open) return;
    highlighted = Math.max(
      0,
      options.findIndex((option) => option.value === value),
    );
    if (triggerEl) {
      const wanted = Math.min(MAX_PANEL_HEIGHT, options.length * ROW_HEIGHT + 8);
      placement = placePanel(triggerEl.getBoundingClientRect(), window.innerHeight, wanted);
    }
    void tick().then(() => scrollToRow(highlighted));
  }

  function scrollToRow(index: number) {
    panelEl?.children[index]?.scrollIntoView({ block: "nearest" });
  }

  // Placed against the window: once the page scrolls or the window resizes
  // it would float away from its trigger, so it closes instead.
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

  function pick(option: RichOption) {
    if (option.disabled) return;
    if (option.value !== value) onpick(option.value);
    close();
  }

  /** The next option that can be picked, `step` rows away and beyond. */
  function move(step: 1 | -1) {
    let index = highlighted;
    for (let i = 0; i < options.length; i++) {
      index = (index + step + options.length) % options.length;
      if (!options[index].disabled) break;
    }
    highlighted = index;
    scrollToRow(index);
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
      // Closes the list, not the dialog around it.
      event.stopPropagation();
      close();
    } else if (event.key === "ArrowDown") {
      move(1);
    } else if (event.key === "ArrowUp") {
      move(-1);
    } else if (event.key === "Enter" || event.key === " ") {
      if (options[highlighted]) pick(options[highlighted]);
    } else if (event.key === "Tab") {
      open = false;
      return;
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

{#snippet body(option: RichOption)}
  {#if icon}<span class="rich-icon">{@render icon(option.value)}</span>{/if}
  <span class="rich-text">
    <span class="rich-title">{option.title}</span>
    <span class="rich-detail">{option.text}</span>
  </span>
{/snippet}

<div class="rich-select">
  <button
    bind:this={triggerEl}
    class="rich-trigger"
    class:open
    onclick={toggle}
    onkeydown={onKeydown}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
  >
    {#if current}{@render body(current)}{/if}
    <svg class="rich-chevron" viewBox="0 0 10 6" width="10" height="6" aria-hidden="true">
      <path d="M0 0l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
  </button>

  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="rich-backdrop" onpointerdown={() => (open = false)}></div>
    <div
      class="rich-panel"
      role="listbox"
      aria-label={label}
      bind:this={panelEl}
      style:left={placement ? `${placement.left}px` : undefined}
      style:width={placement ? `${placement.width}px` : undefined}
      style:top={placement?.top !== undefined ? `${placement.top}px` : undefined}
      style:bottom={placement?.bottom !== undefined ? `${placement.bottom}px` : undefined}
      style:max-height={placement ? `${placement.maxHeight}px` : undefined}
    >
      {#each options as option, index (option.value)}
        <button
          class="rich-item"
          class:selected={option.value === value}
          class:highlighted={index === highlighted}
          role="option"
          aria-selected={option.value === value}
          disabled={option.disabled}
          tabindex="-1"
          onmouseenter={() => !option.disabled && (highlighted = index)}
          onclick={() => pick(option)}
        >
          {@render body(option)}
          {#if option.value === value}
            <svg class="rich-check" viewBox="0 0 12 10" width="12" height="10" aria-hidden="true">
              <path d="M1 5l3.5 3.5L11 1" fill="none" stroke="currentColor" stroke-width="1.4" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .rich-select {
    position: relative;
  }

  .rich-trigger,
  .rich-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    box-sizing: border-box;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .rich-trigger {
    padding: 9px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
  }

  .rich-trigger:hover,
  .rich-trigger.open,
  .rich-trigger:focus-visible {
    border-color: var(--pane-active-border, #3a6df0);
    outline: none;
  }

  .rich-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    flex-shrink: 0;
    font-size: 16px;
  }

  .rich-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rich-title {
    font-weight: 600;
  }

  .rich-detail {
    font-size: 11.5px;
    line-height: 1.45;
    opacity: 0.7;
  }

  .rich-chevron {
    flex-shrink: 0;
    opacity: 0.7;
    transition: transform 120ms ease;
  }

  .rich-trigger.open .rich-chevron {
    transform: rotate(180deg);
  }

  .rich-backdrop {
    position: fixed;
    inset: 0;
    z-index: 499;
  }

  /* Fixed, placed by placePanel: the scrolling step can't clip it, and it
     opens upwards when the trigger is near the window's bottom. */
  .rich-panel {
    position: fixed;
    z-index: 500;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    box-sizing: border-box;
    overflow-y: auto;
    scrollbar-width: thin;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12.5px;
  }

  .rich-item {
    flex-shrink: 0;
    padding: 8px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
  }

  .rich-item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .rich-item.selected {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .rich-item:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .rich-check {
    flex-shrink: 0;
    color: var(--pane-active-border, #3a6df0);
  }
</style>
