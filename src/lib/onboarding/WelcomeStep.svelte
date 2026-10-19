<script lang="ts">
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  let {
    firstRun,
    path,
  }: {
    /** Whether Termo never ran here, or the setup was asked for again. */
    firstRun: boolean;
    /** The steps ahead, in order: each one's name and what it's about. */
    path: { label: MessageKey; about: MessageKey }[];
  } = $props();
</script>

<div class="welcome">
  <img class="welcome-icon" src="/app-icon.png" alt="" />
  <h2>{t(firstRun ? "onboarding.welcome.titleFirst" : "onboarding.welcome.titleAgain")}</h2>
  <p class="welcome-intro">{t("onboarding.welcome.intro")}</p>

  <ol class="path" aria-label={t("onboarding.welcome.pathLabel")}>
    {#each path as item, index}
      <li style="--order:{index}">
        <span class="path-number">{index + 1}</span>
        <strong>{t(item.label)}</strong>
        <span class="path-about">{t(item.about)}</span>
      </li>
    {/each}
  </ol>
</div>

<style>
  /* The one screen without a picture beside it: it takes both halves. */
  .welcome {
    grid-column: 1 / -1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px 28px;
    text-align: center;
    overflow-y: auto;
  }

  .welcome-icon {
    width: 76px;
    height: 76px;
    animation: arrive 420ms cubic-bezier(0.3, 1.4, 0.5, 1) backwards;
  }

  h2 {
    max-width: 22ch;
    margin: 18px 0 0;
    font-size: 26px;
    font-weight: 600;
    line-height: 1.2;
    text-wrap: balance;
  }

  .welcome-intro {
    max-width: 52ch;
    margin: 10px 0 0;
    font-size: 13px;
    line-height: 1.55;
    opacity: 0.8;
  }

  /* The road ahead, laid out left to right like the steps bar it becomes. */
  .path {
    align-self: stretch;
    display: grid;
    /* One column a step, however many there are: a single row. */
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 8px;
    margin: 28px 0 0;
    padding: 0;
    list-style: none;
  }

  .path li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 14px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    font-size: 12px;
    animation: arrive 360ms ease-out calc(200ms + var(--order) * 70ms) backwards;
  }

  .path-number {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1.5px solid var(--pane-active-border, #3a6df0);
    border-radius: 50%;
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .path-about {
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.65;
    text-wrap: balance;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  @media (max-width: 680px) {
    .path {
      grid-auto-flow: row;
      gap: 6px;
    }

    .path li {
      flex-direction: row;
      padding: 8px 10px;
      text-align: left;
    }

    .path-about {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .welcome-icon,
    .path li {
      animation: none;
    }
  }
</style>
