<script lang="ts">
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  /** What the panel tells, beside the picture. */
  const TOPICS: { title: MessageKey; body: MessageKey }[] = [
    { title: "onboarding.remote.persistent.title", body: "onboarding.remote.persistent.body" },
    { title: "onboarding.remote.devices.title", body: "onboarding.remote.devices.body" },
  ];

  /** What the console shows, on every screen: the same in any language. */
  const AGENT_LINES = ["❯ npm test", "✓ 42 passed", "Commit? (y/n) y"];
  /** Where the browsers on the other devices open Termo: a tailnet name. */
  const ADDRESS = "pc.tailnet.ts.net";

  /** Tailscale's mark: a three-by-three grid of dots, the middle row and the
   * one under its center lit. */
  const MARK_DOTS = [0, 1, 2].flatMap((row) => [0, 1, 2].map((col) => ({ row, col, lit: row === 1 || (row === 2 && col === 1) })));
</script>

{#snippet lines()}
  {#each AGENT_LINES as line, index (index)}
    <span class="line">{line}</span>
  {/each}
{/snippet}

{#snippet padlock()}
  <svg class="padlock" viewBox="0 0 16 16" aria-hidden="true">
    <rect x="3.5" y="7" width="9" height="7" rx="1.5" />
    <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
  </svg>
{/snippet}

{#snippet address()}
  <span class="address">{@render padlock()}<span>{ADDRESS}</span></span>
{/snippet}

{#snippet caption(label: string)}
  <span class="caption">
    {label}
    <span class="badge yours">
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.3l2.6 2.6L12 5.2" /></svg>
      {t("onboarding.remote.yours")}
    </span>
  </span>
{/snippet}

<!-- This computer's consoles, through the tailnet, on your laptop and your
     phone at once; someone else's device kept out. All of it there from the
     start: only what goes over the connectors moves. -->
<div class="onb-stage">
  <div class="demo" role="img" aria-label={t("onboarding.remote.demoLabel", { name: t("onboarding.remote.devices.title") })}>
    <div class="net">
      <!-- The connectors, under the devices; stretched to the scene, the
           strokes keep their width. -->
      <svg class="wires" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <path class="wire" d="M50 27 L50 38" />
        <path class="wire" d="M50 52 C50 57 27 54 27 58" />
        <path class="wire" d="M50 52 C50 57 75 54 75 58" />
        <path class="wire refused" d="M19 45 L44 45" />
      </svg>

      <!-- This computer: a monitor on its stand, Termo on screen. -->
      <div class="monitor">
        <div class="bezel">
          <i class="camera"></i>
          <div class="glass">
            <span class="app-bar"><i></i><i></i><i></i><ConsoleIcon kind="claude" size={10} /> Claude Code</span>
            <div class="console">{@render lines()}</div>
          </div>
        </div>
        <span class="neck"></span>
        <span class="foot"></span>
        <span class="monitor-caption">{t("onboarding.remote.computer")}</span>
      </div>

      <div class="hub">
        <span class="mark">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            {#each MARK_DOTS as dot (`${dot.row}-${dot.col}`)}
              <circle cx={5 + dot.col * 7} cy={5 + dot.row * 7} r="2.6" class:lit={dot.lit} />
            {/each}
          </svg>
          <span class="lock">{@render padlock()}</span>
        </span>
        <span class="hub-label">
          <strong>{t("titlebar.remote.tailscale")}</strong>
          <small>{t("onboarding.remote.encrypted")}</small>
        </span>
      </div>

      <div class="stranger">
        <span class="stranger-device"><i></i></span>
        <span class="refused-mark" aria-hidden="true">
          <svg viewBox="0 0 16 16"><path d="M5 5l6 6M11 5l-6 6" /></svg>
        </span>
        <span class="badge refused">{t("onboarding.remote.noAccess")}</span>
      </div>

      <!-- A laptop: the lid, with its camera and a browser on Termo, the
           hinge, the deck seen from the front and its shadow. -->
      <div class="device laptop">
        <div class="lid">
          <i class="camera"></i>
          <div class="glass">
            <span class="browser-tabs"><i></i><i></i><i></i><span class="browser-tab">Termo</span></span>
            {@render address()}
            <div class="console">{@render lines()}</div>
          </div>
        </div>
        <span class="hinge"></span>
        <span class="deck"><i></i></span>
        <span class="floor-shadow"></span>
        {@render caption(t("onboarding.remote.laptopLabel"))}
      </div>

      <!-- A phone: its body and side buttons, the island, the status bar,
           a browser on Termo and the home indicator. -->
      <div class="device phone">
        <div class="body">
          <i class="button power"></i>
          <i class="button volume"></i>
          <div class="glass">
            <span class="status-bar"><span>9:41</span><i class="island"></i><span class="signal"><i></i><i></i><i></i></span></span>
            {@render address()}
            <div class="console">{@render lines()}</div>
            <i class="home"></i>
          </div>
        </div>
        {@render caption(t("onboarding.remote.phoneLabel"))}
      </div>
    </div>
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.remote.title")}</h2>
  <p class="onb-intro">{t("onboarding.remote.intro")}</p>
  <ul class="topics">
    {#each TOPICS as topic (topic.title)}
      <li>
        <span class="topic-title">{t(topic.title)}</span>
        <span class="topic-body">{t(topic.body)}</span>
      </li>
    {/each}
  </ul>
  <p class="onb-hint">{t("onboarding.remote.hint")}</p>
</div>

<style>
  .demo {
    --accent: var(--pane-active-border, #3a6df0);
    --ok: #22a55b;
    --refused: #f14c4c;
    /* The hardware itself is dark, whatever the theme. */
    --frame: #1b1c20;
    --frame-edge: #3b3e46;
    --frame-shine: #50545e;
    --screen: #0f1013;
    --screen-fg: #d8dbe2;
    width: min(100%, 400px);
    font-size: 11px;
  }

  .console {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-family: var(--terminal-font-family, monospace);
    line-height: 1.35;
    overflow: hidden;
  }

  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Placed on the same 0–100 grid as the connectors. */
  .net {
    position: relative;
    height: 345px;
  }

  .wires {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  /* What goes over them, moving along: encrypted, from end to end. */
  .wire {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-dasharray: 6 4;
    vector-effect: non-scaling-stroke;
    animation: flow 600ms linear infinite;
  }

  @keyframes flow {
    to {
      stroke-dashoffset: -10;
    }
  }

  .wire.refused {
    stroke: var(--refused);
    stroke-dasharray: 3 4;
    opacity: 0.75;
    animation: none;
  }

  /* Every screen: dark glass, a faint reflection across it. */
  .glass {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
    background:
      linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent 40%),
      var(--screen);
    color: var(--screen-fg);
  }

  .glass .console {
    flex: 1;
    min-height: 0;
  }

  .camera {
    display: block;
    width: 3px;
    height: 3px;
    margin: 0 auto;
    border-radius: 50%;
    background: #2c3038;
    box-shadow: inset 0 0 0 1px #3c4250;
  }

  .padlock {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
  }

  /* A browser's address bar, the padlock first: the tailnet's own address. */
  .address {
    display: flex;
    align-items: center;
    gap: 3px;
    margin: 4px 5px 0;
    padding: 1px 5px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.07);
    font-size: 8px;
    white-space: nowrap;
    overflow: hidden;
  }

  .address .padlock {
    color: #3fbf75;
  }

  .address span {
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.75;
  }

  /* The devices' shadow, and the accent ring of being connected. */
  .monitor .bezel,
  .lid,
  .phone .body {
    box-shadow:
      0 0 0 1px var(--accent),
      0 10px 24px rgba(0, 0, 0, 0.35);
  }

  /* This computer. */
  .monitor {
    position: absolute;
    top: 0;
    left: 50%;
    width: 52%;
    display: flex;
    flex-direction: column;
    align-items: center;
    transform: translateX(-50%);
  }

  .monitor .bezel {
    box-sizing: border-box;
    width: 100%;
    height: 76px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 3px 4px 4px;
    border: 1px solid var(--frame-edge);
    border-radius: 7px;
    background: var(--frame);
  }

  .monitor .glass {
    flex: 1;
    border-radius: 3px;
  }

  .app-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 6px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 8.5px;
  }

  .app-bar i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.18);
  }

  .app-bar i:last-of-type {
    margin-right: 4px;
  }

  .monitor .console {
    padding: 3px 7px;
    font-size: 9px;
  }

  .neck {
    width: 12%;
    height: 8px;
    background: linear-gradient(90deg, var(--frame-edge), var(--frame-shine), var(--frame-edge));
  }

  .foot {
    width: 34%;
    height: 4px;
    border-radius: 3px 3px 1px 1px;
    background: var(--frame-shine);
  }

  .monitor-caption {
    position: absolute;
    left: calc(100% + 10px);
    top: 30px;
    width: 70px;
    font-size: 10px;
    line-height: 1.3;
    opacity: 0.75;
  }

  /* The tailnet. */
  .hub {
    position: absolute;
    top: 45%;
    left: 50%;
    display: flex;
    align-items: center;
    gap: 10px;
    transform: translate(-22px, -50%);
  }

  .mark {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    width: 44px;
    height: 44px;
    border: 1.5px solid var(--accent);
    border-radius: 12px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 0 0 4px var(--tab-focus-bg, rgba(58, 109, 240, 0.25));
  }

  .mark > svg {
    width: 26px;
    height: 26px;
  }

  .mark circle {
    fill: currentColor;
    opacity: 0.25;
  }

  .mark circle.lit {
    opacity: 1;
  }

  /* The padlock on its corner: what goes through is encrypted. */
  .lock {
    position: absolute;
    right: -7px;
    bottom: -7px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--ok);
    color: #fff;
  }

  .lock .padlock {
    width: 11px;
    height: 11px;
    stroke-width: 1.6;
  }

  .hub-label {
    display: flex;
    flex-direction: column;
    white-space: nowrap;
  }

  .hub-label small {
    margin-top: 1px;
    font-size: 10px;
    opacity: 0.65;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 5px;
    border-radius: 8px;
    font-size: 9px;
    white-space: nowrap;
  }

  .badge svg {
    width: 9px;
    height: 9px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .badge.yours {
    background: rgba(34, 165, 91, 0.18);
    color: #3fbf75;
  }

  .badge.refused {
    background: rgba(241, 76, 76, 0.16);
    color: var(--refused);
  }

  /* Someone else's device: it reaches for the tailnet and is turned away. */
  .stranger {
    position: absolute;
    top: 45%;
    left: 2%;
    width: 42%;
    height: 0;
  }

  .stranger-device {
    position: absolute;
    left: 6%;
    top: -19px;
    box-sizing: border-box;
    width: 22px;
    height: 38px;
    border: 1.5px dashed var(--refused);
    border-radius: 6px;
    opacity: 0.75;
  }

  .stranger-device i {
    position: absolute;
    left: 50%;
    bottom: 3px;
    width: 8px;
    height: 1.5px;
    border-radius: 1px;
    background: var(--refused);
    transform: translateX(-50%);
  }

  .refused-mark {
    position: absolute;
    left: 62%;
    top: -9px;
    display: flex;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--refused);
    color: #fff;
    transform: translateX(-50%);
  }

  .refused-mark svg {
    width: 100%;
    height: 100%;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .stranger .badge {
    position: absolute;
    left: 62%;
    top: 14px;
    transform: translateX(-50%);
  }

  /* Your devices, under the tailnet. */
  .device {
    position: absolute;
    top: 58%;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .device .console {
    padding: 3px 7px 4px;
    font-size: 8.5px;
  }

  .caption {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 6px;
    font-size: 10px;
    white-space: nowrap;
  }

  .laptop {
    left: 4%;
    width: 46%;
  }

  .lid {
    box-sizing: border-box;
    width: 100%;
    height: 104px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 3px 5px 6px;
    border: 1px solid var(--frame-edge);
    border-bottom: none;
    border-radius: 9px 9px 2px 2px;
    background: linear-gradient(180deg, #24262b, var(--frame));
  }

  .lid .glass {
    flex: 1;
    border-radius: 2px;
  }

  /* The browser on it: its window buttons and a tab on Termo, then the
     address bar. */
  .browser-tabs {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 10px;
    padding: 0 5px;
    background: rgba(255, 255, 255, 0.04);
  }

  .browser-tabs > i {
    align-self: center;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
  }

  .browser-tabs > i:nth-of-type(1) {
    background: #ff5f57;
  }

  .browser-tabs > i:nth-of-type(2) {
    background: #febc2e;
  }

  .browser-tabs > i:nth-of-type(3) {
    margin-right: 4px;
    background: #28c840;
  }

  .browser-tab {
    padding: 1px 8px 0;
    border-radius: 3px 3px 0 0;
    background: var(--screen);
    font-size: 6.5px;
    line-height: 8px;
    opacity: 0.9;
  }

  .laptop .address {
    margin-top: 3px;
  }

  /* Where the lid meets the deck. */
  .hinge {
    width: 100%;
    height: 3px;
    background: linear-gradient(180deg, #0b0c0e, #24262c);
  }

  /* The deck, wider than the lid and seen from the front: lit along its
     edge, with the notch to open it. */
  .deck {
    position: relative;
    width: 118%;
    height: 8px;
    clip-path: polygon(2.5% 0, 97.5% 0, 100% 55%, 99% 100%, 1% 100%, 0 55%);
    background: linear-gradient(180deg, #7a7f8a 0, #5a5e68 18%, #42454d 60%, #2c2e34 100%);
  }

  .deck i {
    position: absolute;
    left: 50%;
    top: 0;
    width: 15%;
    height: 3px;
    border-radius: 0 0 5px 5px;
    background: linear-gradient(180deg, #2a2c32, #3a3d44);
    transform: translateX(-50%);
  }

  .floor-shadow {
    width: 110%;
    height: 6px;
    margin-top: -1px;
    border-radius: 50%;
    background: radial-gradient(ellipse at center, rgba(0, 0, 0, 0.45), transparent 70%);
  }

  .laptop .caption {
    margin-top: 1px;
  }

  .phone {
    left: calc(75% - 33px);
    width: 66px;
  }

  .phone .body {
    position: relative;
    box-sizing: border-box;
    width: 100%;
    height: 118px;
    padding: 3px;
    border: 1px solid var(--frame-shine);
    border-radius: 13px;
    background: var(--frame);
  }

  .button {
    position: absolute;
    width: 2px;
    border-radius: 1px;
    background: var(--frame-shine);
  }

  .button.power {
    right: -3px;
    top: 26px;
    height: 16px;
  }

  .button.volume {
    left: -3px;
    top: 20px;
    height: 22px;
  }

  .phone .glass {
    height: 100%;
    border-radius: 10px;
  }

  .status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 6px 0;
    font-size: 6.5px;
    font-weight: 600;
  }

  .island {
    width: 18px;
    height: 5px;
    border-radius: 3px;
    background: #000;
  }

  .signal {
    display: flex;
    align-items: flex-end;
    gap: 1px;
    height: 5px;
  }

  .signal i {
    width: 1.5px;
    background: currentColor;
  }

  .signal i:nth-child(1) {
    height: 2px;
  }

  .signal i:nth-child(2) {
    height: 3.5px;
  }

  .signal i:nth-child(3) {
    height: 5px;
  }

  .phone .address {
    margin: 3px 3px 0;
    padding: 1px 3px;
    font-size: 6.5px;
  }

  .phone .console {
    padding: 3px 5px;
    font-size: 7.5px;
  }

  .phone .line {
    white-space: normal;
  }

  .home {
    align-self: center;
    flex-shrink: 0;
    width: 36%;
    height: 2px;
    margin: 0 0 3px;
    border-radius: 1px;
    background: rgba(255, 255, 255, 0.45);
  }

  .topics {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-size: 12px;
  }

  .topics li {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .topic-title {
    font-weight: 600;
  }

  .topic-body {
    line-height: 1.5;
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .wire {
      animation: none;
    }
  }
</style>
