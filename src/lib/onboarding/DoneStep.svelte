<script lang="ts">
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  /** The agents left with hooks installed. */
  let { hooked }: { hooked: ToolId[] } = $props();

  const SPARK_COUNT = 10;
  // The chosen theme's own colors, so the celebration is in it too.
  const sparkColors = $derived(
    (["red", "yellow", "green", "cyan", "blue", "magenta"] as const).map((name) => themeStore.current.colors[name]),
  );

  const theme = $derived(
    themeStore.syncWithOs
      ? t("onboarding.done.themeSynced", { light: themeStore.lightTheme, dark: themeStore.darkTheme })
      : themeStore.current.name,
  );
  const notifying = $derived(
    appConfigStore.aiNotifyAwaiting || appConfigStore.aiNotifyFinished || appConfigStore.aiNotifyError,
  );
</script>

<div class="onb-stage">
  <div class="ready" aria-hidden="true">
    {#each { length: SPARK_COUNT } as _, index}
      <i
        class="spark"
        style="--angle:{(360 / SPARK_COUNT) * index}deg; --color:{sparkColors[index % sparkColors.length]}; --delay:{index *
          25}ms"
      ></i>
    {/each}
    <svg class="ring" viewBox="0 0 120 120">
      <circle cx="60" cy="60" r="56" pathLength="100" />
    </svg>
    <img src="/app-icon.png" alt="" />
    <svg class="check" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="12" />
      <path d="M6.5 12.4l3.6 3.6 7.4-8" pathLength="100" />
    </svg>
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.done.title")}</h2>
  <p class="onb-intro">{t("onboarding.done.intro")}</p>

  <dl class="summary">
    <dt>{t("onboarding.done.theme")}</dt>
    <dd>
      <i class="theme-chip" style="background:{themeStore.current.colors.background}; border-color:{themeStore.current.ui.accent}"
      ></i>
      {theme}
    </dd>
    <dt>{t("onboarding.done.hooks")}</dt>
    <dd>
      {#each hooked as tool (tool)}
        <span class="agent"><ConsoleIcon kind={tool} size={14} />{TOOL_DISPLAY_NAMES[tool]}</span>
      {:else}
        {t("onboarding.done.hooksNone")}
      {/each}
    </dd>
    <dt>{t("onboarding.done.notifications")}</dt>
    <dd>{t(notifying ? "onboarding.done.notificationsOn" : "onboarding.done.notificationsOff")}</dd>
  </dl>
  <p class="onb-hint">{t("onboarding.done.hint")}</p>
</div>

<style>
  .ready {
    position: relative;
    width: 150px;
    height: 150px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .ready img {
    width: 84px;
    height: 84px;
    animation: settle 420ms cubic-bezier(0.3, 1.4, 0.5, 1) backwards;
  }

  @keyframes settle {
    from {
      transform: scale(0.7);
      opacity: 0;
    }
  }

  .ring {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }

  .ring circle {
    fill: none;
    stroke: var(--pane-active-border, #3a6df0);
    stroke-width: 2.5;
    stroke-linecap: round;
    stroke-dasharray: 100;
    stroke-dashoffset: 100;
    animation: draw 700ms ease-out 150ms forwards;
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  .check {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 36px;
    height: 36px;
    animation: settle 300ms cubic-bezier(0.3, 1.4, 0.5, 1) 750ms backwards;
  }

  .check circle {
    fill: #22a55b;
  }

  .check path {
    fill: none;
    stroke: #fff;
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-dasharray: 100;
    stroke-dashoffset: 100;
    animation: draw 300ms ease-out 950ms forwards;
  }

  /* Thrown outward once, as the ring closes. */
  .spark {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 6px;
    height: 6px;
    margin: -3px;
    border-radius: 50%;
    background: var(--color);
    opacity: 0;
    animation: burst 900ms cubic-bezier(0.2, 0.7, 0.3, 1) calc(800ms + var(--delay)) backwards;
  }

  @keyframes burst {
    from {
      opacity: 1;
      transform: rotate(var(--angle)) translateY(-70px) scale(1);
    }
    to {
      opacity: 0;
      transform: rotate(var(--angle)) translateY(-118px) scale(0.4);
    }
  }

  .summary {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px 16px;
    margin: 16px 0 0;
    font-size: 12px;
  }

  .summary dt {
    opacity: 0.6;
  }

  .summary dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }

  .theme-chip {
    width: 12px;
    height: 12px;
    box-sizing: border-box;
    border: 2px solid;
    border-radius: 50%;
  }

  .agent {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  @media (prefers-reduced-motion: reduce) {
    .ready img,
    .check {
      animation: none;
    }

    .ring circle,
    .check path {
      animation: none;
      stroke-dashoffset: 0;
    }

    .spark {
      display: none;
    }
  }
</style>
