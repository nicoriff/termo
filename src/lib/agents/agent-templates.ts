import type { AgentAutonomy, AgentSchedule, AgentSpec } from "$lib/core/agents-client";
import type { MessageKey } from "$lib/i18n/translate";

/** A ready-made starting point in the wizard's first step. Its texts are
 * catalog keys, so a template reads in the user's language. */
export interface AgentTemplate {
  id: string;
  icon: string;
  name: MessageKey;
  description: MessageKey;
  task: MessageKey;
  instructions: MessageKey;
  autonomy: AgentAutonomy;
  web: boolean;
  schedule: AgentSchedule | null;
  /** Works on a project: the current console's folder is a good default. */
  inProject: boolean;
}

const key = (id: string, field: string) => `agents.templates.${id}.${field}` as MessageKey;

function template(
  id: string,
  icon: string,
  options: Pick<AgentTemplate, "autonomy" | "web" | "schedule" | "inProject">,
): AgentTemplate {
  return {
    id,
    icon,
    name: key(id, "name"),
    description: key(id, "description"),
    task: key(id, "task"),
    instructions: key(id, "instructions"),
    ...options,
  };
}

export const AGENT_TEMPLATES: AgentTemplate[] = [
  template("reviewer", "🔍", { autonomy: "read", web: false, schedule: null, inProject: true }),
  template("dailySummary", "📊", {
    autonomy: "read",
    web: false,
    schedule: { kind: "daily", at: "09:00", weekdays: [1, 2, 3, 4, 5] },
    inProject: true,
  }),
  template("tidyFolder", "🧹", { autonomy: "run", web: false, schedule: { kind: "interval", minutes: 60 }, inProject: false }),
  template("docsKeeper", "📝", { autonomy: "edit", web: false, schedule: null, inProject: true }),
  template("researcher", "📚", { autonomy: "read", web: true, schedule: null, inProject: false }),
];

/** Fills `spec` from a template; `translate` turns its keys into text. */
export function applyTemplate(
  spec: AgentSpec,
  chosen: AgentTemplate,
  translate: (key: MessageKey) => string,
  here: string | null,
): void {
  spec.icon = chosen.icon;
  spec.name = translate(chosen.name);
  spec.description = translate(chosen.description);
  spec.task = translate(chosen.task);
  spec.instructions = translate(chosen.instructions);
  spec.autonomy = chosen.autonomy;
  spec.tools.web = chosen.web;
  spec.triggers.schedule = chosen.schedule ? structuredClone(chosen.schedule) : null;
  if (chosen.inProject && here && !spec.workDir) spec.workDir = here;
}
