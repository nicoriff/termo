import { getVersion } from "@tauri-apps/api/app";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { uiState } from "$lib/core/ui-state.svelte";
import changelog from "./changelog.json";
import { compareVersions, entriesBetween, type ChangelogEntry } from "./release-notes";

/**
 * On the first launch after an update, shows what it brought: the changelog
 * entries since the version that ran before. Then remembers this one, so it's
 * shown once. The very first launch (nothing remembered yet) only remembers:
 * someone installing Termo doesn't need to be told what's new in it.
 * Call once the config has loaded.
 */
export async function showWhatsNewOnce(): Promise<void> {
  const current = await getVersion().catch(() => null);
  if (!current) return;
  const previous = appConfigStore.lastSeenVersion;
  if (previous && compareVersions(current, previous) > 0) {
    const entries = entriesBetween(changelog as ChangelogEntry[], previous, current);
    if (entries.length > 0) uiState.showWhatsNew(entries);
  }
  if (previous !== current) appConfigStore.setLastSeenVersion(current);
}
