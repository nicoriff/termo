import { describe, expect, it } from "vitest";
import type { RunRecord } from "$lib/core/agents-client";
import { agentState, ago, formatDuration, newAgentSpec, specProblems, stepProblems, withoutEchoes } from "./agent-spec";

function run(status: RunRecord["status"], activity = ""): RunRecord {
  return { id: "r", agentId: "a", trigger: "manual", status, startedAt: 0, endedAt: null, summary: "", activity, costUsd: null };
}

describe("stepProblems", () => {
  it("asks for a name and a task before leaving the logic step", () => {
    const spec = newAgentSpec("a");
    expect(stepProblems("logic", spec)).toEqual(["agents.wizard.problems.name", "agents.wizard.problems.task"]);
    spec.name = "Revisor";
    spec.task = "Revisá el PR";
    expect(stepProblems("logic", spec)).toEqual([]);
  });

  it("checks the schedule's shape", () => {
    const spec = newAgentSpec("a");
    spec.triggers.schedule = { kind: "daily", at: "9:00", weekdays: [] };
    expect(stepProblems("triggers", spec)).toEqual(["agents.wizard.problems.time"]);
    spec.triggers.schedule = { kind: "daily", at: "09:00", weekdays: [] };
    expect(stepProblems("triggers", spec)).toEqual([]);
    spec.triggers.schedule = { kind: "interval", minutes: 0 };
    expect(stepProblems("triggers", spec)).toEqual(["agents.wizard.problems.interval"]);
  });

  it("a fresh spec only lacks what the user must write", () => {
    expect(specProblems(newAgentSpec("a"))).toEqual(["agents.wizard.problems.name", "agents.wizard.problems.task"]);
  });
});

describe("agentState", () => {
  const spec = newAgentSpec("a");

  it("says what a running agent is doing", () => {
    expect(agentState(spec, run("running", "Bash · ls"))).toEqual({
      tone: "running",
      key: "agents.state.runningDoing",
      params: { activity: "Bash · ls" },
    });
  });

  it("reports the last run, or that there was none", () => {
    expect(agentState(spec, null).key).toBe("agents.state.never");
    expect(agentState(spec, run("success")).tone).toBe("success");
    expect(agentState(spec, run("timedOut")).tone).toBe("error");
  });

  it("a paused agent says so unless it's running right now", () => {
    const paused = { ...spec, enabled: false };
    expect(agentState(paused, run("success")).tone).toBe("paused");
    expect(agentState(paused, run("running")).tone).toBe("running");
  });
});

describe("withoutEchoes", () => {
  it("keeps the answer once, as the result", () => {
    const events = [
      { kind: "text" as const, text: "Voy a mirar.", at: 0 },
      { kind: "text" as const, text: "Hay 21.", at: 1 },
      { kind: "result" as const, text: "Hay 21.", at: 2 },
    ];
    expect(withoutEchoes(events).map((event) => event.kind)).toEqual(["text", "result"]);
  });
});

describe("time formatting", () => {
  it("rounds ages to the coarsest useful unit", () => {
    expect(ago(0, 30_000).key).toBe("agents.time.justNow");
    expect(ago(0, 5 * 60_000)).toEqual({ key: "agents.time.minutes", params: { n: "5" } });
    expect(ago(0, 3 * 3600_000)).toEqual({ key: "agents.time.hours", params: { n: "3" } });
    expect(ago(0, 49 * 3600_000)).toEqual({ key: "agents.time.days", params: { n: "2" } });
  });

  it("formats durations", () => {
    expect(formatDuration(4_200)).toBe("4 s");
    expect(formatDuration(65_000)).toBe("1 min 05 s");
  });
});
