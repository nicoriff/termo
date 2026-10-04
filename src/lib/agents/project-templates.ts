import type { AfterOutcome, AgentAutonomy, AgentSchedule, AgentSpec, Project } from "$lib/core/agents-client";
import type { MessageKey } from "$lib/i18n/translate";
import { newAgentSpec } from "./agent-spec";
import { newProject } from "./project-spec";

/** One agent of a project template. Its texts are catalog keys, so the
 * template reads in the user's language; other members are named by key. */
interface TemplateMember {
  key: string;
  icon: string;
  autonomy: AgentAutonomy;
  web?: boolean;
  schedule?: AgentSchedule;
  contacts?: string[];
  after?: { key: string; on: AfterOutcome };
  board?: boolean;
}

export interface ProjectTemplate {
  id: string;
  icon: string;
  entry: string;
  members: TemplateMember[];
}

export const PROJECT_TEMPLATES: ProjectTemplate[] = [
  {
    id: "prPipeline",
    icon: "🔀",
    entry: "reviewer",
    members: [
      { key: "reviewer", icon: "🔍", autonomy: "read" },
      { key: "tester", icon: "🧪", autonomy: "run", after: { key: "reviewer", on: "success" } },
      { key: "documenter", icon: "📝", autonomy: "edit", after: { key: "tester", on: "success" } },
    ],
  },
  {
    id: "research",
    icon: "🔎",
    entry: "researcher",
    members: [
      { key: "researcher", icon: "📚", autonomy: "read", web: true },
      { key: "writer", icon: "✍️", autonomy: "edit", after: { key: "researcher", on: "success" } },
      { key: "checker", icon: "✅", autonomy: "read", web: true, after: { key: "writer", on: "success" } },
    ],
  },
  {
    id: "repoCare",
    icon: "🧰",
    entry: "dependencies",
    members: [
      {
        key: "dependencies",
        icon: "📦",
        autonomy: "read",
        web: true,
        board: true,
        schedule: { kind: "daily", at: "09:00", weekdays: [1, 2, 3, 4, 5] },
      },
      { key: "tests", icon: "🧪", autonomy: "run", after: { key: "dependencies", on: "any" } },
      { key: "changelog", icon: "📝", autonomy: "edit", board: true, after: { key: "tests", on: "success" } },
    ],
  },
  {
    id: "weeklyReport",
    icon: "📰",
    entry: "collector",
    members: [
      {
        key: "collector",
        icon: "📊",
        autonomy: "read",
        board: true,
        schedule: { kind: "daily", at: "09:00", weekdays: [1] },
        contacts: ["writer"],
      },
      { key: "writer", icon: "✍️", autonomy: "edit", board: true, after: { key: "collector", on: "success" } },
    ],
  },
];

const key = (template: string, field: string) => `agents.projects.templates.${template}.${field}` as MessageKey;
const memberKey = (template: string, member: string, field: string) =>
  `agents.projects.templates.${template}.agents.${member}.${field}` as MessageKey;

export const templateName = (template: ProjectTemplate) => key(template.id, "name");
export const templateDescription = (template: ProjectTemplate) => key(template.id, "description");

/** A new project and its agents from a template, not saved yet: paused, in
 * `workDir`, a team with a daily limit. */
export function buildFromTemplate(
  template: ProjectTemplate,
  translate: (key: MessageKey) => string,
  workDir: string,
  newId: () => string,
): { project: Project; agents: AgentSpec[] } {
  const project: Project = {
    ...newProject(newId()),
    name: translate(templateName(template)),
    icon: template.icon,
    goal: translate(templateDescription(template)),
    workDir,
    isolated: true,
    limits: { maxRunsPerDay: 30 },
    orchestrationId: "",
  };
  const ids = new Map(template.members.map((member) => [member.key, newId()]));
  const agents = template.members.map((member) => {
    const spec = newAgentSpec(ids.get(member.key)!);
    spec.name = translate(memberKey(template.id, member.key, "name"));
    spec.description = translate(memberKey(template.id, member.key, "description"));
    spec.task = translate(memberKey(template.id, member.key, "task"));
    spec.instructions = translate(memberKey(template.id, member.key, "instructions"));
    spec.icon = member.icon;
    spec.autonomy = member.autonomy;
    spec.workDir = workDir;
    spec.tools.web = member.web ?? false;
    spec.triggers.schedule = member.schedule ? structuredClone(member.schedule) : null;
    spec.triggers.after = member.after ? { agentId: ids.get(member.after.key)!, on: member.after.on } : null;
    spec.collaboration.contacts = (member.contacts ?? []).map((contact) => ids.get(contact)!);
    spec.collaboration.board = member.board ?? false;
    spec.enabled = false;
    spec.projectId = project.id;
    return spec;
  });
  project.entryAgentId = ids.get(template.entry)!;
  return { project, agents };
}
