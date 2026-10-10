<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { sendNotification } from "@tauri-apps/plugin-notification";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { canNotify } from "$lib/ai-mode/attention";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
  import { playSound } from "$lib/ai-mode/sounds";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  /** The agents the sample notifications come from: the ones with hooks, or
   * all of them (without hooks Termo still reads their state off the screen). */
  let { tools }: { tools: ToolId[] } = $props();

  const TOAST_MS = 2400;
  type NoticeEvent = "awaiting" | "finished" | "error";

  const events = $derived<NoticeEvent[]>([
    ...(appConfigStore.aiNotifyAwaiting ? (["awaiting"] as const) : []),
    ...(appConfigStore.aiNotifyFinished ? (["finished"] as const) : []),
    ...(appConfigStore.aiNotifyError ? (["error"] as const) : []),
  ]);
  const agents = $derived(tools.length > 0 ? tools : [...TOOL_IDS]);
  const soundsOn = $derived(TOOL_IDS.some((tool) => appConfigStore.aiSoundsEnabled[tool]));

  let beat = $state(0);
  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timer = setInterval(() => beat++, TOAST_MS);
    return () => clearInterval(timer);
  });
  const agent = $derived(agents[beat % agents.length]);
  const event = $derived(events.length > 0 ? events[beat % events.length] : null);
  const eventColor = $derived(
    event &&
      { awaiting: themeStore.current.colors.yellow, finished: themeStore.current.colors.green, error: themeStore.current.colors.red }[
        event
      ],
  );

  /** One switch for every agent; which sound each plays stays as it was. */
  function setSounds(enabled: boolean) {
    for (const tool of TOOL_IDS) appConfigStore.setAiSoundsEnabled(tool, enabled);
  }

  let denied = $state(false);

  /** A real one, so the system's own permission prompt comes up here rather
   * than in the middle of the first agent's turn. */
  async function test() {
    const tool = agents[0];
    if (soundsOn) playSound(appConfigStore.aiSounds[tool].awaiting, appConfigStore.aiSoundVolume);
    denied = !(await canNotify());
    if (denied) return;
    sendNotification({
      title: `${TOOL_DISPLAY_NAMES[tool]}: ${t("aiMode.notify.awaiting")}`,
      body: t("onboarding.notifications.testBody"),
    });
  }
</script>

<div class="onb-stage">
  <div class="desktop" role="img" aria-label={t("onboarding.notifications.previewLabel")}>
    {#if event}
      {#key beat}
        <div class="toast" style="--event:{eventColor}" in:fly={{ x: 40, duration: 260 }}>
          <img src="/app-icon.png" alt="" />
          <div class="toast-text">
            <strong>{TOOL_DISPLAY_NAMES[agent]}: {t(`aiMode.notify.${event}`)}</strong>
            <span>{t("onboarding.notifications.sampleBody")}</span>
          </div>
          <span class="toast-agent"><ConsoleIcon kind={agent} size={18} /></span>
          {#if soundsOn && event !== "error"}
            <span class="sound" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
          {/if}
        </div>
      {/key}
    {:else}
      <p class="muted">{t("onboarding.notifications.muted")}</p>
    {/if}
    <div class="senders" aria-hidden="true">
      {#each agents as tool (tool)}
        <span class:speaking={event !== null && tool === agent}><ConsoleIcon kind={tool} size={16} /></span>
      {/each}
    </div>
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.notifications.title")}</h2>
  <p class="onb-intro">{t("onboarding.notifications.intro")}</p>

  <div class="toggles">
    <label class="toggle-row">
      <span>{t("preferences.aiMode.notifyAwaiting")}</span>
      <input
        class="onb-check"
        type="checkbox"
        checked={appConfigStore.aiNotifyAwaiting}
        onchange={(e) => appConfigStore.setAiNotifyAwaiting(e.currentTarget.checked)}
      />
    </label>
    <label class="toggle-row">
      <span>{t("preferences.aiMode.notifyFinished")}</span>
      <input
        class="onb-check"
        type="checkbox"
        checked={appConfigStore.aiNotifyFinished}
        onchange={(e) => appConfigStore.setAiNotifyFinished(e.currentTarget.checked)}
      />
    </label>
    <label class="toggle-row">
      <span>{t("preferences.aiMode.notifyError")}</span>
      <input
        class="onb-check"
        type="checkbox"
        checked={appConfigStore.aiNotifyError}
        onchange={(e) => appConfigStore.setAiNotifyError(e.currentTarget.checked)}
      />
    </label>
    <label class="toggle-row">
      <span>{t("onboarding.notifications.sounds")}</span>
      <input
        class="onb-check"
        type="checkbox"
        checked={soundsOn}
        onchange={(e) => setSounds(e.currentTarget.checked)}
      />
    </label>
  </div>

  <button class="test" disabled={events.length === 0} onclick={() => void test()}>
    {t("onboarding.notifications.test")}
  </button>
  {#if denied}
    <p class="onb-error" role="alert">{t("onboarding.notifications.denied")}</p>
  {/if}
  <p class="onb-hint">{t("onboarding.notifications.hint")}</p>
</div>

<style>
  .desktop {
    width: min(100%, 380px);
    min-height: 190px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 22px;
  }

  .toast {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-left: 3px solid var(--event);
    border-radius: 8px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
  }

  .toast img {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
  }

  .toast-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }

  .toast-text span {
    font-size: 11px;
    opacity: 0.65;
  }

  .toast-agent {
    flex-shrink: 0;
  }

  /* The sound, drawn: four bars that ring once as the toast lands. */
  .sound {
    position: absolute;
    top: -16px;
    right: 12px;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
  }

  .sound i {
    width: 2px;
    height: 100%;
    border-radius: 1px;
    background: var(--event);
    transform-origin: bottom;
    transform: scaleY(0.15);
    animation: ring 700ms ease-out 200ms;
  }

  .sound i:nth-child(2) {
    animation-delay: 260ms;
  }

  .sound i:nth-child(3) {
    animation-delay: 320ms;
  }

  .sound i:nth-child(4) {
    animation-delay: 380ms;
  }

  @keyframes ring {
    30% {
      transform: scaleY(1);
    }
    60% {
      transform: scaleY(0.5);
    }
  }

  .muted {
    margin: 0;
    padding: 20px 14px;
    border: 1px dashed var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    font-size: 12px;
    line-height: 1.5;
    text-align: center;
    opacity: 0.7;
  }

  .senders {
    display: flex;
    justify-content: center;
    gap: 14px;
  }

  .senders span {
    display: flex;
    padding: 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 50%;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    opacity: 0.45;
    transition:
      opacity 200ms ease,
      transform 200ms ease;
  }

  .senders span.speaking {
    opacity: 1;
    transform: translateY(-4px);
  }

  .toggles {
    margin-top: 14px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 0;
    font-size: 12px;
    cursor: pointer;
  }

  .toggle-row + .toggle-row {
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .test {
    align-self: flex-start;
    margin-top: 12px;
    padding: 6px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .test:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .test:disabled {
    opacity: 0.5;
    cursor: default;
  }

  @media (prefers-reduced-motion: reduce) {
    .sound i {
      animation: none;
      transform: scaleY(0.7);
    }

    .senders span {
      transition: none;
    }
  }
</style>
