import { describe, expect, it } from "vitest";
import type { RunEvent } from "$lib/core/agents-client";
import type { Catalog, Plan, PlannedAgent, PlannedTask } from "$lib/core/orchestration-client";
import {
  addAgent,
  addTask,
  agentHasRun,
  agentTotals,
  canDependOn,
  canEditPlan,
  changeLabel,
  costRange,
  defaultBudget,
  dependencyTitles,
  formatTokens,
  formatUsd,
  isTaskEditable,
  mergeRunLogs,
  planTotals,
  priceOf,
  progressOf,
  projectedUsd,
  removeAgent,
  removeTask,
  spentUsd,
  tasksWithoutRequest,
  uniqueKey,
  withAgentFields,
  withTaskFields,
  taskEstimate,
  treeOf,
  withInstructions,
  withModel,
  withTaskRequest,
} from "./orchestration-spec";

const catalog: Catalog = [
  {
    engine: "claude",
    installed: true,
    models: [
      { id: "claude-opus-5-5", name: "Opus 5.5", description: "", price: { input: 4, output: 20 } },
      { id: "claude-haiku-4-5", name: "Haiku 4.5", description: "", price: { input: 1, output: 5 } },
    ],
  },
  { engine: "copilot", installed: true, models: [{ id: "auto", name: "auto", description: "", price: null }] },
];

function agent(key: string, model: string, engine: PlannedAgent["engine"] = "claude"): PlannedAgent {
  return {
    key,
    name: key.toUpperCase(),
    icon: "🤖",
    specialty: "",
    engine,
    model,
    whyModel: "",
    autonomy: "edit",
    web: false,
    instructions: "",
    agentId: "",
  };
}

function task(key: string, agentKey: string, dependsOn: string[] = []): PlannedTask {
  return {
    key,
    title: `Task ${key}`,
    description: "",
    agent: agentKey,
    dependsOn,
    doneWhen: "",
    tokensIn: 1_000_000,
    tokensOut: 200_000,
    status: "pending",
    runId: "",
    summary: "",
    costUsd: null,
    attempts: 0,
  };
}

const plan: Plan = {
  agents: [agent("dev", "claude-haiku-4-5"), agent("rev", "auto", "copilot")],
  tasks: [task("a", "dev"), task("b", "dev", ["a"]), task("c", "rev", ["b", "ghost"])],
};

describe("estimates", () => {
  it("prices a task by its agent's model, or not at all", () => {
    expect(priceOf(catalog, "claude", "claude-opus-5-5")).toEqual({ input: 4, output: 20 });
    expect(priceOf(catalog, "claude", "")).toBeNull();
    // Haiku: 1M in at $1 + 200k out at $5.
    expect(taskEstimate(plan, plan.tasks[0], catalog)).toBeCloseTo(2);
    expect(taskEstimate(plan, plan.tasks[2], catalog)).toBeNull();
  });

  it("totals what has a price and counts what doesn't", () => {
    const totals = planTotals(plan, catalog);
    expect(totals).toMatchObject({ tasks: 3, tokensIn: 3_000_000, tokensOut: 600_000, unpriced: 1 });
    expect(totals.usd).toBeCloseTo(4);
    expect(agentTotals(plan, "rev", catalog)).toMatchObject({ tasks: 1, usd: 0, unpriced: 1 });
    expect(defaultBudget(plan, catalog)).toBe(5.2);
  });

  it("leaves dropped tasks out", () => {
    const dropped: Plan = { ...plan, tasks: [{ ...plan.tasks[0], status: "dropped" }, plan.tasks[1]] };
    expect(planTotals(dropped, catalog).tasks).toBe(1);
    expect(progressOf(dropped)).toEqual({ done: 0, total: 1, running: 0, failed: 0 });
  });

  it("recomputes when an agent changes model", () => {
    const changed = withModel(plan, "dev", "claude", "claude-opus-5-5");
    // Opus 5.5: 1M × $4 + 200k × $20 = $8 a task.
    expect(planTotals(changed, catalog).usd).toBeCloseTo(16);
    expect(plan.agents[0].model).toBe("claude-haiku-4-5");
  });

  it("projects with what's known for what ran", () => {
    const ran: Plan = {
      ...plan,
      tasks: [
        { ...plan.tasks[0], status: "done", costUsd: 0.5 },
        { ...plan.tasks[1], status: "failed", costUsd: 1 },
        plan.tasks[2],
      ],
    };
    expect(spentUsd(ran)).toBeCloseTo(1.5);
    // 0.5 known + (1 paid + 2 to run again) + nothing priced.
    expect(projectedUsd(ran, catalog)).toBeCloseTo(3.5);
  });
});

describe("the tree", () => {
  it("puts each agent's tasks under it", () => {
    const tree = treeOf(plan);
    expect(tree.map((branch) => [branch.agent.key, branch.tasks.map((item) => item.key)])).toEqual([
      ["dev", ["a", "b"]],
      ["rev", ["c"]],
    ]);
  });

  it("names what a task comes after", () => {
    expect(dependencyTitles(plan, plan.tasks[2])).toEqual(["Task b", "ghost"]);
  });
});

describe("words", () => {
  it("formats money and tokens", () => {
    expect(formatUsd(1.234)).toBe("US$ 1.23");
    expect(formatUsd(0.042)).toBe("US$ 0.042");
    expect(formatUsd(0)).toBe("US$ 0.00");
    expect(formatTokens(1_250_000)).toBe("1.3M");
    expect(formatTokens(2_000_000)).toBe("2M");
    expect(formatTokens(42_000)).toBe("42k");
    expect(formatTokens(900)).toBe("900");
    expect(costRange(10)).toEqual({ low: 6, high: 16 });
  });

  it("describes a revision's changes by name", () => {
    expect(changeLabel({ kind: "reassign", task: "a", agent: "rev" }, plan)).toEqual({
      key: "agents.orchestration.change.reassign",
      params: { task: "Task a", agent: "REV" },
    });
    expect(changeLabel({ kind: "changeModel", agent: "dev", engine: "codex", model: "gpt-5.5" }, plan).params).toEqual({
      agent: "DEV",
      model: "Codex · gpt-5.5",
    });
    expect(changeLabel({ kind: "dropTask", task: "gone" }, plan).params).toEqual({ task: "gone" });
  });
});

describe("editing a plan", () => {
  it("changes one agent's instructions and one task's request, nothing else", () => {
    const withText = withInstructions(plan, "rev", "Look for bugs only.");
    expect(withText.agents[1].instructions).toBe("Look for bugs only.");
    expect(withText.agents[0]).toBe(plan.agents[0]);
    const withRequest = withTaskRequest(plan, "b", "Do b well", "tests pass");
    expect(withRequest.tasks[1]).toMatchObject({ description: "Do b well", doneWhen: "tests pass" });
    expect(withRequest.tasks[0]).toBe(plan.tasks[0]);
  });

  it("only touches what hasn't run, and only when nothing is running", () => {
    expect(isTaskEditable({ ...plan.tasks[0], status: "pending" })).toBe(true);
    expect(isTaskEditable({ ...plan.tasks[0], status: "failed" })).toBe(true);
    expect(isTaskEditable({ ...plan.tasks[0], status: "done" })).toBe(false);
    expect(isTaskEditable({ ...plan.tasks[0], status: "running" })).toBe(false);
    expect(["stopped", "paused", "failed", "done"].every((status) => canEditPlan(status as never))).toBe(true);
    expect(canEditPlan("running")).toBe(false);
    expect(canEditPlan("draft")).toBe(false);
    expect(canEditPlan("awaitingApproval")).toBe(false);
  });
});

describe("changing the team", () => {
  it("adds an agent and a task with fresh keys, the task after the agent's last", () => {
    expect(uniqueKey("agent", ["agent", "agent-2"])).toBe("agent-3");
    const added = addAgent(plan, "claude", "Tester");
    expect(added.key).toBe("agent");
    expect(added.plan.agents.at(-1)).toMatchObject({ key: "agent", name: "Tester", autonomy: "read", model: "" });
    const withTask = addTask(plan, "dev", "Write tests");
    expect(withTask.plan.tasks.at(-1)).toMatchObject({ agent: "dev", dependsOn: ["b"], status: "pending" });
    // The fixture's own tasks have no request either; the new one is listed.
    expect(tasksWithoutRequest(withTask.plan).map((task) => task.key)).toContain("task");
    const asked = { ...withTask.plan, tasks: withTask.plan.tasks.map((task) => ({ ...task, description: "do it" })) };
    expect(tasksWithoutRequest(asked)).toEqual([]);
  });

  it("removes an agent with its tasks, and a task from what waited for it", () => {
    const withoutDev = removeAgent(plan, "dev");
    expect(withoutDev.agents.map((agent) => agent.key)).toEqual(["rev"]);
    expect(withoutDev.tasks.map((task) => [task.key, task.dependsOn])).toEqual([["c", ["ghost"]]]);
    expect(removeTask(plan, "b").tasks.find((task) => task.key === "c")?.dependsOn).toEqual(["ghost"]);
  });

  it("knows when an agent has run, and which waits would loop", () => {
    const ran: Plan = { ...plan, tasks: plan.tasks.map((task) => (task.key === "a" ? { ...task, status: "done" } : task)) };
    expect(agentHasRun(ran, "dev")).toBe(true);
    expect(agentHasRun(ran, "rev")).toBe(false);
    // c waits for b, which waits for a: a can't wait for c.
    expect(canDependOn(plan, "a", "c")).toBe(false);
    expect(canDependOn(plan, "c", "a")).toBe(true);
    expect(canDependOn(plan, "a", "a")).toBe(false);
  });

  it("edits an agent's and a task's own fields only", () => {
    const renamed = withAgentFields(plan, "rev", { name: "Reviewer", autonomy: "run", web: true });
    expect(renamed.agents[1]).toMatchObject({ name: "Reviewer", autonomy: "run", web: true });
    const moved = withTaskFields(plan, "c", { agent: "dev", dependsOn: [] });
    expect(moved.tasks[2]).toMatchObject({ agent: "dev", dependsOn: [] });
    expect(moved.tasks[0]).toBe(plan.tasks[0]);
  });
});

describe("the console", () => {
  const step = (at: number, text: string, kind: RunEvent["kind"] = "text"): RunEvent => ({ kind, text, at });

  it("puts every run's steps in the order they happened, once", () => {
    const lines = mergeRunLogs([
      { source: "r1", events: [step(1, "a"), step(5, "c"), step(5, "c")] },
      { source: "r2", events: [step(3, "b"), step(5, "d")] },
    ]);
    expect(lines.map((line) => `${line.source}:${line.event.text}`)).toEqual(["r1:a", "r2:b", "r1:c", "r2:d"]);
  });

  it("doesn't repeat a result as text", () => {
    const lines = mergeRunLogs([{ source: "r", events: [step(1, "done"), step(2, "done", "result")] }]);
    expect(lines.map((line) => line.event.kind)).toEqual(["result"]);
  });
});
