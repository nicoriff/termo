<script lang="ts">
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { GROUP_COLOR_IDS, GROUP_COLORS, type GroupColor } from "./tab-groups";
  import { tabGroupsStore } from "./tab-groups-store.svelte";

  /** A group's bubble, as Chrome's: its name and color, edited as they're
   * typed and picked, and what can be done with the group. Opens at the
   * point it was asked from (the chip, a tab's menu, the panel). */
  const WIDTH = 260;
  const MARGIN = 8;

  const editing = $derived(tabGroupsStore.editing!);
  const group = $derived(tabGroupsStore.byId.get(editing.groupId));
  const open = $derived(group ? tabGroupsStore.isOpen(group.id) : false);

  let name = $state(tabGroupsStore.byId.get(tabGroupsStore.editing?.groupId ?? "")?.name ?? "");
  let inputEl: HTMLInputElement | undefined = $state();
  let panelEl: HTMLDivElement | undefined = $state();
  let top = $state(0);
  const left = $derived(Math.max(MARGIN, Math.min(editing.x, window.innerWidth - WIDTH - MARGIN)));

  $effect(() => {
    inputEl?.focus();
    inputEl?.select();
  });

  // Below the point, or above it when there's no room below.
  $effect(() => {
    const height = panelEl?.offsetHeight ?? 0;
    const below = editing.y + 4;
    top = below + height + MARGIN > window.innerHeight ? Math.max(MARGIN, editing.y - height - 4) : below;
  });

  function close() {
    if (group && name.trim() !== group.name) tabGroupsStore.rename(group.id, name);
    tabGroupsStore.closeEditor();
    uiState.refocusTerminals();
  }

  function pickColor(color: GroupColor) {
    if (group) tabGroupsStore.setColor(group.id, color);
  }

  function act(action: (groupId: string) => void | Promise<void>) {
    if (!group) return;
    const id = group.id;
    if (name.trim() !== group.name) tabGroupsStore.rename(id, name);
    tabGroupsStore.closeEditor();
    void action(id);
  }

  function onKeydown(event: KeyboardEvent) {
    // Owns the keyboard while it's open: nothing reaches the console.
    event.stopPropagation();
    if (event.key === "Escape" || (event.key === "Enter" && event.target === inputEl)) {
      event.preventDefault();
      close();
    }
  }
</script>

{#if group}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="group-editor-backdrop"
    onpointerdown={close}
    oncontextmenu={(e) => {
      e.preventDefault();
      close();
    }}
  ></div>
  <div
    class="group-editor"
    role="dialog"
    tabindex="-1"
    aria-label={t("tabGroups.edit")}
    style="top:{top}px; left:{left}px; width:{WIDTH}px; --group-color:{GROUP_COLORS[group.color]}"
    bind:this={panelEl}
    onkeydown={onKeydown}
    in:fly={{ y: -4, duration: 120, easing: cubicOut }}
  >
    <input
      class="group-editor-name"
      bind:this={inputEl}
      bind:value={name}
      placeholder={t("tabGroups.namePlaceholder")}
      aria-label={t("tabGroups.namePlaceholder")}
      maxlength="60"
      spellcheck="false"
    />
    <div class="group-editor-colors" role="radiogroup" aria-label={t("tabGroups.color")}>
      {#each GROUP_COLOR_IDS as color (color)}
        <button
          class="group-editor-swatch"
          class:selected={group.color === color}
          style="--swatch:{GROUP_COLORS[color]}"
          role="radio"
          aria-checked={group.color === color}
          aria-label={t(`tabGroups.colors.${color}`)}
          title={t(`tabGroups.colors.${color}`)}
          onclick={() => pickColor(color)}
        ></button>
      {/each}
    </div>
    <div class="group-editor-divider"></div>
    <button class="group-editor-action" onclick={() => act((id) => tabGroupsStore.newTabInGroup(id))}>
      {t("tabGroups.newTab")}
    </button>
    {#if open}
      <button class="group-editor-action" onclick={() => act((id) => tabGroupsStore.closeGroup(id))}>
        {t("tabGroups.close")}
      </button>
    {:else}
      <button class="group-editor-action" onclick={() => act((id) => tabGroupsStore.reopen(id))}>
        {t("tabGroups.reopen")}
      </button>
    {/if}
    {#if open}
      <button class="group-editor-action" onclick={() => act((id) => tabGroupsStore.ungroup(id))}>
        {t("tabGroups.ungroup")}
      </button>
    {/if}
    <button class="group-editor-action danger" onclick={() => act((id) => tabGroupsStore.remove(id))}>
      {t("tabGroups.delete")}
    </button>
  </div>
{/if}

<style>
  .group-editor-backdrop {
    position: fixed;
    inset: 0;
    z-index: 299;
  }

  .group-editor {
    position: fixed;
    z-index: 300;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 6px 6px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-top: 3px solid var(--group-color);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--tabbar-active-fg, #f0f0f0);
    font-size: 12px;
  }

  .group-editor-name {
    margin: 0 4px 8px;
    padding: 6px 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: var(--tabbar-bg, #181818);
    color: inherit;
    font: inherit;
    outline: none;
  }

  .group-editor-name:focus {
    border-color: var(--group-color);
  }

  .group-editor-colors {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 4px 6px;
  }

  .group-editor-swatch {
    width: 20px;
    height: 20px;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 50%;
    background: var(--swatch);
    background-clip: content-box;
    cursor: pointer;
  }

  .group-editor-swatch.selected,
  .group-editor-swatch:focus-visible {
    border-color: var(--tabbar-active-fg, #f0f0f0);
    outline: none;
  }

  .group-editor-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .group-editor-action {
    padding: 7px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .group-editor-action:hover,
  .group-editor-action:focus-visible {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    outline: none;
  }

  .group-editor-action.danger {
    color: #e5534b;
  }
</style>
