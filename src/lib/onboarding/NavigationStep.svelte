<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel, type ShortcutAction } from "$lib/core/shortcuts";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import {
    FULL,
    NAV_SCENES,
    SCENE_LENGTH,
    STILL_FRAME,
    sceneFrame,
    type NavScene,
    type SceneKey,
    type TileId,
  } from "./nav-scenes";

  const BEAT_MS = 1100;
  const TILE_IDS: TileId[] = ["a", "b", "c"];
  const TAB_COUNT: Record<TileId, number> = { a: 3, b: 2, c: 1 };
  /** Line widths standing in for each console's text; tile "a" has one set
   * per tab, so switching tabs visibly changes what's in it. */
  const LINES: Record<TileId, number[][]> = {
    a: [
      [70, 45, 85, 30, 60],
      [50, 80, 35, 60, 25],
      [88, 40, 62, 48, 72],
    ],
    b: [[60, 82, 40]],
    c: [[45, 70, 55]],
  };

  const TITLES: Record<NavScene, MessageKey> = {
    picker: "onboarding.navigation.picker.title",
    zoom: "onboarding.navigation.zoom.title",
    tabs: "onboarding.navigation.tabs.title",
    split: "onboarding.navigation.split.title",
  };
  const BODIES: Record<NavScene, MessageKey> = {
    picker: "onboarding.navigation.picker.body",
    zoom: "onboarding.navigation.zoom.body",
    tabs: "onboarding.navigation.tabs.body",
    split: "onboarding.navigation.split.body",
  };

  // The user's own bindings, so what the miniature presses is what works.
  const binding = (action: ShortcutAction) => formatKeybindingLabel(appConfigStore.getEffectiveShortcut(action), isMac);
  const zoom = $derived(binding("zoomTile").split(" / "));
  const keyLabels = $derived<Record<SceneKey, string>>({
    zoomUp: zoom[0],
    zoomDown: zoom[1] ?? zoom[0],
    right: "→",
    down: "↓",
    enter: "Enter",
    cycleTab: binding("cycleTab"),
    splitVertical: t("commands.splitVertical"),
    splitHorizontal: t("commands.splitHorizontal"),
  });
  const sceneKeys = $derived<Record<NavScene, string[]>>({
    picker: [keyLabels.zoomDown, t("onboarding.navigation.arrows"), "Enter"],
    zoom: [keyLabels.zoomUp, keyLabels.zoomDown],
    tabs: [keyLabels.cycleTab, binding("newTab")],
    split: [binding("commandPalette")],
  });

  let scene = $state<NavScene>("picker");
  let frame = $state(0);
  let still = $state(false);
  // Once the user picks a scene it loops, instead of moving on to the next.
  let pinned = false;

  onMount(() => {
    still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still) return;
    const timer = setInterval(() => {
      if (frame + 1 < SCENE_LENGTH[scene]) {
        frame++;
        return;
      }
      if (!pinned) scene = NAV_SCENES[(NAV_SCENES.indexOf(scene) + 1) % NAV_SCENES.length];
      frame = 0;
    }, BEAT_MS);
    return () => clearInterval(timer);
  });

  function pick(next: NavScene) {
    pinned = true;
    scene = next;
    frame = 0;
  }

  const view = $derived(sceneFrame(scene, still ? STILL_FRAME[scene] : frame));

  function rectStyle(id: TileId): string {
    const rect = view.zoomed === id ? FULL : view.tiles[id].rect;
    return `left:${rect.left}%; top:${rect.top}%; width:${rect.width}%; height:${rect.height}%`;
  }
</script>

<div class="onb-stage">
  <div class="demo" role="img" aria-label={t("onboarding.navigation.demoLabel", { name: t(TITLES[scene]) })}>
    <div class="mini-window">
      <div class="mini-titlebar"><i></i><i></i><i></i></div>
      <div class="mini-content">
        {#each TILE_IDS as id (id)}
          {@const tab = id === "a" ? view.activeTab : 0}
          <div
            class="mini-tile"
            class:hidden={!view.tiles[id].shown}
            class:zoomed={view.zoomed === id}
            style={rectStyle(id)}
          >
            <div class="mini-tile-box" class:focused={view.focused === id}>
              <div class="mini-tabs">
                {#each { length: TAB_COUNT[id] } as _, index}
                  <i class:active={index === tab}></i>
                {/each}
              </div>
              <div class="mini-lines">
                {#each LINES[id][tab] as width, index}
                  <i class:prompt={index === 0} style="width:{width}%"></i>
                {/each}
              </div>
            </div>
          </div>
        {/each}
        {#if view.picker}
          <div class="mini-picker" transition:fade={{ duration: 140 }}>
            <div class="mini-map">
              {#each TILE_IDS as id (id)}
                <div class="mini-map-tile" style={rectStyle(id)}>
                  <div class:selected={view.picker === id}></div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>
    <div class="pressed">
      {#if view.key}
        {#key `${scene}-${frame}`}
          <kbd class="pressed-key">{keyLabels[view.key]}</kbd>
        {/key}
      {/if}
    </div>
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.navigation.title")}</h2>
  <p class="onb-intro">{t("onboarding.navigation.intro")}</p>
  <ul class="scenes">
    {#each NAV_SCENES as id (id)}
      <li>
        <button class="scene" class:active={scene === id} aria-pressed={scene === id} onclick={() => pick(id)}>
          <span class="scene-head">
            <span class="scene-title">{t(TITLES[id])}</span>
            <span class="scene-keys">
              {#each sceneKeys[id] as key}
                <kbd>{key}</kbd>
              {/each}
            </span>
          </span>
          {#if scene === id}
            <span class="scene-body">{t(BODIES[id])}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
  <p class="onb-hint">{t("onboarding.navigation.hint")}</p>
</div>

<style>
  .demo {
    width: min(100%, 400px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .mini-window {
    width: 100%;
    aspect-ratio: 16 / 10;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  .mini-titlebar {
    flex-shrink: 0;
    display: flex;
    justify-content: flex-end;
    gap: 5px;
    padding: 5px 7px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .mini-titlebar i {
    width: 7px;
    height: 2px;
    border-radius: 1px;
    background: currentColor;
    opacity: 0.35;
  }

  .mini-content {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .mini-tile {
    position: absolute;
    box-sizing: border-box;
    padding: 3px;
    transition:
      left 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      top 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      width 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      height 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      opacity 200ms ease;
  }

  .mini-tile.hidden {
    opacity: 0;
  }

  .mini-tile.zoomed {
    z-index: 2;
  }

  .mini-tile-box {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    overflow: hidden;
    transition: border-color 160ms ease;
  }

  .mini-tile-box.focused {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .mini-tabs {
    flex-shrink: 0;
    display: flex;
    gap: 3px;
    padding: 4px 5px 0;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .mini-tabs i {
    width: 22px;
    height: 7px;
    border-radius: 2px 2px 0 0;
    background: currentColor;
    opacity: 0.14;
    transition: opacity 160ms ease;
  }

  .mini-tabs i.active {
    opacity: 1;
    background: var(--pane-active-border, #3a6df0);
  }

  .mini-lines {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 6px;
    overflow: hidden;
  }

  .mini-lines i {
    flex-shrink: 0;
    height: 3px;
    border-radius: 2px;
    background: currentColor;
    opacity: 0.22;
    transition: width 260ms ease;
  }

  .mini-lines i.prompt {
    opacity: 0.5;
  }

  .mini-picker {
    position: absolute;
    inset: 0;
    z-index: 3;
    background: rgba(0, 0, 0, 0.5);
  }

  .mini-map {
    position: absolute;
    inset: 11%;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.4);
  }

  .mini-map-tile {
    position: absolute;
    box-sizing: border-box;
    padding: 3px;
  }

  .mini-map-tile div {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 3px;
    transition:
      border-color 120ms ease,
      background 120ms ease;
  }

  .mini-map-tile div.selected {
    border: 2px solid var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
  }

  /* Reserved even when no key is down, so the window doesn't jump. */
  .pressed {
    height: 26px;
    display: flex;
    align-items: center;
  }

  kbd {
    padding: 2px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    font: inherit;
    font-size: 10px;
    white-space: nowrap;
  }

  .pressed-key {
    padding: 4px 10px;
    font-size: 12px;
    border-color: var(--pane-active-border, #3a6df0);
    animation: press 260ms ease-out;
  }

  @keyframes press {
    from {
      transform: translateY(-3px);
      opacity: 0;
    }
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

  .scene-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .scene-title {
    font-weight: 600;
  }

  .scene-keys {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 4px;
  }

  .scene-body {
    line-height: 1.5;
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .mini-tile,
    .mini-lines i {
      transition: none;
    }

    .pressed-key {
      animation: none;
    }
  }
</style>
