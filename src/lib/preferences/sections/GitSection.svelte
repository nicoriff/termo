<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore, GIT_STATUS_PARTS } from "$lib/core/app-config.svelte";
  import { gitTools, type GitTool, type GitTools } from "$lib/core/git-client";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleGit"), isMac));
  const off = $derived(!appConfigStore.gitEnabled);

  const fetchOptions = $derived([
    { value: "0", label: t("preferences.git.autoFetchNever") },
    ...[5, 15, 30, 60].map((minutes) => ({ value: String(minutes), label: t("preferences.git.autoFetchEvery", { n: String(minutes) }) })),
  ]);

  /** What's installed of what all this leans on; null while it's asked. */
  let tools = $state<GitTools | null>(null);

  // This window is preloaded once and reused: asked again on every open.
  onMount(() => {
    void gitTools().then((found) => (tools = found));
  });

  const TOOLS = [
    { id: "git", name: "git", use: "preferences.git.tools.gitUse" },
    { id: "gh", name: "gh", use: "preferences.git.tools.ghUse" },
    { id: "glab", name: "glab", use: "preferences.git.tools.glabUse" },
  ] as const;

  function toolState(tool: GitTool): string {
    if (!tool.found) return t("preferences.git.tools.missing");
    if (tool.signedIn === false) return t("preferences.git.tools.signedOut");
    return tool.version ?? t("preferences.git.tools.found");
  }
</script>

<section>
  <h3>{t("preferences.git.title")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.enabled")}</span>
    <input type="checkbox" checked={appConfigStore.gitEnabled} onchange={(e) => appConfigStore.setGitEnabled(e.currentTarget.checked)} />
  </label>
  <p class="setting-hint">{t("preferences.git.enabledHint")}</p>
</section>

<section class:off>
  <h3>{t("preferences.git.panel")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.inActivityBar")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitInActivityBar}
      disabled={off}
      onchange={(e) => appConfigStore.setGitInActivityBar(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.inActivityBarHint", { shortcut })}</p>
</section>

<section class:off>
  <h3>{t("preferences.git.statusBar")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.statusBarShow")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitStatusBar}
      disabled={off}
      onchange={(e) => appConfigStore.setGitStatusBar(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.statusBarHint")}</p>
  {#each GIT_STATUS_PARTS as part (part)}
    <label class="row toggle-row sub">
      <span>{t(`preferences.git.parts.${part}`)}</span>
      <input
        type="checkbox"
        checked={appConfigStore.gitStatusParts[part]}
        disabled={off || !appConfigStore.gitStatusBar}
        onchange={(e) => appConfigStore.setGitStatusPart(part, e.currentTarget.checked)}
      />
    </label>
  {/each}
</section>

<section class:off>
  <h3>{t("preferences.git.remote")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.pr")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitPrEnabled}
      disabled={off}
      onchange={(e) => appConfigStore.setGitPrEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.prHint")}</p>
  <div class="row spaced">
    <span>{t("preferences.git.autoFetch")}</span>
    <SettingDropdown
      options={fetchOptions}
      value={String(appConfigStore.gitAutoFetchMinutes)}
      disabled={off}
      onpick={(value) => appConfigStore.setGitAutoFetchMinutes(Number(value))}
    />
  </div>
  <p class="setting-hint">{t("preferences.git.autoFetchHint")}</p>
</section>

<section>
  <h3>{t("preferences.git.tools.title")}</h3>
  {#each TOOLS as tool (tool.id)}
    {@const found = tools?.[tool.id]}
    <div class="row tool">
      <span class="tool-dot" class:ok={found?.found && found.signedIn !== false} class:warn={found?.found && found.signedIn === false}></span>
      <span class="tool-name">{tool.name}</span>
      <span class="tool-use">{t(tool.use)}</span>
      <span class="tool-state">{found ? toolState(found) : t("preferences.git.tools.checking")}</span>
    </div>
  {/each}
  <p class="setting-hint">{t("preferences.git.tools.hint")}</p>
</section>

<style>
  section {
    margin-bottom: 20px;
  }

  /* Git as a whole is off: its options stay in view, out of reach. */
  section.off {
    opacity: 0.55;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  .row.spaced {
    margin-top: 12px;
  }

  .row.sub {
    margin-top: 7px;
    padding-left: 14px;
  }

  .toggle-row {
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

  .row.tool {
    justify-content: flex-start;
    margin-bottom: 6px;
  }

  .tool-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(128, 128, 128, 0.6);
  }

  .tool-dot.ok {
    background: #3fa04a;
  }

  .tool-dot.warn {
    background: #d7a72a;
  }

  .tool-name {
    flex-shrink: 0;
    width: 34px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-weight: 600;
  }

  .tool-use {
    flex: 1;
    min-width: 0;
    opacity: 0.65;
  }

  .tool-state {
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.8;
  }
</style>
