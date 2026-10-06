/**
 * The orchestrator's steps while it thinks — reading the request, planning,
 * revising — in words for the window waiting on it. Pure: the caller hands
 * in the translator.
 */

import type { PlanStep } from "$lib/core/orchestration-client";
import type { MessageKey } from "$lib/i18n/translate";

type Translate = (key: MessageKey, params?: Record<string, string>) => string;

/** What each of Claude Code's tools reads as, as an action in progress. */
const TOOL_ACTIONS: Record<string, { icon: string; key: MessageKey }> = {
  Read: { icon: "📖", key: "agents.orchestration.thinking.tool.read" },
  Grep: { icon: "🔎", key: "agents.orchestration.thinking.tool.search" },
  Glob: { icon: "🗂", key: "agents.orchestration.thinking.tool.list" },
  LS: { icon: "🗂", key: "agents.orchestration.thinking.tool.list" },
  Bash: { icon: "⌨", key: "agents.orchestration.thinking.tool.run" },
  PowerShell: { icon: "⌨", key: "agents.orchestration.thinking.tool.run" },
  WebFetch: { icon: "🌐", key: "agents.orchestration.thinking.tool.web" },
  WebSearch: { icon: "🌐", key: "agents.orchestration.thinking.tool.web" },
  Task: { icon: "🧩", key: "agents.orchestration.thinking.tool.delegate" },
};

/** A tool step as an icon and a line: "📖 Leyendo src/app/page.tsx". */
export function stepLine(step: PlanStep, tr: Translate): { icon: string; text: string } {
  if (step.kind !== "tool") return { icon: "💭", text: step.text };
  const action = TOOL_ACTIONS[step.name];
  if (!action) return { icon: "•", text: step.detail ? `${step.name} · ${step.detail}` : step.name };
  return { icon: action.icon, text: tr(action.key, { what: step.detail || "…" }) };
}

/** The last thing it said or thought, for the line under the spinner. */
export function latestThought(steps: PlanStep[], max = 280): string {
  for (let index = steps.length - 1; index >= 0; index--) {
    const step = steps[index];
    if (step.kind !== "tool" && step.text.trim()) {
      const text = step.text.replace(/\s+/g, " ").trim();
      return text.length > max ? `${text.slice(0, max)}…` : text;
    }
  }
  return "";
}

/** How much it looked at: files read, searches made. */
export function stepCounts(steps: PlanStep[]): { files: number; searches: number } {
  const files = new Set<string>();
  let searches = 0;
  for (const step of steps) {
    if (step.kind !== "tool") continue;
    if (step.name === "Read" && step.detail) files.add(step.detail);
    if (step.name === "Grep" || step.name === "Glob" || step.name === "LS") searches++;
  }
  return { files: files.size, searches };
}

/** The tool steps only, latest last: the trail under the thought. */
export function toolSteps(steps: PlanStep[], last: number): PlanStep[] {
  return steps.filter((step) => step.kind === "tool").slice(-last);
}
