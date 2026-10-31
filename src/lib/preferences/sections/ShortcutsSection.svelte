<script lang="ts">
  import ShortcutRecorder from "../ShortcutRecorder.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import {
    filterShortcutActions,
    keybindingSearchText,
    SHORTCUT_GROUPS,
    SHORTCUT_LABEL_KEYS,
  } from "$lib/core/shortcuts";
  import { t } from "$lib/i18n/i18n.svelte";

  let query = $state("");

  // Groups left with no matching shortcut disappear, title included.
  const groups = $derived(
    SHORTCUT_GROUPS.map((group) => ({
      ...group,
      actions: filterShortcutActions(
        group.actions,
        query,
        (action) => t(SHORTCUT_LABEL_KEYS[action]),
        (action) => keybindingSearchText(appConfigStore.getEffectiveShortcut(action), isMac),
      ),
    })).filter((group) => group.actions.length > 0),
  );
</script>

<section>
  <h3>{t("preferences.shortcuts.title")}</h3>
  <input
    class="shortcut-search"
    type="search"
    bind:value={query}
    placeholder={t("preferences.shortcuts.search")}
    aria-label={t("preferences.shortcuts.search")}
  />
  {#each groups as group (group.id)}
    <h4>{t(group.titleKey)}</h4>
    {#each group.actions as action (action)}
      <div class="shortcut-row">
        <span>{t(SHORTCUT_LABEL_KEYS[action])}</span>
        <ShortcutRecorder {action} mode={action === "zoomTile" ? "zoom" : "combo"} />
      </div>
    {/each}
  {:else}
    <p class="shortcut-empty">{t("preferences.shortcuts.empty")}</p>
  {/each}
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

  h4 {
    font-size: 11px;
    font-weight: 600;
    opacity: 0.6;
    margin: 14px 0 0;
  }

  .shortcut-search {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    font: inherit;
    font-size: 12px;
    color: inherit;
    background: transparent;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
  }

  .shortcut-search:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .shortcut-empty {
    margin: 12px 0 0;
    font-size: 12px;
    opacity: 0.6;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    padding: 8px 0;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .shortcut-row:last-child {
    border-bottom: none;
  }
</style>
