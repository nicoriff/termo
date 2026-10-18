import { invoke } from "@tauri-apps/api/core";
import { agentStatusStore } from "$lib/ai-mode/agent-status-store.svelte";
import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
import { ptyIdsByTab } from "$lib/terminal/pty-registry";

/** How often it looks for what changed. */
const EVERY_MS = 1500;

/** One console's agent, as src-tauri/src/mux/protocol.rs's AgentInfo. */
interface AgentInfo {
  tool: string;
  status: string;
  planWindows: { windowMinutes: number | null; percent: number }[];
  contextPercent: number | null;
  costUsd: number | null;
}

/** What this window's status bar knows of the agent in each console. */
function agentsNow(): Map<string, AgentInfo> {
  const agents = new Map<string, AgentInfo>();
  for (const [tabId, ptyId] of Object.entries(ptyIdsByTab())) {
    const entry = aiStatusStore.byId[tabId];
    if (!entry?.tool || !entry.status) continue;
    const usage = agentStatusStore.byTab[tabId];
    agents.set(ptyId, {
      tool: TOOL_DISPLAY_NAMES[entry.tool],
      status: entry.status,
      planWindows: (usage?.planWindows ?? []).map((window) => ({
        windowMinutes: window.windowMinutes,
        percent: window.percentage,
      })),
      contextPercent: usage?.contextUsedPercentage ?? null,
      costUsd: usage?.costUsd ?? null,
    });
  }
  return agents;
}

/**
 * Tells the backend what the status bar knows of each console's agent —
 * its state, the plan's usage, the context and the cost —, so a phone
 * showing the console from the tailnet can show it too (see
 * src-tauri/src/pty/sharing.rs). Only what changed goes. Returns a teardown.
 */
export function startAgentSharing(): () => void {
  let sent = new Map<string, string>();
  const tick = () => {
    const now = agentsNow();
    const changed: { id: string; agent: AgentInfo | null }[] = [];
    const next = new Map<string, string>();
    for (const [id, agent] of now) {
      const text = JSON.stringify(agent);
      next.set(id, text);
      if (sent.get(id) !== text) changed.push({ id, agent });
    }
    for (const id of sent.keys()) {
      if (!now.has(id)) changed.push({ id, agent: null });
    }
    sent = next;
    if (changed.length) void invoke("remote_share_agents", { agents: changed }).catch(() => {});
  };
  tick();
  const timer = setInterval(tick, EVERY_MS);
  return () => clearInterval(timer);
}
