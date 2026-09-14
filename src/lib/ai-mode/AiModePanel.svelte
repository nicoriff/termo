<script lang="ts">
  import { aiStatusStore } from "./ai-status-store.svelte";
  import type { AiSessionStatus } from "./classifier";
  import { formatElapsed } from "./elapsed";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { eventMatchesKeybinding, formatKeybindingLabel, type ShortcutAction } from "$lib/core/shortcuts";
  import { collectSessions } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  const STATUSES: AiSessionStatus[] = ["running", "awaiting", "error", "finished"];
  const LABELS: Record<AiSessionStatus, string> = {
    running: "Corriendo",
    awaiting: "Esperando",
    error: "Error",
    finished: "Listo",
  };
  const SHORTCUT_BY_STATUS: Record<AiSessionStatus, ShortcutAction> = {
    running: "aiRunning",
    awaiting: "aiAwaiting",
    error: "aiError",
    finished: "aiFinished",
  };

  // No CSS custom properties exist for the ANSI palette (theme-loader.ts
  // only exposes chrome colors like --titlebar-bg) — read straight from the
  // active theme instead, same convention AppearanceSection's swatch dots use.
  const colors = $derived(themeStore.current.colors);

  function statusColor(status: AiSessionStatus): string {
    switch (status) {
      case "running":
        return colors.blue;
      case "awaiting":
        return colors.yellow;
      case "error":
        return colors.red;
      case "finished":
        return colors.green;
    }
  }

  let openStatus = $state<AiSessionStatus | null>(null);
  let highlighted = $state(0);
  let now = $state(Date.now());

  // Ticks only while a list is open, so the elapsed times stay live.
  $effect(() => {
    if (!openStatus) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  const sessionsById = $derived(
    new Map((workspaceStore.root ? collectSessions(workspaceStore.root) : []).map((loc) => [loc.session.id, loc.session])),
  );

  const openEntries = $derived(
    openStatus
      ? aiStatusStore.entriesWith(openStatus).flatMap((entry) => {
          const session = sessionsById.get(entry.sessionId);
          return session ? [{ ...entry, title: tabTitleStore.titleFor(session.id, session.title) }] : [];
        })
      : [],
  );

  function toggle(status: AiSessionStatus) {
    openStatus = openStatus === status ? null : status;
    highlighted = 0;
  }

  function pick(sessionId: string) {
    openStatus = null;
    workspaceStore.focusSession(sessionId);
  }

  // Capture phase at window level, like the Ctrl+Alt tile overview: runs
  // before xterm's own key handling, so these keys never reach the shell.
  // Keyboard focus stays in the terminal the whole time — the list is
  // driven from here instead of moving focus into it.
  // Sessions waiting on the user, longest-waiting first; repeated presses
  // walk through them.
  function focusNextPending() {
    const pending = [...aiStatusStore.entriesWith("awaiting"), ...aiStatusStore.entriesWith("error")]
      .map((entry) => entry.sessionId)
      .filter((id) => sessionsById.has(id));
    if (pending.length === 0) return;
    const current = pending.indexOf(workspaceStore.focusedSessionId ?? "");
    workspaceStore.focusSession(pending[(current + 1) % pending.length]);
  }

  function onKeydown(event: KeyboardEvent) {
    if (eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("aiNextPending"))) {
      event.preventDefault();
      event.stopPropagation();
      openStatus = null;
      focusNextPending();
      return;
    }
    const status = STATUSES.find((s) =>
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut(SHORTCUT_BY_STATUS[s])),
    );
    if (status) {
      toggle(status);
    } else if (!openStatus) {
      return;
    } else if (event.key === "Escape") {
      openStatus = null;
    } else if (event.key === "ArrowDown") {
      highlighted = Math.min(highlighted + 1, openEntries.length - 1);
    } else if (event.key === "ArrowUp") {
      highlighted = Math.max(highlighted - 1, 0);
    } else if (event.key === "Enter") {
      const entry = openEntries[highlighted];
      if (entry) pick(entry.sessionId);
      else openStatus = null;
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<svelte:window onkeydowncapture={onKeydown} />

<div class="ai-mode-panel">
  {#each STATUSES as status (status)}
    <div class="ai-mode-slot">
      <button
        class="ai-mode-badge"
        class:open={openStatus === status}
        aria-expanded={openStatus === status}
        title={formatKeybindingLabel(appConfigStore.getEffectiveShortcut(SHORTCUT_BY_STATUS[status]), isMac)}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => toggle(status)}
      >
        <i style="background:{statusColor(status)}"></i>
        {LABELS[status]}
        {aiStatusStore.counts[status]}
      </button>
      {#if openStatus === status}
        <div class="ai-mode-dropdown" role="listbox">
          {#each openEntries as entry, index (entry.sessionId)}
            <!-- mousedown preventDefault: keep keyboard focus in the terminal -->
            <button
              class="ai-mode-item"
              class:highlighted={index === highlighted}
              onmousedown={(e) => e.preventDefault()}
              onmouseenter={() => (highlighted = index)}
              onclick={() => pick(entry.sessionId)}
            >
              <span class="ai-mode-item-title">{entry.title}</span>
              <span class="ai-mode-item-time">hace {formatElapsed(now - entry.since)}</span>
            </button>
          {:else}
            <p class="ai-mode-empty">Ninguna pestaña en este estado</p>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</div>

{#if openStatus}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ai-mode-backdrop" onpointerdown={() => (openStatus = null)}></div>
{/if}

<style>
  .ai-mode-panel {
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    font-size: 12px;
    background: var(--tabbar-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    color: var(--tabbar-fg, #d4d4d4);
  }

  .ai-mode-slot {
    flex: 1;
    display: flex;
    position: relative;
  }

  .ai-mode-slot:not(:last-child) {
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .ai-mode-badge {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 4px 12px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .ai-mode-badge:hover,
  .ai-mode-badge.open {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .ai-mode-badge i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: block;
  }

  /* Above the backdrop (299), which is above every tile. */
  .ai-mode-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    min-width: 240px;
    margin-top: 2px;
    z-index: 300;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    max-height: 280px;
    overflow-y: auto;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .ai-mode-slot:last-child .ai-mode-dropdown {
    left: auto;
  }

  .ai-mode-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .ai-mode-item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .ai-mode-item-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-mode-item-time {
    flex-shrink: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .ai-mode-empty {
    margin: 0;
    padding: 8px;
    font-size: 11px;
    opacity: 0.6;
  }

  .ai-mode-backdrop {
    position: fixed;
    inset: 0;
    z-index: 299;
  }
</style>
