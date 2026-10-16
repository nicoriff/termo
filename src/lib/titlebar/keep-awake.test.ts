import { describe, expect, it } from "vitest";
import { durationLabel, minutesLeft, remaining } from "./keep-awake";

describe("keep awake", () => {
  it("says what's left, rounded up to the minute", () => {
    const now = 1_000_000;
    expect(remaining(now + 45 * 60_000, now)).toBe("45 min");
    expect(remaining(now + 80 * 60_000, now)).toBe("1 h 20 min");
    expect(remaining(now + 120 * 60_000, now)).toBe("2 h");
    expect(remaining(now + 10_000, now)).toBe("1 min");
    expect(remaining(now - 5_000, now)).toBe("1 min");
  });

  it("names each duration", () => {
    expect(durationLabel(null).key).toBe("titlebar.keepAwake.forever");
    expect(durationLabel(30)).toEqual({ key: "titlebar.keepAwake.minutes", params: { n: "30" } });
    expect(durationLabel(60)).toEqual({ key: "titlebar.keepAwake.hour" });
    expect(durationLabel(120)).toEqual({ key: "titlebar.keepAwake.hours", params: { n: "2" } });
  });

  it("carries a session's time over", () => {
    expect(minutesLeft(null, 0)).toBeNull();
    expect(minutesLeft(90 * 60_000 + 1, 0)).toBe(91);
  });
});
