import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { resolveDefaultShell } from "$lib/core/default-shell";
import { resolveShellArgs } from "$lib/core/shell-args";
import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
import { agentSessionsStore } from "$lib/ai-mode/agent-sessions-store.svelte";
import { samePath } from "$lib/agent-launch/folder-nav";
import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
import { collectTiles } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { getTabId, ptyIdsByTab } from "$lib/terminal/pty-registry";
import { killPty } from "$lib/core/pty-client";

/** A shell or an agent to open from the tailnet, as
 * src-tauri/src/mux/protocol.rs's OpenChoice. */
interface OpenChoice {
  id: string;
  name: string;
  kind: "shell" | "agent";
}

/** What the tailnet asked for (see src-tauri/src/lib.rs). */
interface OpenRequest {
  request: number;
  choice: string;
  cwd: string | null;
}

/** How long a new tab's console may take to start. */
const START_WAIT_MS = 10_000;
/** The recent folders offered, as many as the agent-launch dialog shows. */
const RECENT_LIMIT = 5;

/** What the new-tab menu offers: the shells, and the agents installed (Modo
 * IA's, so only with it on). */
function choicesNow(): OpenChoice[] {
  const shells: OpenChoice[] = newTabOptions.shells.map((shell) => ({
    id: `shell:${shell.id}`,
    name: shell.display_name,
    kind: "shell",
  }));
  const agents: OpenChoice[] = appConfigStore.aiModeEnabled
    ? TOOL_IDS.filter((tool) => newTabOptions.agentClis?.[tool] !== false).map((tool) => ({
        id: `agent:${tool}`,
        name: TOOL_DISPLAY_NAMES[tool],
        kind: "agent",
      }))
    : [];
  return [...shells, ...agents];
}

/** The folders agents ran in lately, as the agent-launch dialog offers them
 * (see AgentLaunchDialog's rootItems). */
function recentFolders(): string[] {
  const recent: string[] = [];
  for (const session of agentSessionsStore.sessions) {
    const path = session.cwd ?? session.projectDir;
    if (!path || recent.some((other) => samePath(other, path))) continue;
    recent.push(path);
    if (recent.length >= RECENT_LIMIT) break;
  }
  return recent;
}

/** Opens it as a new tab of the focused tile; its id once its console
 * started there. */
async function openTab(asked: OpenRequest): Promise<string> {
  const root = workspaceStore.root;
  const tileId = workspaceStore.focusedTileId ?? (root ? collectTiles(root)[0]?.id : undefined);
  if (!tileId) throw new Error("no-window");
  const cwd = asked.cwd ?? undefined;
  const split = asked.choice.indexOf(":");
  const kind = asked.choice.slice(0, split);
  const id = asked.choice.slice(split + 1);
  let tabId: string | null = null;
  if (kind === "shell") {
    const shell = newTabOptions.shells.find((candidate) => candidate.id === id);
    if (!shell) throw new Error("unknown-choice");
    const args = resolveShellArgs(shell.id, shell.args, appConfigStore.psreadlineMenuComplete);
    tabId = workspaceStore.addTab(tileId, shell.path, args, shell.display_name, cwd);
  } else if (kind === "agent" && (TOOL_IDS as readonly string[]).includes(id)) {
    const shell = await resolveDefaultShell();
    tabId = workspaceStore.addTab(tileId, shell.path, shell.args, shell.title, cwd, { tool: id as ToolId });
  } else {
    throw new Error("unknown-choice");
  }
  if (!tabId) throw new Error("no-window");
  const deadline = Date.now() + START_WAIT_MS;
  for (;;) {
    const console = ptyIdsByTab()[tabId];
    if (console) return console;
    if (Date.now() > deadline) throw new Error("timeout");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/**
 * Lets the tailnet open consoles here (a phone, from the web client): tells
 * the backend what the new-tab menu offers, and opens what's asked as a new
 * tab, like the menu would — the daemon then shows it there (see
 * src-tauri/src/pty/sharing.rs); and closes the tab of one closed from
 * there. Only the window Termo opens with does it.
 * Returns a teardown.
 */
export function startRemoteOpen(): () => void {
  const stopChoices = $effect.root(() => {
    $effect(() => {
      const choices = choicesNow();
      const recent = recentFolders();
      void invoke("remote_set_open_choices", { choices, recent }).catch(() => {});
    });
  });
  const unlisten = listen<OpenRequest>("remote-open", (event) => {
    const asked = event.payload;
    openTab(asked).then(
      (console) => invoke("remote_opened", { request: asked.request, console, error: null }),
      (err: unknown) =>
        invoke("remote_opened", {
          request: asked.request,
          console: null,
          error: err instanceof Error ? err.message : String(err),
        }),
    ).catch(() => {});
  });
  // Closed from there: its tab closes, as if closed here. One no tab of this
  // window shows (another window's, the only tab left) just ends.
  const unlistenClose = listen<string>("remote-close", (event) => {
    const console = event.payload;
    const tabId = getTabId(console);
    if (tabId && workspaceStore.closeSession(tabId)) return;
    void killPty(console).catch(() => {});
  });
  return () => {
    stopChoices();
    void unlisten.then((stop) => stop());
    void unlistenClose.then((stop) => stop());
  };
}
