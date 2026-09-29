import { describe, expect, it } from "vitest";
import type { RunRecord } from "$lib/core/agents-client";
import type { AgentDraft } from "$lib/core/agents-client";
import {
  agentState,
  ago,
  applyDraft,
  conversationTree,
  formatDuration,
  newAgentSpec,
  nextRunLabel,
  specProblems,
  secretsOf,
  stepProblems,
  strongerContacts,
  withoutEchoes,
} from "./agent-spec";

function run(status: RunRecord["status"], activity = ""): RunRecord {
  return {
    id: "r",
    agentId: "a",
    trigger: "manual",
    status,
    startedAt: 0,
    endedAt: null,
    summary: "",
    activity,
    costUsd: null,
    link: null,
    rehearsal: false,
  };
}

describe("working with other agents", () => {
  it("asks which agent to follow", () => {
    const spec = newAgentSpec("a");
    spec.triggers.after = { agentId: "", on: "success" };
    expect(stepProblems("triggers", spec)).toEqual(["agents.wizard.problems.afterAgent"]);
    spec.triggers.after.agentId = "b";
    expect(stepProblems("triggers", spec)).toEqual([]);
  });

  it("finds the contacts allowed to do more than the agent", () => {
    const spec = newAgentSpec("a");
    const tester = { ...newAgentSpec("t"), autonomy: "run" as const };
    const reader = newAgentSpec("r");
    spec.collaboration.contacts = ["t", "r"];
    expect(strongerContacts(spec, [tester, reader, newAgentSpec("x")]).map((agent) => agent.id)).toEqual(["t"]);
    spec.autonomy = "run";
    expect(strongerContacts(spec, [tester, reader])).toEqual([]);
  });

  it("lays out a conversation with each run under the one that set it off", () => {
    const at = (id: string, startedAt: number, parent?: string): RunRecord => ({
      ...run("success"),
      id,
      startedAt,
      link: parent ? { conversationId: "a1", parentRunId: parent, parentAgentId: "", path: [], rehearsal: false } : null,
    });
    const tree = conversationTree([at("c1", 3, "b1"), at("a1", 1), at("d1", 4, "a1"), at("b1", 2, "a1")]);
    expect(tree.map(({ run, depth }) => `${run.id}:${depth}`)).toEqual(["a1:0", "b1:1", "c1:2", "d1:1"]);
  });
});

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

describe("applyDraft", () => {
  const draft: AgentDraft = {
    name: " Resumen diario ",
    icon: "📊",
    task: "Resumí los commits",
    instructions: "Pasos: …",
    autonomy: "read",
    web: true,
    schedule: { kind: "daily", minutes: 0, at: "09:00", weekdays: [5, 1, 1, 9] },
    integrations: [],
    explanation: "",
  };

  it("fills the drafted steps and keeps the user's own choices", () => {
    const spec = newAgentSpec("a");
    spec.engine = "codex";
    spec.workDir = "C:\\repo";
    applyDraft(spec, draft);
    expect(spec.name).toBe("Resumen diario");
    expect(spec.tools.web).toBe(true);
    expect(spec.triggers.schedule).toEqual({ kind: "daily", at: "09:00", weekdays: [1, 5] });
    expect(spec.engine).toBe("codex");
    expect(spec.workDir).toBe("C:\\repo");
  });

  it("drops what doesn't fit instead of trusting it", () => {
    const spec = newAgentSpec("a");
    applyDraft(spec, { ...draft, autonomy: "root" as never, schedule: { kind: "daily", minutes: 0, at: "9am", weekdays: [] } });
    expect(spec.autonomy).toBe("read");
    expect(spec.triggers.schedule).toBeNull();
  });
});

describe("secretsOf", () => {
  it("finds the secrets an integration stored, and only those", () => {
    const secrets = ["SMITHERY_AI_FETCH_AUTHORIZATION", "GITHUB_TOKEN", "SMITHERY_AI_FETCHER_KEY"];
    expect(secretsOf("smithery-ai-fetch", secrets)).toEqual(["SMITHERY_AI_FETCH_AUTHORIZATION"]);
    expect(secretsOf("github", secrets)).toEqual(["GITHUB_TOKEN"]);
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

describe("nextRunLabel", () => {
  const time = () => "09:00";
  const weekday = () => "jue";
  const now = new Date(2026, 8, 28, 20, 0).getTime();

  it("says today, tomorrow or the day", () => {
    expect(nextRunLabel(new Date(2026, 8, 28, 21, 0).getTime(), now, time, weekday).key).toBe("agents.next.today");
    expect(nextRunLabel(new Date(2026, 8, 29, 9, 0).getTime(), now, time, weekday).key).toBe("agents.next.tomorrow");
    expect(nextRunLabel(new Date(2026, 9, 1, 9, 0).getTime(), now, time, weekday)).toEqual({
      key: "agents.next.later",
      params: { day: "jue", time: "09:00" },
    });
  });

  it("a missed run reads as due now, not as a past day", () => {
    expect(nextRunLabel(new Date(2026, 8, 27, 9, 0).getTime(), now, time, weekday).key).toBe("agents.next.today");
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
