<script lang="ts">
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { Theme } from "$lib/theme/theme-types";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/translate";
  import ThemeCard from "../ThemeCard.svelte";

  // Save only user edits. An OS change or a config broadcast must not save again.
  function chooseTheme(theme: Theme) {
    if (themeStore.syncWithOs) themeStore.setAppearanceTheme(theme.appearance, theme.name);
    else themeStore.selectTheme(theme.name);
    void appConfigStore.persist();
  }

  function onSyncChange(event: Event) {
    themeStore.setSyncWithOs((event.currentTarget as HTMLInputElement).checked);
    void appConfigStore.persist();
  }

  function isSelected(theme: Theme): boolean {
    if (!themeStore.syncWithOs) return themeStore.manualTheme === theme.name;
    return (theme.appearance === "light" ? themeStore.lightTheme : themeStore.darkTheme) === theme.name;
  }

  /** Which themes the grid shows. "all" shows them all, in groups. */
  type ThemeFilter = "all" | "dark" | "light" | "colorful" | "retro";
  const FILTERS: { value: ThemeFilter; label: MessageKey }[] = [
    { value: "all", label: "preferences.appearance.themeFilterAll" },
    { value: "dark", label: "preferences.appearance.themeFilterDark" },
    { value: "light", label: "preferences.appearance.themeFilterLight" },
    { value: "colorful", label: "preferences.appearance.themeFilterColorful" },
    { value: "retro", label: "preferences.appearance.themeFilterRetro" },
  ];
  let filter = $state<ThemeFilter>("all");

  /** The grid's groups: with every theme, Termo's colorful ones first, then
   * the retro ones, then the classics; filtered, one group of the rest. */
  const groups = $derived.by((): { title: MessageKey | null; themes: Theme[] }[] => {
    const all = themeStore.themes;
    if (filter === "all") {
      const grouped: { title: MessageKey; themes: Theme[] }[] = [
        { title: "preferences.appearance.themeFilterColorful", themes: all.filter((th) => th.kind === "colorful") },
        { title: "preferences.appearance.themeFilterRetro", themes: all.filter((th) => th.kind === "retro") },
        { title: "preferences.appearance.themeGroupClassic", themes: all.filter((th) => !th.kind) },
      ];
      return grouped.filter((group) => group.themes.length > 0);
    }
    const shown =
      filter === "dark" || filter === "light"
        ? all.filter((th) => th.appearance === filter)
        : all.filter((th) => th.kind === filter);
    return [{ title: null, themes: shown }];
  });
</script>

<section>
  <label class="toggle-row">
    <span>{t("preferences.appearance.syncWithOs")}</span>
    <input type="checkbox" checked={themeStore.syncWithOs} onchange={onSyncChange} />
  </label>
  <p class="setting-hint">{t("preferences.appearance.syncWithOsHint")}</p>
  {#if themeStore.syncWithOs}
    <p class="setting-hint">{t("preferences.appearance.activeTheme", { theme: themeStore.current.name })}</p>
  {/if}
</section>

<section>
  <div class="theme-filters" role="radiogroup" aria-label={t("preferences.appearance.themeFilter")}>
    {#each FILTERS as option (option.value)}
      <button
        class="theme-filter"
        class:on={filter === option.value}
        role="radio"
        aria-checked={filter === option.value}
        onclick={() => (filter = option.value)}
      >
        {t(option.label)}
      </button>
    {/each}
  </div>
  {#each groups as group (group.title ?? "all")}
    {#if group.title}
      <h3 class="theme-group-title">{t(group.title)}</h3>
    {/if}
    <div class="theme-grid" role="group" aria-label={group.title ? t(group.title) : t("preferences.appearance.theme")}>
      {#each group.themes as theme (theme.name)}
        <ThemeCard {theme} selected={isSelected(theme)} onpick={() => chooseTheme(theme)} />
      {/each}
    </div>
  {/each}
</section>

<style>
  section {
    margin-bottom: 18px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .theme-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .theme-filter {
    padding: 3px 11px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
    opacity: 0.8;
    transition:
      background 120ms ease,
      border-color 120ms ease,
      opacity 120ms ease;
  }

  .theme-filter:hover {
    opacity: 1;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .theme-filter.on {
    opacity: 1;
    border-color: var(--pane-active-border, #3a6df0);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 22%, transparent);
  }

  .theme-group-title {
    margin: 16px 2px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.55;
  }

  /* Three or more across when there's room, two at least. With Sync with OS
     on, two are marked: the light one and the dark one. */
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 8px;
    margin-top: 8px;
  }
</style>
