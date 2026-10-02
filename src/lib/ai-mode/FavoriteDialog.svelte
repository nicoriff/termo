<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { favoritesStore } from "./favorites-store.svelte";
  import { sessionLabel } from "./restore-session";

  /** Adds the conversation in uiState.favoriteDialog to the favorites, or
   * renames (or removes) it when it already is one. The name is optional:
   * without one it shows as recent sessions do, by agent and folder. */
  const favorite = $derived(uiState.favoriteDialog!);
  const existing = $derived(favoritesStore.byId.has(favorite.agentSessionId));
  const fallback = $derived(sessionLabel(favorite));

  let name = $state(uiState.favoriteDialog?.name ?? "");
  let error = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    inputEl?.focus();
    inputEl?.select();
  });

  function close() {
    uiState.closeFavoriteDialog();
    // The input took focus away from the terminal; hand it back — unless it
    // was opened from the favorites manager, which is still up.
    if (!uiState.favoritesManagerOpen) uiState.refocusTerminals();
  }

  async function confirm() {
    try {
      await favoritesStore.save({ ...favorite, name: name.trim() || null });
      close();
    } catch (err) {
      error = String(err);
    }
  }

  async function remove() {
    try {
      await favoritesStore.remove(favorite.agentSessionId);
      close();
    } catch (err) {
      error = String(err);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's open, so nothing reaches the
    // terminal underneath.
    event.stopPropagation();
    if (event.key === "Escape") close();
    else if (event.key === "Enter") void confirm();
    else return;
    event.preventDefault();
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
    aria-label={t(existing ? "favorites.editTitle" : "favorites.addTitle")}
    onpointerdown={(e) => e.stopPropagation()}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">
      <span class="star" aria-hidden="true">★</span>
      {t(existing ? "favorites.editTitle" : "favorites.addTitle")}
    </h2>
    <p class="dialog-session">
      <ConsoleIcon kind={favorite.tool} size={12} />
      <span>{fallback}</span>
    </p>
    <input
      bind:this={inputEl}
      bind:value={name}
      class="dialog-input"
      maxlength="80"
      placeholder={t("favorites.namePlaceholder")}
      aria-label={t("favorites.nameLabel")}
      onkeydown={onKeydown}
    />
    <p class="dialog-hint">{t("favorites.nameHint", { label: fallback })}</p>
    {#if error}
      <p class="dialog-error">{error}</p>
    {/if}
    <div class="dialog-actions">
      {#if existing}
        <button class="dialog-button danger" onclick={remove}>{t("favorites.remove")}</button>
      {/if}
      <span class="spacer"></span>
      <button class="dialog-button" onclick={close}>{t("favorites.cancel")}</button>
      <button class="dialog-button primary" onclick={confirm}>{t("favorites.save")}</button>
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

  /* The panel carries the fly transition, which animates transform — so the
     centering lives on this wrapper instead, same as the command palette. */
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
    width: min(420px, 90%);
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

  .dialog-title {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .star {
    color: #e5c07b;
  }

  .dialog-session {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 12px;
    opacity: 0.8;
  }

  .dialog-input {
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-bg, #181818);
    color: inherit;
    font: inherit;
    font-size: 13px;
  }

  .dialog-input:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .dialog-hint {
    margin: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .dialog-error {
    margin: 0;
    font-size: 11px;
    color: #f14c4c;
  }

  .dialog-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .spacer {
    flex: 1;
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

  .dialog-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .dialog-button.danger:hover {
    border-color: #f14c4c;
    color: #f14c4c;
  }
</style>
