import { describe, expect, it } from "vitest";
import type { Catalog, Plan, PlannedAgent, PlannedTask } from "$lib/core/orchestration-client";
import {
  agentTotals,
  changeLabel,
  costRange,
  defaultBudget,
  dependencyTitles,
  formatTokens,
  formatUsd,
  planTotals,
  priceOf,
  progressOf,
  projectedUsd,
  spentUsd,
  taskEstimate,
  treeOf,
  withModel,
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
