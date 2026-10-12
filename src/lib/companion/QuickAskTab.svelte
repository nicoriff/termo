<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { quickAskStore } from "./quick-ask-store.svelte";

  const isMac = navigator.platform.toLowerCase().includes("mac");

  const available = $derived(appConfigStore.aiModeEnabled && appConfigStore.companionEnabled && appConfigStore.quickAskEnabled);
  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("quickAsk"), isMac));
  const label = $derived(t("companion.quick.title"));
</script>

<!-- A tab on the console's right edge, low down: a sliver until the pointer
     comes near, then it slides out with its name. The pointerdown is left to
     reach the console's slot, which takes the focus, so the question is asked
     about this console. -->
{#if available}
  <button
    class="qa-tab"
    class:asking={quickAskStore.asking !== null}
    tabindex="-1"
    title={shortcut ? `${label} (${shortcut})` : label}
    aria-label={label}
    onclick={() => uiState.openQuickAsk()}
  >
    <!-- A bolt: a quick one. -->
    <span class="qa-tab-mark" aria-hidden="true">
      <svg viewBox="0 0 12 12" width="19" height="19">
        <path d="M7.2 0.8 2.4 6.8h3.2l-0.9 4.4 4.9-6.2H6.3z" fill="currentColor" />
      </svg>
    </span>
    <span class="qa-tab-text">{label}</span>
  </button>
{/if}

<style>
  .qa-tab {
    position: absolute;
    right: 0;
    bottom: 44px;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 50px;
    /* At rest only the mark shows. */
    max-width: 26px;
    padding: 0 3px;
    overflow: hidden;
    white-space: nowrap;
    border: 1px solid var(--pane-active-border, #3a6df0);
    border-right: none;
    border-radius: 5px 0 0 5px;
    font: inherit;
    font-size: 12px;
    color: #fff;
    background: var(--pane-active-border, #3a6df0);
    opacity: 0.8;
    box-shadow: -2px 1px 8px rgba(0, 0, 0, 0.3);
    cursor: pointer;
    transition:
      max-width 180ms ease,
      opacity 140ms ease,
      padding 180ms ease,
      box-shadow 180ms ease;
  }

  .qa-tab:hover,
  .qa-tab:focus-visible {
    max-width: 220px;
    padding: 0 12px 0 6px;
    opacity: 1;
    box-shadow: -4px 2px 14px rgba(0, 0, 0, 0.35);
    outline: none;
  }

  .qa-tab:active {
    filter: brightness(0.9);
  }

  .qa-tab-mark {
    flex-shrink: 0;
    width: 19px;
    display: flex;
    align-items: center;
  }

  .qa-tab-text {
    opacity: 0;
    transition: opacity 140ms ease;
  }

  .qa-tab:hover .qa-tab-text,
  .qa-tab:focus-visible .qa-tab-text {
    opacity: 1;
  }

  /* An answer on its way while the dialog is closed. */
  .qa-tab.asking {
    animation: qa-tab-pulse 1.2s ease-in-out infinite;
  }

  @keyframes qa-tab-pulse {
    50% {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .qa-tab,
    .qa-tab-text {
      transition: none;
    }

    .qa-tab.asking {
      animation: none;
      opacity: 1;
    }
  }
</style>
