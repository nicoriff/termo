<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { getTauriVersion, getVersion } from "@tauri-apps/api/app";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openLink } from "$lib/core/links";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { isMac, isWindows } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

  const YEAR = new Date().getFullYear();
  const system = isMac ? "macOS" : isWindows ? "Windows" : "Linux";

  // From tauri.conf.json, which reads package.json: the one place the
  // version is set (see scripts/sync-version.mjs).
  let version = $state<string | null>(null);
  // Not shown, only copied: it matters for a bug report, not to the reader.
  let tauriVersion = $state<string | null>(null);
  let copied = $state(false);
  let dialogEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    void getVersion().then((value) => (version = value));
    void getTauriVersion().then((value) => (tauriVersion = value));
  });

  $effect(() => {
    dialogEl?.focus();
  });

  function close() {
    uiState.closeAbout();
    // The window took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  /** What a bug report needs, in one paste. Kept in English on purpose: it's
   * meant for an issue tracker, not for the person reading the dialog. */
  async function copyInfo() {
    await writeText(`Termo ${version ?? "?"}\nTauri ${tauriVersion ?? "?"}\n${system}\n${navigator.userAgent}`);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function onKeydown(event: KeyboardEvent) {
    // This window owns the keyboard while it's open (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    if (event.key === "Escape" || event.key === "Enter") {
      event.preventDefault();
      close();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("about.title")}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <img class="about-icon" src="/app-icon.png" alt="" />
    <h2 class="about-name">Termo</h2>
    <p class="about-version">{version ? t("about.version", { version }) : " "}</p>
    <p class="about-description">{t("about.description")}</p>

    <dl class="about-details">
      <dt>{t("about.system")}</dt>
      <dd>{system}</dd>
    </dl>

    <p class="about-copyright">
      © {YEAR}
      <button class="about-link" onclick={() => void openLink("https://github.com/nicoriff")}>@nicoriff</button>
    </p>

    <div class="dialog-actions">
      <button class="dialog-button" onclick={() => uiState.openUpdates()}>{t("updates.check")}</button>
      <button class="dialog-button" onclick={() => uiState.openOnboarding()}>{t("onboarding.title")}</button>
      <button class="dialog-button" onclick={() => void copyInfo()}>
        {copied ? t("about.copied") : t("about.copy")}
      </button>
      <button class="dialog-button primary" onclick={close}>{t("about.close")}</button>
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
     centering lives on this wrapper instead, same as the layout manager. */
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
    /* Wide enough for its four buttons in one row. */
    width: min(540px, 92%);
    margin-top: 10vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 24px 20px 16px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    color: var(--titlebar-fg, #d4d4d4);
    text-align: center;
  }

  .dialog:focus {
    outline: none;
  }

  .about-icon {
    width: 64px;
    height: 64px;
  }

  .about-name {
    margin: 10px 0 0;
    font-size: 18px;
    font-weight: 600;
  }

  .about-version {
    margin: 2px 0 0;
    font-size: 12px;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }

  .about-description {
    margin: 14px 0 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
  }

  .about-details {
    display: grid;
    grid-template-columns: auto auto;
    gap: 4px 12px;
    margin: 14px 0 0;
    font-size: 11px;
  }

  .about-details dt {
    text-align: right;
    opacity: 0.6;
  }

  .about-details dd {
    margin: 0;
    text-align: left;
    font-variant-numeric: tabular-nums;
  }

  .about-copyright {
    margin: 14px 0 0;
    font-size: 11px;
    opacity: 0.5;
  }

  /* The GitHub handle, opening the profile in the browser. */
  .about-link {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .about-link:hover,
  .about-link:focus-visible {
    color: var(--pane-active-border, #3a6df0);
    outline: none;
  }

  .dialog-actions {
    align-self: stretch;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
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
</style>
