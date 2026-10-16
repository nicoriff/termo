<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { remoteConsoles, tailnetMachines, type TailnetMachine } from "$lib/core/remote-client";
  import type { KeptConsole } from "$lib/core/pty-client";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { backgroundLabel, programName } from "$lib/layout/new-tab-options.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";

  /** First the computers on the tailnet, then the consoles of the one picked. */
  let machines = $state<TailnetMachine[] | null>(null);
  let machinesError = $state(false);
  let machine = $state<TailnetMachine | null>(null);
  let consoles = $state<KeptConsole[] | null>(null);
  let consolesError = $state<string | null>(null);
  let dialogEl: HTMLDivElement | undefined = $state();

  onMount(() => {
    dialogEl?.focus();
    tailnetMachines()
      .then((list) => (machines = list))
      .catch(() => (machinesError = true));
  });

  async function pick(picked: TailnetMachine) {
    if (!picked.ip) return;
    machine = picked;
    consoles = null;
    consolesError = null;
    try {
      consoles = await remoteConsoles(picked.ip);
    } catch (err) {
      consolesError = err instanceof Error ? err.message : String(err);
    }
  }

  function open(kept: KeptConsole) {
    const target = uiState.remoteConnect;
    if (!target || !machine?.ip) return;
    workspaceStore.attachTab(target.tileId, kept.id, `${machine.name} · ${backgroundLabel(kept)}`, undefined, {
      ip: machine.ip,
      name: machine.name,
    });
    close();
  }

  function back() {
    machine = null;
    consoles = null;
    consolesError = null;
  }

  function close() {
    uiState.closeRemoteConnect();
    uiState.refocusTerminals();
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's up.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      if (machine) back();
      else close();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("remote.connectTitle")}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    {#if !machine}
      <h2 class="dialog-title">{t("remote.connectTitle")}</h2>
      <p class="dialog-text">{t("remote.machines")}</p>
      {#if machinesError}
        <p class="dialog-text muted">{t("remote.noTailscale")}</p>
      {:else if machines === null}
        <p class="dialog-text muted">{t("remote.looking")}</p>
      {:else if machines.length === 0}
        <p class="dialog-text muted">{t("remote.noMachines")}</p>
      {:else}
        <div class="list">
          {#each machines as entry (entry.dnsName || entry.name)}
            <button class="row" onclick={() => void pick(entry)}>
              <span class="name">{entry.name}</span>
              <span class="detail">{entry.os}{entry.login ? ` · ${entry.login}` : ""}</span>
            </button>
          {/each}
        </div>
      {/if}
    {:else}
      <h2 class="dialog-title">{t("remote.consolesOn", { name: machine.name })}</h2>
      {#if consolesError}
        <p class="dialog-text muted">{t("remote.failed", { name: machine.name, error: consolesError })}</p>
      {:else if consoles === null}
        <p class="dialog-text muted">{t("remote.looking")}</p>
      {:else if consoles.length === 0}
        <p class="dialog-text muted">{t("remote.noConsoles", { name: machine.name })}</p>
      {:else}
        <div class="list">
          {#each consoles as kept (kept.id)}
            <button class="row" title={kept.cwd ?? kept.path} onclick={() => open(kept)}>
              <span class="name"><ConsoleIcon kind={programName(kept.path)} /> {backgroundLabel(kept)}</span>
              {#if kept.cwd}<span class="detail">{kept.cwd}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    {/if}
    <div class="dialog-actions">
      {#if machine}
        <button class="dialog-button" onclick={back}>{t("remote.back")}</button>
      {/if}
      <button class="dialog-button" onclick={close}>{t("remote.close")}</button>
    </div>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.35);
  }

  .dialog-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .dialog {
    pointer-events: auto;
    width: min(460px, 90%);
    margin-top: 12vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .dialog:focus {
    outline: none;
  }

  .dialog-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .dialog-text {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
  }

  .dialog-text.muted {
    opacity: 0.6;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 50vh;
    overflow-y: auto;
  }

  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .row:hover,
  .row:focus-visible {
    background: var(--tabbar-hover-bg, rgba(255, 255, 255, 0.06));
    outline: none;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .detail {
    font-size: 10px;
    opacity: 0.6;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .dialog-button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
</style>
