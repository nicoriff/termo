<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { preferencesReady, setBackgroundBlur } from "$lib/core/window-client";
  import { isWindows } from "$lib/core/platform";
  import AppearanceSection from "./sections/AppearanceSection.svelte";
  import FontSection from "./sections/FontSection.svelte";
  import ConsoleSection from "./sections/ConsoleSection.svelte";
  import ShortcutsSection from "./sections/ShortcutsSection.svelte";
  import OhMyPoshSection from "./sections/OhMyPoshSection.svelte";
  import AiModeSection from "./sections/AiModeSection.svelte";
  import AiNotificationsSection from "./sections/AiNotificationsSection.svelte";
  import AiAgentsSection from "./sections/AiAgentsSection.svelte";
  import MarketplaceSection from "./sections/MarketplaceSection.svelte";
  import { listen } from "@tauri-apps/api/event";
  import AiToolSection from "./sections/AiToolSection.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import PlatformSection from "./sections/PlatformSection.svelte";
  import GeneralSection from "./sections/GeneralSection.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  type SectionId =
    | "general"
    | "appearance"
    | "font"
    | "console"
    | "shortcuts"
    | "oh-my-posh"
    | "ai-mode"
    | "ai-notifications"
    | "ai-agents"
    | "marketplace"
    | `ai-tool-${ToolId}`
    | "mac"
    | "windows"
    | "linux";

  /** Sidebar groups that only hold subsections and have no page of their own. */
  type GroupId = "integrations";

  interface SectionNode {
    id: SectionId | GroupId;
    label: MessageKey;
    /** A product name shows as it is (see CLAUDE.md), the rest translated. */
    children?: ({ id: SectionId; tool?: ToolId } & ({ label: MessageKey } | { name: string }))[];
  }

  function childLabel(child: NonNullable<SectionNode["children"]>[number]): string {
    return "name" in child ? child.name : t(child.label);
  }

  const TREE: SectionNode[] = [
    { id: "general", label: "preferences.sidebar.general" },
    {
      id: "appearance",
      label: "preferences.sidebar.appearance",
      children: [{ id: "font", label: "preferences.sidebar.font" }],
    },
    {
      id: "console",
      label: "preferences.sidebar.console",
      children: [
        { id: "mac", label: "preferences.sidebar.mac" },
        { id: "windows", label: "preferences.sidebar.windows" },
        { id: "linux", label: "preferences.sidebar.linux" },
      ],
    },
    {
      id: "ai-mode",
      label: "preferences.sidebar.aiMode",
      children: [
        { id: "ai-notifications", label: "preferences.sidebar.aiNotifications" },
        { id: "ai-agents", label: "preferences.sidebar.aiAgents" },
        // Each agent CLI's own settings: its tab, hooks, status bar and sounds.
        ...TOOL_IDS.map((tool) => ({ id: `ai-tool-${tool}` as const, name: TOOL_DISPLAY_NAMES[tool], tool })),
      ],
    },
    {
      id: "integrations",
      label: "preferences.sidebar.integrations",
      children: [
        { id: "oh-my-posh", label: "preferences.sidebar.ohMyPosh" },
        { id: "marketplace", label: "preferences.sidebar.marketplace" },
      ],
    },
    { id: "shortcuts", label: "preferences.sidebar.shortcuts" },
  ];

  let selected = $state<SectionId>("general");
  let expanded = $state<Partial<Record<SectionNode["id"], boolean>>>({
    appearance: true,
    console: true,
    "ai-mode": true,
    integrations: true,
  });

  function hasPage(node: SectionNode): node is SectionNode & { id: SectionId } {
    return node.id !== "integrations";
  }

  function containsSelected(node: SectionNode): boolean {
    return !!node.children?.some((child) => child.id === selected);
  }

  function select(node: SectionNode) {
    if (hasPage(node)) selected = node.id;
    else if (node.children?.length) selected = node.children[0].id;
    if (node.children) expanded = { ...expanded, [node.id]: true };
  }

  function toggle(node: SectionNode) {
    const open = !expanded[node.id];
    expanded = { ...expanded, [node.id]: open };
    // Don't leave the selection on a subsection that's no longer visible; a
    // group without a page keeps it and highlights its own row instead.
    if (!open && hasPage(node) && containsSelected(node)) selected = node.id;
  }

  // Another window asks for a page of this one: the marketplace panel,
  // for where a source's key goes.
  onMount(() => {
    let unlisten: (() => void) | null = null;
    void listen<string>("preferences-section", (event) => {
      if (event.payload === "marketplace") {
        selected = "marketplace";
        expanded = { ...expanded, integrations: true };
      }
    }).then((stop) => (unlisten = stop));
    return () => unlisten?.();
  });

  onMount(() => {
    // The window is preloaded hidden; the backend shows it only once the real
    // config has been applied and painted, so it never flashes the defaults.
    // A window that was never shown doesn't paint, so its
    // requestAnimationFrame callbacks never run there — without the timeout
    // the preloaded window would report ready only after being shown, which
    // is exactly what the report unblocks.
    const configSync = appConfigStore.init({ restoreWindowSize: false });
    let mounted = true;
    void configSync.ready.finally(() => {
      if (!mounted) return;
      let reported = false;
      const report = () => {
        if (reported) return;
        reported = true;
        void preferencesReady();
      };
      requestAnimationFrame(() => requestAnimationFrame(report));
      setTimeout(report, 200);
    });
    return () => {
      mounted = false;
      configSync.stop();
    };
  });

  $effect(() => {
    if (!appConfigStore.loaded) return;
    void setBackgroundBlur(appConfigStore.backgroundBlur).catch(() => {});
  });
</script>

{#snippet chevronSpacer()}
  <span class="chevron-spacer"></span>
{/snippet}

<div class="window-chrome" class:bordered={!isWindows}>
  <TitleBar title={t("preferences.title")} showSettingsButton={false} showMaximizeButton={false} />
  <div class="body">
    <nav class="sidebar">
      {#each TREE as node (node.id)}
        <div
          class="sidebar-row"
          class:selected={selected === node.id || (!expanded[node.id] && containsSelected(node))}
        >
          {#if node.children}
            <button
              class="sidebar-chevron"
              class:expanded={expanded[node.id]}
              aria-label={t(expanded[node.id] ? "preferences.collapse" : "preferences.expand", { name: t(node.label) })}
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
          <button class="sidebar-item" onclick={() => select(node)}>{t(node.label)}</button>
        </div>

        {#if node.children && expanded[node.id]}
          <div class="sidebar-children" transition:slide={{ duration: 120 }}>
            {#each node.children as child (child.id)}
              <div class="sidebar-row child" class:selected={selected === child.id}>
                {@render chevronSpacer()}
                <button class="sidebar-item" onclick={() => (selected = child.id)}>
                  {#if child.tool}
                    <!-- The agent's own small logo, as in the "+" menu. -->
                    <span class="sidebar-icon"><ConsoleIcon kind={child.tool} size={12} /></span>
                  {/if}
                  {childLabel(child)}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      {/each}
    </nav>
    <div class="detail">
      {#if selected === "general"}
        <GeneralSection />
      {:else if selected === "appearance"}
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
      {:else if selected === "ai-notifications"}
        <AiNotificationsSection />
      {:else if selected === "ai-agents"}
        <AiAgentsSection />
      {:else if selected === "marketplace"}
        <MarketplaceSection />
      {:else if selected.startsWith("ai-tool-")}
        {#key selected}
          <AiToolSection tool={selected.slice("ai-tool-".length) as ToolId} />
        {/key}
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
    /* No background here: title bar, sidebar and detail pane each paint
       theirs once, so all of them match the opacity setting (see the same
       note on MainWindow's .window-chrome). */
    color: var(--titlebar-fg, #d4d4d4);
    box-sizing: border-box;
    border-radius: 10px;
    overflow: hidden;
  }

  /* Neutral translucent gray rather than a theme color, so the edge stays
     visible over both light and dark windows behind this one. Not on
     Windows: the native rounded window region (window_shape.rs) clips it
     into a broken-looking frame at the corners. */
  .window-chrome.bordered {
    border: 1px solid rgba(128, 128, 128, 0.45);
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

  .sidebar-icon {
    display: inline-flex;
    vertical-align: -2px;
    margin-right: 6px;
  }

  .sidebar-row.child .sidebar-item {
    font-size: 11px;
  }

  .detail {
    flex: 1;
    min-width: 0;
    background: var(--terminal-bg, #1e1e1e);
    overflow-y: auto;
    padding: 16px 20px;
    box-sizing: border-box;
  }

  /* Every scrolling area in this window — the detail pane, the sidebar, and
     the lists inside the font picker and the dropdowns — rather than each
     one styling its own. The engine's default draws a filled track and a
     stepper arrow at each end, which is what read as dated next to a
     console, where the same treatment as here is already applied to xterm's
     viewport (see SplitView). Only the thumb shows; the track is the pane
     it sits on, and the padding-box border keeps the thumb slim without
     changing the gutter's width. Neutral gray so it holds up on a light
     theme as well as a dark one. */
  .window-chrome :global(::-webkit-scrollbar) {
    width: 10px;
    height: 10px;
  }

  .window-chrome :global(::-webkit-scrollbar-track) {
    background: transparent;
  }

  .window-chrome :global(::-webkit-scrollbar-thumb) {
    background: rgba(128, 128, 128, 0.45);
    border: 2px solid transparent;
    background-clip: padding-box;
    border-radius: 5px;
  }

  .window-chrome :global(::-webkit-scrollbar-thumb:hover) {
    background: rgba(128, 128, 128, 0.7);
    background-clip: padding-box;
  }

  /* The square where the two bars meet, which would otherwise be filled. */
  .window-chrome :global(::-webkit-scrollbar-corner) {
    background: transparent;
  }
</style>
