<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  interface ToolStatus {
    path: string | null;
    managed: boolean;
  }

  // One prompt tool (Oh My Posh or Starship): where it's installed, and its
  // theme. Both work the same way, so PromptSection hands each one in.
  interface Props {
    name: string;
    getStatus: () => Promise<ToolStatus>;
    /** Downloads the binary, then whatever the current theme needs. */
    install: () => Promise<void>;
    themes: string[];
    theme: string;
    themeLabel: string;
    /** Prepares `theme` and saves it; throws to keep the previous one. */
    pickTheme: (theme: string) => Promise<void>;
    /** The theme picker has no effect right now (e.g. own config in use). */
    themeInactive?: boolean;
    searching: string;
    installHint: string;
    hint: string;
    children?: Snippet<[{ installed: boolean }]>;
  }

  let {
    name,
    getStatus,
    install,
    themes,
    theme,
    themeLabel,
    pickTheme,
    themeInactive = false,
    searching,
    installHint,
    hint,
    children,
  }: Props = $props();

  let status = $state<ToolStatus | null>(null);
  let busy = $state<"binary" | "theme" | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    status = await getStatus();
  }

  onMount(() => {
    void refresh();
  });

  async function run(kind: "binary" | "theme", task: () => Promise<void>) {
    busy = kind;
    error = null;
    try {
      await task();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = null;
    }
  }

  function installTool() {
    void run("binary", async () => {
      await install();
      await refresh();
    });
  }

  /** The combo shows `theme`, so one that fails to install leaves it as it was. */
  function onThemePick(picked: string) {
    if (picked === theme) return;
    void run("theme", () => pickTheme(picked));
  }
</script>

<svelte:window onfocus={() => void refresh()} />

<section>
  <h3>{name}</h3>
  {#if status === null}
    <p class="setting-hint">{searching}</p>
  {:else if status.path}
    <p class="status-line">
      {t(status.managed ? "preferences.prompt.installedByTermo" : "preferences.prompt.installed")}
      <code>{status.path}</code>
    </p>
  {:else}
    <div class="row">
      <span>{t("preferences.prompt.notInstalled")}</span>
      <button class="action-btn" onclick={installTool} disabled={busy !== null}>
        {t(busy === "binary" ? "preferences.prompt.downloading" : "preferences.prompt.install")}
      </button>
    </div>
    <p class="setting-hint">{installHint}</p>
  {/if}
</section>

<section>
  <div class="row">
    <span>{themeLabel}</span>
    <SettingDropdown
      options={themes.map((option) => ({ value: option, label: option }))}
      value={theme}
      label={themeLabel}
      disabled={busy !== null || !status?.path || themeInactive}
      onpick={onThemePick}
    />
  </div>
  {@render children?.({ installed: !!status?.path })}
  <p class="setting-hint">{hint}</p>
</section>

{#if error}
  <p class="error">{error}</p>
{/if}

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

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 8px;
  }

  .status-line {
    margin: 0;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  code {
    font-size: 10px;
    opacity: 0.7;
    word-break: break-all;
  }

  .action-btn {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .error {
    font-size: 11px;
    color: #f14c4c;
  }
</style>
