<script lang="ts">
  import { onMount } from "svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  type Scene = "persistent" | "phone";
  const SCENES: Scene[] = ["persistent", "phone"];
  const BEAT_MS = 1100;
  const SCENE_LENGTH: Record<Scene, number> = { persistent: 7, phone: 7 };
  /** The beat that stands for the whole scene when motion is reduced. */
  const STILL_BEAT: Record<Scene, number> = { persistent: 3, phone: 5 };

  const TITLES: Record<Scene, MessageKey> = {
    persistent: "onboarding.remote.persistent.title",
    phone: "onboarding.remote.phone.title",
  };
  const BODIES: Record<Scene, MessageKey> = {
    persistent: "onboarding.remote.persistent.body",
    phone: "onboarding.remote.phone.body",
  };

  /** What the console prints, a line a beat: the same in any language. */
  const SERVER_LINES = ["$ npm run dev", "ready on :5173", "GET / 200", "GET /api 200", "GET /login 200", "GET /api 200"];
  const AGENT_LINES = ["❯ npm test", "✓ 42 passed", "Commit the changes? (y/n)"];
  /** What's typed on the phone, and the beat it shows up there and here. */
  const TYPED = "y";
  const TYPED_BEAT = 4;

  let scene = $state<Scene>("persistent");
  let beat = $state(0);
  let still = $state(false);
  // Once the user picks a scene it loops, instead of moving on to the other.
  let pinned = false;

  onMount(() => {
    still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still) return;
    const timer = setInterval(() => {
      if (beat + 1 < SCENE_LENGTH[scene]) {
        beat++;
        return;
      }
      if (!pinned) scene = SCENES[(SCENES.indexOf(scene) + 1) % SCENES.length];
      beat = 0;
    }, BEAT_MS);
    return () => clearInterval(timer);
  });

  function pick(next: Scene) {
    pinned = true;
    scene = next;
    beat = 0;
  }

  const now = $derived(still ? STILL_BEAT[scene] : beat);

  // Kept running: open, Termo closes and the console goes on, Termo is back.
  const windowState = $derived<"open" | "closed" | "back">(now < 2 ? "open" : now < 5 ? "closed" : "back");
  const serverLines = $derived(SERVER_LINES.slice(0, Math.min(SERVER_LINES.length, now + 2)));
  const windowLabel = $derived(
    windowState === "closed"
      ? t("onboarding.remote.running")
      : windowState === "back"
        ? t("onboarding.remote.reconnected")
        : null,
  );

  // From the phone: the link comes up, the phone shows the console, an
  // answer typed there reaches it.
  const linked = $derived(now >= 1);
  const onPhone = $derived(now >= 2);
</script>

<div class="onb-stage">
  <div class="demo" role="img" aria-label={t("onboarding.remote.demoLabel", { name: t(TITLES[scene]) })}>
    {#if scene === "persistent"}
      <div class="persistent">
        <div class="window" class:closed={windowState === "closed"}>
          <div class="window-bar">
            <i></i><i></i><i></i>
            <span class="window-name">{windowState === "closed" ? t("onboarding.remote.closed") : "Termo"}</span>
          </div>
          <div class="console" class:live={windowState === "closed"}>
            {#each serverLines as line, index (index)}
              <span class="line">{line}</span>
            {/each}
          </div>
        </div>
        <span class="state" class:shown={windowLabel !== null}>
          <i class="dot {windowState === 'closed' ? 'running' : 'done'}"></i>{windowLabel ?? ""}
        </span>
      </div>
    {:else}
      <div class="phone-scene">
        <div class="device computer">
          <span class="device-name">{t("onboarding.remote.computer")}</span>
          <div class="console">
            <span class="console-head"><ConsoleIcon kind="claude" size={14} /> Claude Code</span>
            {#each AGENT_LINES as line, index (index)}
              <span class="line">{line}</span>
            {/each}
            {#if now >= TYPED_BEAT + 1}
              <span class="line typed">{TYPED}</span>
            {/if}
          </div>
        </div>
        <div class="link" class:live={linked}>
          <span class="pill">{t("titlebar.remote.tailscale")}</span>
        </div>
        <div class="device phone" class:shown={onPhone}>
          <span class="device-name">{t("onboarding.remote.phoneLabel")}</span>
          <div class="console">
            <span class="console-head"><ConsoleIcon kind="claude" size={12} /></span>
            {#each AGENT_LINES as line, index (index)}
              <span class="line">{line}</span>
            {/each}
            {#if now >= TYPED_BEAT}
              <span class="line typed">{TYPED}</span>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.remote.title")}</h2>
  <p class="onb-intro">{t("onboarding.remote.intro")}</p>
  <ul class="scenes">
    {#each SCENES as id (id)}
      <li>
        <button class="scene" class:active={scene === id} aria-pressed={scene === id} onclick={() => pick(id)}>
          <span class="scene-title">{t(TITLES[id])}</span>
          {#if scene === id}
            <span class="scene-body">{t(BODIES[id])}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
  <p class="onb-hint">{t("onboarding.remote.hint")}</p>
</div>

<style>
  .demo {
    width: min(100%, 400px);
    font-size: 11px;
  }

  .dot {
    display: inline-block;
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.35;
  }

  .dot.running {
    opacity: 1;
    background: var(--pane-active-border, #3a6df0);
    animation: pulse 900ms ease-in-out infinite;
  }

  .dot.done {
    opacity: 1;
    background: #22a55b;
  }

  @keyframes pulse {
    50% {
      transform: scale(1.5);
      opacity: 0.5;
    }
  }

  .console {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 10px;
    font-family: var(--terminal-font-family, monospace);
    font-size: 10.5px;
    line-height: 1.35;
    overflow: hidden;
  }

  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .line.typed {
    color: var(--pane-active-border, #3a6df0);
  }

  /* Kept running: a Termo window whose frame goes while its console stays. */
  .persistent {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .window {
    box-sizing: border-box;
    width: 82%;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
    transition:
      border-color 260ms ease,
      background 260ms ease,
      box-shadow 260ms ease;
  }

  .window.closed {
    border-style: dashed;
    background: transparent;
    box-shadow: none;
  }

  .window-bar {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    transition: opacity 260ms ease;
  }

  .window-bar i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--tabbar-border, #2a2a2a);
  }

  .window.closed .window-bar {
    opacity: 0.6;
  }

  .window.closed .window-bar i {
    visibility: hidden;
  }

  .window-name {
    margin-left: 6px;
    opacity: 0.7;
  }

  .window .console {
    height: 92px;
    justify-content: flex-end;
    margin: 6px;
    border: 1px solid transparent;
    border-radius: 6px;
    transition:
      border-color 260ms ease,
      background 260ms ease;
  }

  .window .console.live {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tabbar-active-bg-solid, #1e1e1e);
  }

  .state {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 14px;
    opacity: 0;
    transition: opacity 200ms ease;
  }

  .state.shown {
    opacity: 0.85;
  }

  /* From the phone: this computer, the tailnet between, the phone. */
  .phone-scene {
    display: flex;
    align-items: center;
  }

  .device {
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
  }

  .device-name {
    padding: 6px 10px 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .console-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
    font-family: inherit;
    opacity: 0.8;
  }

  .computer {
    width: 52%;
    border-radius: 8px;
  }

  .phone {
    width: 30%;
    min-height: 150px;
    border-radius: 14px;
    opacity: 0.35;
    transition:
      opacity 260ms ease,
      border-color 260ms ease;
  }

  .phone.shown {
    opacity: 1;
    border-color: var(--pane-active-border, #3a6df0);
  }

  /* Before it's up, the phone's console is just its frame. */
  .phone:not(.shown) .console {
    visibility: hidden;
  }

  .phone .console {
    padding: 6px 8px;
    font-size: 9.5px;
  }

  .phone .line {
    white-space: normal;
  }

  .link {
    position: relative;
    flex: 1;
    min-width: 16px;
    height: 2px;
    background: var(--tabbar-border, #2a2a2a);
    transition: background 200ms ease;
  }

  .link.live {
    background: var(--pane-active-border, #3a6df0);
  }

  .pill {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    padding: 3px 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    font-size: 9.5px;
    white-space: nowrap;
    transition: border-color 200ms ease;
  }

  .link.live .pill {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .scenes {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .scene {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .scene:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .scene:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 1px;
  }

  .scene.active {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .scene-title {
    font-weight: 600;
  }

  .scene-body {
    line-height: 1.5;
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .dot.running {
      animation: none;
    }

    .window,
    .window-bar,
    .window .console,
    .state,
    .phone,
    .link,
    .pill {
      transition: none;
    }
  }
</style>
