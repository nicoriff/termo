import { describe, expect, it } from "vitest";

import { agoLabel, formatDuration, formatPath, formatTokens, formatUsd, promptEngineName, resetsInMs } from "./format";

describe("promptEngineName", () => {
  it("names the tools, and leaves the shell's own to be worded", () => {
    expect(promptEngineName("oh_my_posh")).toBe("Oh My Posh");
    expect(promptEngineName("starship")).toBe("Starship");
    expect(promptEngineName("none")).toBeNull();
  });
});

describe("formatTokens", () => {
  it("keeps small counts whole and shortens the rest", () => {
    expect(formatTokens(950)).toBe("950");
    expect(formatTokens(15_500)).toBe("15.5k");
    expect(formatTokens(200_000)).toBe("200k");
    expect(formatTokens(1_000_000)).toBe("1M");
  });
});

describe("formatDuration", () => {
  it("grows its unit with the time", () => {
    expect(formatDuration(45_000)).toBe("45s");
    expect(formatDuration(12 * 60_000)).toBe("12m");
    expect(formatDuration(65 * 60_000)).toBe("1h 05m");
    expect(formatDuration(51 * 3_600_000)).toBe("2d 3h");
  });
});

it("formatUsd doesn't call a started session free", () => {
  expect(formatUsd(0.004)).toBe("<$0.01");
  expect(formatUsd(1.5)).toBe("$1.50");
});

it("agoLabel words how long ago", () => {
  expect(agoLabel(20_000)).toEqual({ key: "statusBar.ago.now" });
  expect(agoLabel(5 * 60_000)).toEqual({ key: "statusBar.ago.minutes", params: { n: "5" } });
  expect(agoLabel(3 * 3_600_000)).toEqual({ key: "statusBar.ago.hours", params: { n: "3" } });
});

describe("formatPath", () => {
  const home = "C:\\Users\\nico";

  it("shows the home folder as ~", () => {
    expect(formatPath("C:\\Users\\nico\\code\\termo", "home", home, null)).toBe("~\\code\\termo");
    expect(formatPath("c:\\users\\NICO", "home", home, null)).toBe("~");
    expect(formatPath("/home/nico/src", "home", "/home/nico", null)).toBe("~/src");
    expect(formatPath("D:\\other", "home", home, null)).toBe("D:\\other");
  });

  it("doesn't take a folder that only starts like home for it", () => {
    expect(formatPath("/home/nicolas", "home", "/home/nico", null)).toBe("/home/nicolas");
  });

  it("starts from the repository's name", () => {
    expect(formatPath("E:\\Proyectos\\Termo\\src\\lib", "repo", home, "E:\\Proyectos\\Termo")).toBe("Termo\\src\\lib");
    expect(formatPath("/work/termo", "repo", null, "/work/termo")).toBe("termo");
  });

  it("falls back to the home form outside a repository", () => {
    expect(formatPath("C:\\Users\\nico\\notes", "repo", home, null)).toBe("~\\notes");
  });

  it("can show only the folder, or all of it", () => {
    expect(formatPath("/work/termo/", "folder", null, null)).toBe("termo");
    expect(formatPath("/work/termo/", "full", null, null)).toBe("/work/termo");
  });
});

it("resetsInMs reads seconds and milliseconds, and nothing once past", () => {
  const now = 1_789_000_000_000;
  expect(resetsInMs(1_789_000_600, now)).toBe(600_000);
  expect(resetsInMs(1_789_000_600_000, now)).toBe(600_000);
  expect(resetsInMs(1_788_000_000, now)).toBeNull();
  expect(resetsInMs(null, now)).toBeNull();
});
