import type { AgentAutonomy, AgentSchedule, AgentSpec, Project, RunRecord } from "$lib/core/agents-client";
import type { MessageKey } from "$lib/i18n/translate";
import type { AgentTone } from "./agent-spec";

export function newProject(id: string): Project {
  return {
    id,
    name: "",
    icon: "📁",
    goal: "",
    workDir: "",
    isolated: false,
    entryAgentId: "",
    limits: { maxRunsPerDay: 0 },
    createdAt: 0,
    updatedAt: 0,
  };
}

export const PROJECT_ICONS = ["📁", "🧪", "🔀", "🔎", "🧰", "📰", "🚀", "🛡️", "📦", "🧠"];

/** One word for a whole group in the sidebar: busy if any agent runs, in
 * trouble if any last run failed, paused if every agent is. */
export function projectTone(agents: AgentSpec[], lastRun: Record<string, RunRecord | undefined>): AgentTone {
  if (agents.length === 0) return "idle";
  if (agents.some((agent) => lastRun[agent.id]?.status === "running")) return "running";
  const failed = (status: RunRecord["status"] | undefined) => status === "error" || status === "timedOut";
  if (agents.some((agent) => failed(lastRun[agent.id]?.status))) return "error";
  if (agents.every((agent) => !agent.enabled)) return "paused";
  if (agents.some((agent) => lastRun[agent.id]?.status === "success")) return "success";
  return "idle";
}

/** How many times a day a schedule runs. */
export function scheduleRunsPerDay(schedule: AgentSchedule | null): number {
  if (!schedule) return 0;
  if (schedule.kind === "interval") return 1440 / Math.max(1, schedule.minutes);
  return schedule.weekdays.length === 0 ? 1 : schedule.weekdays.length / 7;
}

/** A rough count of runs a day from the agents' schedules and the chains
 * they set off (as if every run ended well). What agents ask each other
 * can't be known beforehand, so it isn't counted. */
export function estimatedRunsPerDay(agents: AgentSpec[]): number {
  const own = new Map(agents.map((agent) => [agent.id, scheduleRunsPerDay(agent.triggers.schedule)]));
  const total = new Map<string, number>();
  const runsOf = (agent: AgentSpec, seen: string[]): number => {
    if (total.has(agent.id)) return total.get(agent.id)!;
    if (seen.includes(agent.id)) return 0;
    const leaderId = agent.triggers.after?.agentId;
    const leader = agents.find((candidate) => candidate.id === leaderId);
    const runs = (own.get(agent.id) ?? 0) + (leader ? runsOf(leader, [...seen, agent.id]) : 0);
    total.set(agent.id, runs);
    return runs;
  };
  const sum = agents.reduce((acc, agent) => acc + runsOf(agent, []), 0);
  return Math.round(sum * 10) / 10;
}

const RANK: Record<AgentAutonomy, number> = { read: 0, edit: 1, run: 2 };

export interface ProjectWarning {
  key: MessageKey;
  params?: Record<string, string>;
  /** Something to look at before turning it on, rather than a note. */
  serious: boolean;
}

/** What deserves a look before turning a project on. */
export function projectWarnings(project: Project, agents: AgentSpec[]): ProjectWarning[] {
  const warnings: ProjectWarning[] = [];
  const named = (id: string) => agents.find((agent) => agent.id === id);
  const askedBySomeone = (id: string) => agents.some((agent) => agent.collaboration.contacts.includes(id));
  for (const agent of agents) {
    const stronger = agent.collaboration.contacts
      .map(named)
      .filter((other): other is AgentSpec => Boolean(other) && RANK[other!.autonomy] > RANK[agent.autonomy]);
    if (stronger.length > 0 && agent.collaboration.allowEscalation) {
      warnings.push({
        key: "agents.projects.warnings.escalation",
        params: { agent: agent.name, names: stronger.map((other) => other.name).join(", ") },
        serious: true,
      });
    }
    const outside = Boolean(
      agent.triggers.git || agent.triggers.webhook || agent.triggers.after || agent.tools.web || askedBySomeone(agent.id),
    );
    if (agent.autonomy === "run" && outside) {
      warnings.push({ key: "agents.projects.warnings.runsCommands", params: { agent: agent.name }, serious: true });
    }
  }
  if (agents.length > 0) {
    const entry = named(project.entryAgentId);
    if (!entry) {
      warnings.push({ key: "agents.projects.warnings.noEntry", serious: false });
    } else {
      const triggers = entry.triggers;
      if (!triggers.schedule && !triggers.watch && !triggers.git && !triggers.webhook && !triggers.after) {
        warnings.push({ key: "agents.projects.warnings.entryManual", params: { agent: entry.name }, serious: false });
      }
    }
  }
  if (project.isolated && project.limits.maxRunsPerDay === 0 && agents.length > 1) {
    warnings.push({ key: "agents.projects.warnings.noLimit", serious: false });
  }
  return warnings;
}

export interface GraphNode {
  agent: AgentSpec;
  x: number;
  y: number;
}

export interface GraphEdge {
  from: string;
  to: string;
  /** "after": `to` runs when `from` finishes; "asks": `from` can ask `to`. */
  kind: "after" | "asks";
}

export const GRAPH = { nodeWidth: 150, nodeHeight: 46, columnGap: 70, rowGap: 22 };

/** Lays the team out in columns: the entry agent first, then each agent one
 * column right of whoever sets it off (the one it follows, or one that can
 * ask it). Agents nothing reaches go in the first column. */
export function layoutGraph(agents: AgentSpec[], entryId: string): { nodes: GraphNode[]; edges: GraphEdge[]; width: number; height: number } {
  const ids = new Set(agents.map((agent) => agent.id));
  const edges: GraphEdge[] = [];
  for (const agent of agents) {
    const leader = agent.triggers.after?.agentId;
    if (leader && ids.has(leader) && leader !== agent.id) edges.push({ from: leader, to: agent.id, kind: "after" });
    for (const contact of agent.collaboration.contacts) {
      if (ids.has(contact) && contact !== agent.id) edges.push({ from: agent.id, to: contact, kind: "asks" });
    }
  }
  const column = new Map<string, number>();
  const starts = [entryId, ...agents.map((agent) => agent.id)].filter((id) => ids.has(id));
  for (const start of starts) {
    if (column.has(start)) continue;
    column.set(start, 0);
    const queue = [start];
    while (queue.length > 0) {
      const current = queue.shift()!;
      for (const edge of edges.filter((candidate) => candidate.from === current)) {
        if (!column.has(edge.to)) {
          column.set(edge.to, column.get(current)! + 1);
          queue.push(edge.to);
        }
      }
    }
  }
  const rows = new Map<number, number>();
  const nodes = agents.map((agent) => {
    const col = column.get(agent.id) ?? 0;
    const row = rows.get(col) ?? 0;
    rows.set(col, row + 1);
    return {
      agent,
      x: col * (GRAPH.nodeWidth + GRAPH.columnGap),
      y: row * (GRAPH.nodeHeight + GRAPH.rowGap),
    };
  });
  const columns = Math.max(1, ...[...column.values()].map((col) => col + 1));
  const maxRows = Math.max(1, ...rows.values());
  return {
    nodes,
    edges,
    width: columns * GRAPH.nodeWidth + (columns - 1) * GRAPH.columnGap,
    height: maxRows * GRAPH.nodeHeight + (maxRows - 1) * GRAPH.rowGap,
  };
}

/** The agents of each project, and the ones in none (under ""), in the
 * order they come. */
export function groupByProject(agents: AgentSpec[], projects: Project[]): Map<string, AgentSpec[]> {
  const known = new Set(projects.map((project) => project.id));
  const groups = new Map<string, AgentSpec[]>(projects.map((project) => [project.id, []]));
  groups.set("", []);
  for (const agent of agents) {
    const key = known.has(agent.projectId) ? agent.projectId : "";
    groups.get(key)!.push(agent);
  }
  return groups;
}
