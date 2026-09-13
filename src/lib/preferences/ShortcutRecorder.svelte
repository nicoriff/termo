<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import {
    findConflict,
    formatKeybindingLabel,
    normalizeEventToBinding,
    normalizeEventToZoomBinding,
    SHORTCUT_LABELS,
    type ShortcutAction,
  } from "$lib/core/shortcuts";

  let { action, mode = "combo" }: { action: ShortcutAction; mode?: "combo" | "zoom" } = $props();

  let recording = $state(false);
  let message = $state<string | null>(null);
  let buttonEl: HTMLButtonElement | undefined = $state();

  const label = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut(action), isMac));
  const hasOverride = $derived(appConfigStore.getShortcutOverride(action) !== null);

  function startRecording() {
    recording = true;
    message = null;
    buttonEl?.focus();
  }

  function stopRecording() {
    recording = false;
  }

  function apply(binding: string) {
    const conflict = findConflict(binding, action, appConfigStore.allShortcutOverrides, isMac);
    if (conflict) {
      message = `Ya lo usa "${SHORTCUT_LABELS[conflict]}" — elegí otra combinación.`;
      recording = false;
      return;
    }
    appConfigStore.setShortcutOverride(action, binding);
    recording = false;
    message = null;
  }

  function onKeydown(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      stopRecording();
      return;
    }

    if (mode === "zoom") {
      const result = normalizeEventToZoomBinding(event);
      if ("error" in result) {
        message = result.error;
        return;
      }
      apply(result.binding);
      return;
    }

    const binding = normalizeEventToBinding(event);
    if (binding === null) return;
    apply(binding);
  }

  function resetToDefault() {
    appConfigStore.setShortcutOverride(action, null);
    message = null;
  }
</script>

<div class="recorder">
  <button
    bind:this={buttonEl}
    class="recorder-btn"
    class:recording
    onclick={startRecording}
    onkeydown={onKeydown}
    onblur={stopRecording}
  >
    {recording ? "Presioná una combinación..." : label}
  </button>
  {#if hasOverride}
    <button class="reset-btn" onclick={resetToDefault}>restablecer</button>
  {/if}
</div>
{#if message}
  <p class="recorder-message">{message}</p>
{/if}

<style>
  .recorder {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .recorder-btn {
    min-width: 120px;
    font-size: 12px;
    padding: 6px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
    text-align: center;
  }

  .recorder-btn:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .recorder-btn.recording {
    border-color: var(--pane-active-border, #3a6df0);
    color: var(--pane-active-border, #3a6df0);
  }

  .reset-btn {
    width: auto !important;
    padding: 0 6px;
    font-size: 10px;
    opacity: 0.75;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
  }

  .recorder-message {
    margin: 6px 0 0;
    font-size: 10px;
    color: #f14c4c;
    opacity: 0.9;
  }
</style>
