import { describe, expect, it } from "vitest";
import type { MessageKey } from "$lib/i18n/translate";
import { es } from "$lib/i18n/messages/es";
import { en } from "$lib/i18n/messages/en";
import { PROJECT_TEMPLATES, buildFromTemplate } from "./project-templates";

function lookup(catalog: unknown, key: string): unknown {
  return key.split(".").reduce<unknown>((node, part) => (node as Record<string, unknown> | undefined)?.[part], catalog);
}

describe("project templates", () => {
  for (const catalog of [es, en]) {
    it("have every text they use", () => {
      for (const template of PROJECT_TEMPLATES) {
        const missing: string[] = [];
        const translate = (key: MessageKey) => {
          const text = lookup(catalog, key);
          if (typeof text !== "string" || !text.trim()) missing.push(key);
          return String(text);
        };
        buildFromTemplate(template, translate, "", () => crypto.randomUUID());
        expect(missing).toEqual([]);
      }
    });
  }

  it("wire their agents to each other and arrive paused", () => {
    let n = 0;
    for (const template of PROJECT_TEMPLATES) {
      const { project, agents } = buildFromTemplate(template, (key) => key, "C:\\repo", () => `id${++n}`);
      const ids = new Set(agents.map((agent) => agent.id));
      expect(ids.has(project.entryAgentId)).toBe(true);
      expect(project.isolated).toBe(true);
      for (const agent of agents) {
        expect(agent.enabled).toBe(false);
        expect(agent.projectId).toBe(project.id);
        expect(agent.workDir).toBe("C:\\repo");
        if (agent.triggers.after) expect(ids.has(agent.triggers.after.agentId)).toBe(true);
        for (const contact of agent.collaboration.contacts) expect(ids.has(contact)).toBe(true);
      }
    }
  });
});
