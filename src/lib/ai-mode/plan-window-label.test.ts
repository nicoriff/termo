import { describe, expect, it } from "vitest";

import { planWindowLabel } from "./plan-window-label";

describe("planWindowLabel", () => {
  it("keeps the familiar names for Claude's two windows", () => {
    expect(planWindowLabel(300)).toEqual({ key: "statusBar.fiveHour" });
    expect(planWindowLabel(7 * 24 * 60)).toEqual({ key: "statusBar.sevenDay" });
  });

  it("names any other window after how long it runs", () => {
    // What a free Codex plan reports: a single 30-day window.
    expect(planWindowLabel(43_200)).toEqual({ key: "statusBar.windowDays", params: { days: 30 } });
    expect(planWindowLabel(180)).toEqual({ key: "statusBar.windowHours", params: { hours: 3 } });
  });

  it("doesn't round an odd window into a lie", () => {
    expect(planWindowLabel(210)).toEqual({ key: "statusBar.windowHours", params: { hours: 3.5 } });
  });

  it("falls back when the agent reports no length", () => {
    expect(planWindowLabel(null)).toEqual({ key: "statusBar.planWindow" });
    expect(planWindowLabel(0)).toEqual({ key: "statusBar.planWindow" });
  });
});
