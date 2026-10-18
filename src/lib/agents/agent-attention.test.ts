import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/core/agents-client", () => ({ onAgentQuestion: vi.fn(), onAgentRun: vi.fn() }));
vi.mock("$lib/core/notification-client", () => ({ notify: vi.fn() }));
vi.mock("$lib/core/app-config.svelte", () => ({ appConfigStore: {} }));
vi.mock("$lib/core/window-client", () => ({ isPrimaryWindow: vi.fn() }));
vi.mock("$lib/i18n/i18n.svelte", () => ({ t: (key: string) => key }));
vi.mock("$lib/layout/split-tree", () => ({ collectSessions: vi.fn() }));
vi.mock("$lib/layout/workspace-store.svelte", () => ({ workspaceStore: {} }));
vi.mock("$lib/ai-mode/attention", () => ({ canNotify: vi.fn() }));
vi.mock("$lib/ai-mode/sounds", () => ({ playSound: vi.fn() }));
vi.mock("./agents-store.svelte", () => ({ agentsStore: { agents: [] } }));

import type { RunRecord, RunStatus, RunTrigger } from "$lib/core/agents-client";
import { worthTelling } from "./agent-attention";

const run = (trigger: RunTrigger, status: RunStatus, agentId = "a1") => ({ id: "r", agentId, trigger, status }) as RunRecord;

describe("which ended runs notify", () => {
  it("tells every end of a run the user started", () => {
    const failed = new Set<string>();
    expect(worthTelling(run("manual", "success"), failed)).toBe(true);
    expect(worthTelling(run("console", "success"), failed)).toBe(true);
    expect(worthTelling(run("manual", "error"), failed)).toBe(true);
    expect(worthTelling(run("manual", "error"), failed)).toBe(true);
  });

  it("keeps quiet when one that ran on its own goes well", () => {
    const failed = new Set<string>();
    for (const trigger of ["schedule", "watch", "git", "webhook", "chain", "agent", "message"] as const) {
      expect(worthTelling(run(trigger, "success"), failed)).toBe(false);
    }
  });

  it("tells a failure on its own once, until it works again", () => {
    const failed = new Set<string>();
    expect(worthTelling(run("schedule", "error"), failed)).toBe(true);
    expect(worthTelling(run("schedule", "timedOut"), failed)).toBe(false);
    // Another agent's failures are its own.
    expect(worthTelling(run("schedule", "error", "a2"), failed)).toBe(true);
    // Worked again, by hand or on its own: the next failure is news.
    expect(worthTelling(run("schedule", "success"), failed)).toBe(false);
    expect(worthTelling(run("schedule", "error"), failed)).toBe(true);
    expect(worthTelling(run("manual", "success"), failed)).toBe(true);
    expect(worthTelling(run("watch", "error"), failed)).toBe(true);
  });
});
