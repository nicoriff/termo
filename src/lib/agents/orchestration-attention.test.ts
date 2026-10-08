import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({ sendNotification: vi.fn() }));
vi.mock("$lib/core/app-config.svelte", () => ({ appConfigStore: {} }));
vi.mock("$lib/core/window-client", () => ({ isPrimaryWindow: vi.fn() }));
vi.mock("$lib/i18n/i18n.svelte", () => ({ t: (key: string) => key }));
vi.mock("$lib/ai-mode/attention", () => ({ canNotify: vi.fn() }));

import { attentionTitle } from "./orchestration-attention";

describe("when an orchestration notifies", () => {
  it("speaks when a running one ends or needs the user", () => {
    expect(attentionTitle("running", "done")).toBe("agents.orchestration.notify.done");
    expect(attentionTitle("running", "awaitingApproval")).toBe("agents.orchestration.notify.awaitingApproval");
    expect(attentionTitle("running", "paused")).toBe("agents.orchestration.notify.paused");
    expect(attentionTitle("running", "failed")).toBe("agents.orchestration.notify.failed");
  });

  it("stays quiet for the user's own doing and for what wasn't running", () => {
    expect(attentionTitle("running", "stopped")).toBeNull();
    expect(attentionTitle("running", "running")).toBeNull();
    expect(attentionTitle("draft", "running")).toBeNull();
    expect(attentionTitle(undefined, "done")).toBeNull();
    expect(attentionTitle("failed", "failed")).toBeNull();
  });
});
