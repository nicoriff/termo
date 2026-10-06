import { describe, expect, it } from "vitest";
import type { PlanStep } from "$lib/core/orchestration-client";
import { latestThought, stepCounts, stepLine, toolSteps } from "./planning-progress";

const tr = (key: string, params?: Record<string, string>) =>
  params ? `${key}(${Object.values(params).join(",")})` : key;

const steps: PlanStep[] = [
  { kind: "thinking", text: "Primero miro   la estructura" },
  { kind: "tool", name: "Glob", detail: "src/**/*.ts" },
  { kind: "tool", name: "Read", detail: "src/a.ts" },
  { kind: "tool", name: "Read", detail: "src/a.ts" },
  { kind: "tool", name: "Grep", detail: "Listing" },
  { kind: "text", text: "Ya entiendo el modelo de datos." },
  { kind: "tool", name: "Mystery", detail: "" },
];

describe("the orchestrator thinking", () => {
  it("words each tool as an action", () => {
    expect(stepLine(steps[2], tr)).toEqual({ icon: "📖", text: "agents.orchestration.thinking.tool.read(src/a.ts)" });
    expect(stepLine(steps[6], tr)).toEqual({ icon: "•", text: "Mystery" });
    expect(stepLine(steps[0], tr).icon).toBe("💭");
  });

  it("keeps the latest thought, tidied and short", () => {
    expect(latestThought(steps)).toBe("Ya entiendo el modelo de datos.");
    expect(latestThought(steps.slice(0, 2))).toBe("Primero miro la estructura");
    expect(latestThought([{ kind: "text", text: "x".repeat(10) }], 4)).toBe("xxxx…");
    expect(latestThought([])).toBe("");
  });

  it("counts what it looked at, each file once", () => {
    expect(stepCounts(steps)).toEqual({ files: 1, searches: 2 });
    expect(toolSteps(steps, 2).map((step) => (step.kind === "tool" ? step.name : ""))).toEqual(["Grep", "Mystery"]);
  });
});
