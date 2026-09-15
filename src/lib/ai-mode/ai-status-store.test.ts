import { beforeEach, describe, expect, it } from "vitest";
import { aiStatusStore } from "./ai-status-store.svelte";

const TAB = "tab-1";
const UNRECOGNIZED = { tool: null, status: null } as const;

describe("aiStatusStore with hooks", () => {
  beforeEach(() => aiStatusStore.remove(TAB));

  it("keeps a hook status when the screen never recognized the tool", () => {
    aiStatusStore.setFromHook(TAB, "claude", "running");
    aiStatusStore.set(TAB, UNRECOGNIZED);
    aiStatusStore.set(TAB, UNRECOGNIZED);
    expect(aiStatusStore.byId[TAB]?.status).toBe("running");
    expect(aiStatusStore.counts.running).toBe(1);
  });

  it("ignores ordinary screen statuses once hooks own the session", () => {
    aiStatusStore.setFromHook(TAB, "claude", "running");
    aiStatusStore.set(TAB, { tool: "claude", status: "finished" });
    expect(aiStatusStore.byId[TAB]?.status).toBe("running");
  });

  it("still lets the screen flag an error or see work resume after a prompt", () => {
    aiStatusStore.setFromHook(TAB, "claude", "running");
    aiStatusStore.set(TAB, { tool: "claude", status: "error" });
    expect(aiStatusStore.byId[TAB]?.status).toBe("error");

    aiStatusStore.setFromHook(TAB, "claude", "awaiting");
    aiStatusStore.set(TAB, { tool: "claude", status: "running" });
    expect(aiStatusStore.byId[TAB]?.status).toBe("running");
  });

  it("clears only when the hook reports the session ended", () => {
    aiStatusStore.setFromHook(TAB, "claude", "finished");
    aiStatusStore.setFromHook(TAB, "claude", null);
    expect(aiStatusStore.byId[TAB]).toBeUndefined();
  });

  it("without hooks, the screen still drives the status", () => {
    aiStatusStore.set(TAB, { tool: "claude", status: "running" });
    aiStatusStore.set(TAB, { tool: "claude", status: "finished" });
    expect(aiStatusStore.byId[TAB]?.status).toBe("finished");
  });
});
