import { describe, expect, it } from "vitest";
import type { AgentSpec, RunRecord } from "$lib/core/agents-client";
import { newAgentSpec } from "./agent-spec";
import {
  estimatedRunsPerDay,
  groupByProject,
  layoutGraph,
  newProject,
  projectTone,
  projectWarnings,
  scheduleRunsPerDay,
} from "./project-spec";

function agent(id: string, changes: (spec: AgentSpec) => void = () => {}): AgentSpec {
  const spec = newAgentSpec(id);
  spec.name = id.toUpperCase();
  changes(spec);
  return spec;
}

function run(status: RunRecord["status"]): RunRecord {
  return {
    id: "r",
    agentId: "a",
    trigger: "manual",
    status,
    startedAt: 0,
    endedAt: null,
    summary: "",
    activity: "",
    costUsd: null,
    link: null,
    rehearsal: false,
  };
}

describe("projectTone", () => {
  it("says running before failing before paused", () => {
    const a = agent("a");
    const b = agent("b", (spec) => (spec.enabled = false));
    expect(projectTone([a, b], { a: run("running"), b: run("error") })).toBe("running");
    expect(projectTone([a, b], { a: run("success"), b: run("timedOut") })).toBe("error");
    expect(projectTone([b], {})).toBe("paused");
    expect(projectTone([a], { a: run("success") })).toBe("success");
    expect(projectTone([], {})).toBe("idle");
  });
});

describe("estimatedRunsPerDay", () => {
  it("counts schedules and the chains they set off", () => {
    expect(scheduleRunsPerDay({ kind: "interval", minutes: 60 })).toBe(24);
    expect(scheduleRunsPerDay({ kind: "daily", at: "09:00", weekdays: [] })).toBe(1);
    const lead = agent("a", (spec) => (spec.triggers.schedule = { kind: "interval", minutes: 120 }));
    const next = agent("b", (spec) => (spec.triggers.after = { agentId: "a", on: "success" }));
    const last = agent("c", (spec) => (spec.triggers.after = { agentId: "b", on: "any" }));
    expect(estimatedRunsPerDay([lead, next, last])).toBe(36);
  });

  it("survives a loop", () => {
    const a = agent("a", (spec) => (spec.triggers.after = { agentId: "b", on: "any" }));
    const b = agent("b", (spec) => (spec.triggers.after = { agentId: "a", on: "any" }));
    expect(estimatedRunsPerDay([a, b])).toBe(0);
  });
});

describe("projectWarnings", () => {
  it("points at escalations, commands on outside text and a missing entry", () => {
    const reviewer = agent("r", (spec) => {
      spec.collaboration.contacts = ["t"];
      spec.collaboration.allowEscalation = true;
    });
    const tester = agent("t", (spec) => (spec.autonomy = "run"));
    const project = { ...newProject("p"), isolated: true };
    const keys = projectWarnings(project, [reviewer, tester]).map((warning) => warning.key);
    expect(keys).toEqual([
      "agents.projects.warnings.escalation",
      "agents.projects.warnings.runsCommands",
      "agents.projects.warnings.noEntry",
      "agents.projects.warnings.noLimit",
    ]);
    const settled = { ...project, entryAgentId: "r", limits: { maxRunsPerDay: 10 } };
    expect(projectWarnings(settled, [reviewer, tester]).map((warning) => warning.key)).toContain(
      "agents.projects.warnings.entryManual",
    );
  });

  it("has nothing to say about an empty folder", () => {
    expect(projectWarnings(newProject("p"), [])).toEqual([]);
  });
});

describe("layoutGraph", () => {
  it("puts each agent one column right of whoever sets it off", () => {
    const a = agent("a", (spec) => (spec.collaboration.contacts = ["b"]));
    const b = agent("b");
    const c = agent("c", (spec) => (spec.triggers.after = { agentId: "a", on: "success" }));
    const loose = agent("d");
    const { nodes, edges } = layoutGraph([a, b, c, loose], "a");
    const columnOf = (id: string) => nodes.find((node) => node.agent.id === id)!.x;
    expect(columnOf("a")).toBe(0);
    expect(columnOf("b")).toBe(columnOf("c"));
    expect(columnOf("b")).toBeGreaterThan(0);
    expect(columnOf("d")).toBe(0);
    expect(edges).toEqual([
      { from: "a", to: "b", kind: "asks" },
      { from: "a", to: "c", kind: "after" },
    ]);
  });
});

describe("groupByProject", () => {
  it("keeps agents of unknown projects with the loose ones", () => {
    const groups = groupByProject(
      [agent("a", (spec) => (spec.projectId = "p")), agent("b", (spec) => (spec.projectId = "gone")), agent("c")],
      [newProject("p"), newProject("empty")],
    );
    expect(groups.get("p")!.map((spec) => spec.id)).toEqual(["a"]);
    expect(groups.get("empty")).toEqual([]);
    expect(groups.get("")!.map((spec) => spec.id)).toEqual(["b", "c"]);
  });
});
