<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import AppearanceSection from "./sections/AppearanceSection.svelte";
  import FontSection from "./sections/FontSection.svelte";
  import ConsoleSection from "./sections/ConsoleSection.svelte";
  import ShortcutsSection from "./sections/ShortcutsSection.svelte";
  import OhMyPoshSection from "./sections/OhMyPoshSection.svelte";
  import AiModeSection from "./sections/AiModeSection.svelte";
  import PlatformSection from "./sections/PlatformSection.svelte";

  type SectionId =
    | "appearance"
    | "font"
    | "console"
    | "shortcuts"
    | "oh-my-posh"
    | "ai-mode"
    | "mac"
    | "windows"
    | "linux";

  interface SectionNode {
    id: SectionId;
    label: string;
    children?: { id: SectionId; label: string }[];
  }

  const TREE: SectionNode[] = [
    { id: "appearance", label: "Apariencia", children: [{ id: "font", label: "Fuente" }] },
    {
      id: "console",
      label: "Consola",
      children: [
        { id: "oh-my-posh", label: "Oh My Posh" },
        { id: "ai-mode", label: "IA Mode" },
        { id: "mac", label: "macOS" },
        { id: "windows", label: "Windows" },
        { id: "linux", label: "Linux" },
      ],
    },
    { id: "shortcuts", label: "Shortcuts" },
  ];

  let selected = $state<SectionId>("appearance");
  let expanded = $state<Partial<Record<SectionId, boolean>>>({ appearance: true, console: true });

  function select(node: SectionNode) {
    selected = node.id;
    if (node.children) expanded = { ...expanded, [node.id]: true };
  }

  function toggle(node: SectionNode) {
    const open = !expanded[node.id];
    expanded = { ...expanded, [node.id]: open };
    // Don't leave the selection on a subsection that's no longer visible.
    if (!open && node.children?.some((child) => child.id === selected)) selected = node.id;
  }

  onMount(() => {
    void appConfigStore.init({ restoreWindowSize: false });
  });
</script>

{#snippet chevronSpacer()}
  <span class="chevron-spacer"></span>
{/snippet}

<div class="window-chrome">
  <TitleBar title="Preferencias" showSettingsButton={false} showMaximizeButton={false} />
  <div class="body">
    <nav class="sidebar">
      {#each TREE as node (node.id)}
        <div class="sidebar-row" class:selected={selected === node.id}>
          {#if node.children}
            <button
              class="sidebar-chevron"
              class:expanded={expanded[node.id]}
              aria-label={expanded[node.id] ? `Colapsar ${node.label}` : `Expandir ${node.label}`}
              aria-expanded={!!expanded[node.id]}
              onclick={() => toggle(node)}
            >
              <svg viewBox="0 0 10 10" width="9" height="9">
                <path d="M3 1.5 7 5 3 8.5" fill="none" stroke="currentColor" stroke-width="1.4" />
              </svg>
            </button>
          {:else}
            {@render chevronSpacer()}
          {/if}
          <button class="sidebar-item" onclick={() => select(node)}>{node.label}</button>
        </div>

        {#if node.children && expanded[node.id]}
          <div class="sidebar-children" transition:slide={{ duration: 120 }}>
            {#each node.children as child (child.id)}
              <div class="sidebar-row child" class:selected={selected === child.id}>
                {@render chevronSpacer()}
                <button class="sidebar-item" onclick={() => (selected = child.id)}>{child.label}</button>
              </div>
            {/each}
          </div>
        {/if}
      {/each}
    </nav>
    <div class="detail">
      {#if selected === "appearance"}
        <AppearanceSection />
      {:else if selected === "font"}
        <FontSection />
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
    /* Neutral translucent gray rather than a theme color, so the edge stays
       visible over both light and dark windows behind this one. */
    border: 1px solid rgba(128, 128, 128, 0.45);
    box-sizing: border-box;
    border-radius: 10px;
    overflow: hidden;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .sidebar {
    width: 170px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 8px;
    box-sizing: border-box;
    background: var(--tabbar-active-bg, #1e1e1e);
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    overflow-y: auto;
  }

  .sidebar-children {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .sidebar-row {
    display: flex;
    align-items: center;
    border-radius: 6px;
  }

  .sidebar-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .sidebar-row.selected {
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .sidebar-row.child {
    padding-left: 14px;
  }

  .sidebar-chevron,
  .chevron-spacer {
    width: 18px;
    flex-shrink: 0;
  }

  .sidebar-chevron {
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .sidebar-chevron:hover {
    opacity: 1;
  }

  .sidebar-chevron svg {
    transition: transform 120ms ease;
  }

  .sidebar-chevron.expanded svg {
    transform: rotate(90deg);
  }

  .sidebar-item {
    flex: 1;
    min-width: 0;
    text-align: left;
    padding: 8px 10px 8px 2px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .sidebar-row.child .sidebar-item {
    font-size: 11px;
  }

  .detail {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 16px 20px;
    box-sizing: border-box;
  }
</style>
