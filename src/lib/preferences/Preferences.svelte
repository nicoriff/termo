<script lang="ts">
  import { onMount } from "svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import AppearanceSection from "./sections/AppearanceSection.svelte";
  import ConsoleSection from "./sections/ConsoleSection.svelte";
  import ShortcutsSection from "./sections/ShortcutsSection.svelte";
  import OhMyPoshSection from "./sections/OhMyPoshSection.svelte";
  import AiModeSection from "./sections/AiModeSection.svelte";
  import PlatformSection from "./sections/PlatformSection.svelte";

  type SectionId = "appearance" | "console" | "shortcuts" | "oh-my-posh" | "ai-mode" | "mac" | "windows" | "linux";

  const SECTIONS: { id: SectionId; label: string }[] = [
    { id: "appearance", label: "Apariencia" },
    { id: "console", label: "Consola" },
    { id: "shortcuts", label: "Shortcuts" },
    { id: "oh-my-posh", label: "Oh My Posh" },
    { id: "ai-mode", label: "IA Mode" },
    { id: "mac", label: "macOS" },
    { id: "windows", label: "Windows" },
    { id: "linux", label: "Linux" },
  ];

  let selected = $state<SectionId>("appearance");

  onMount(() => {
    void appConfigStore.init({ restoreWindowSize: false });
  });
</script>

<div class="window-chrome">
  <TitleBar title="Preferencias" showSettingsButton={false} showMaximizeButton={false} />
  <div class="body">
    <nav class="sidebar">
      {#each SECTIONS as section (section.id)}
        <button class="sidebar-item" class:selected={selected === section.id} onclick={() => (selected = section.id)}>
          {section.label}
        </button>
      {/each}
    </nav>
    <div class="detail">
      {#if selected === "appearance"}
        <AppearanceSection />
      {:else if selected === "console"}
        <ConsoleSection />
      {:else if selected === "shortcuts"}
        <ShortcutsSection />
      {:else if selected === "oh-my-posh"}
        <OhMyPoshSection />
      {:else if selected === "ai-mode"}
        <AiModeSection />
      {:else if selected === "mac"}
        <PlatformSection os="mac" />
      {:else if selected === "windows"}
        <PlatformSection os="windows" />
      {:else if selected === "linux"}
        <PlatformSection os="linux" />
      {/if}
    </div>
  </div>
</div>

<style>
  .window-chrome {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    background: var(--terminal-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-radius: 10px;
    overflow: hidden;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .sidebar {
    width: 160px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px;
    box-sizing: border-box;
    background: var(--tabbar-active-bg, #1e1e1e);
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    overflow-y: auto;
  }

  .sidebar-item {
    text-align: left;
    padding: 8px 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .sidebar-item:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .sidebar-item.selected {
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .detail {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 16px 20px;
    box-sizing: border-box;
  }
</style>
