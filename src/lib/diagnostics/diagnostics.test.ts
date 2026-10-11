import { describe, expect, it } from "vitest";
import {
  cpuOfMachine,
  formatBytes,
  formatPercent,
  formatUptime,
  HISTORY_LENGTH,
  isHeavy,
  pushSample,
  sparklineArea,
  sparklinePoints,
} from "./diagnostics";

describe("pushSample", () => {
  it("appends and keeps only the newest readings", () => {
    expect(pushSample([1, 2], 3, 3)).toEqual([1, 2, 3]);
    expect(pushSample([1, 2, 3], 4, 3)).toEqual([2, 3, 4]);
  });

  it("does not change the series it was given", () => {
    const series = [1];
    pushSample(series, 2);
    expect(series).toEqual([1]);
  });

  it("keeps a minute by default", () => {
    let series: number[] = [];
    for (let i = 0; i < HISTORY_LENGTH + 5; i++) series = pushSample(series, i);
    expect(series).toHaveLength(HISTORY_LENGTH);
    expect(series[series.length - 1]).toBe(HISTORY_LENGTH + 4);
  });
});

describe("cpuOfMachine", () => {
  it("shares the raw reading across the cores", () => {
    expect(cpuOfMachine(200, 8)).toBe(25);
    expect(cpuOfMachine(50, 1)).toBe(50);
  });

  it("stays within 0-100", () => {
    expect(cpuOfMachine(900, 8)).toBe(100);
    expect(cpuOfMachine(-3, 8)).toBe(0);
    expect(cpuOfMachine(10, 0)).toBe(10);
  });
});

describe("sparklinePoints", () => {
  it("is empty without readings", () => {
    expect(sparklinePoints([], 100, 20)).toBe("");
  });

  it("draws a full history from edge to edge, high values near the top", () => {
    const points = sparklinePoints([0, 50, 100], 100, 20, 100, 3).split(" ");
    expect(points).toEqual(["0.0,20.0", "50.0,10.0", "100.0,0.0"]);
  });

  it("starts part-way across while the history is short", () => {
    const points = sparklinePoints([10], 100, 20, 100, 11).split(" ");
    expect(points[0].startsWith("100.0,")).toBe(true);
  });

  it("scales to the largest reading when there is no ceiling", () => {
    expect(sparklinePoints([5, 10], 10, 10, undefined, 2)).toBe("0.0,5.0 10.0,0.0");
  });

  it("does not divide by zero on a flat zero line", () => {
    expect(sparklinePoints([0, 0], 10, 10, undefined, 2)).toBe("0.0,10.0 10.0,10.0");
  });

  it("clamps a reading past the ceiling", () => {
    expect(sparklinePoints([500], 10, 10, 100, 1)).toBe("0.0,0.0");
  });
});

describe("sparklineArea", () => {
  it("closes the line along the bottom", () => {
    expect(sparklineArea("0.0,5.0 10.0,0.0", 10)).toBe("0.0,10 0.0,5.0 10.0,0.0 10.0,10");
    expect(sparklineArea("", 10)).toBe("");
  });
});

describe("formatting", () => {
  it("writes bytes as MB or GB", () => {
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(120 * 1024 * 1024)).toBe("120 MB");
    expect(formatBytes(1.5 * 1024 * 1024 * 1024)).toBe("1.5 GB");
  });

  it("writes a percentage with a decimal only when small", () => {
    expect(formatPercent(3.456)).toBe("3.5%");
    expect(formatPercent(42.4)).toBe("42%");
  });

  it("writes how long something has been up", () => {
    expect(formatUptime(12)).toBe("12 s");
    expect(formatUptime(185)).toBe("3 min");
    expect(formatUptime(3600)).toBe("1 h");
    expect(formatUptime(7500)).toBe("2 h 5 min");
  });
});

describe("isHeavy", () => {
  it("flags most of a core or a gigabyte, and nothing less", () => {
    expect(isHeavy({ cpu: 85, memory: 1 })).toBe(true);
    expect(isHeavy({ cpu: 1, memory: 2 * 1024 * 1024 * 1024 })).toBe(true);
    expect(isHeavy({ cpu: 30, memory: 200 * 1024 * 1024 })).toBe(false);
  });
});
