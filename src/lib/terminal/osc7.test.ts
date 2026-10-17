import { describe, expect, it } from "vitest";
import { parseOsc7 } from "./osc7";

describe("parseOsc7", () => {
  it("reads a Windows directory, with or without a host", () => {
    expect(parseOsc7("file:///C:/Users/nico/proj")).toBe("C:\\Users\\nico\\proj");
    expect(parseOsc7("file://desktop-pc/D:/work")).toBe("D:\\work");
  });

  it("leaves a POSIX directory as it is", () => {
    expect(parseOsc7("file:///home/nico/proj")).toBe("/home/nico/proj");
  });

  it("decodes escaped characters", () => {
    expect(parseOsc7("file:///C:/Users/nico/My%20Projects")).toBe("C:\\Users\\nico\\My Projects");
  });

  it("survives a path with a stray percent sign", () => {
    expect(parseOsc7("file:///C:/100%done")).toBe("C:\\100%done");
  });

  it("ignores anything that isn't a file location", () => {
    expect(parseOsc7("http://example.com/x")).toBeNull();
    expect(parseOsc7("file://host-only")).toBeNull();
    expect(parseOsc7("")).toBeNull();
  });

  it("ignores a server's share", () => {
    expect(parseOsc7("file:////evil/share")).toBeNull();
    expect(parseOsc7("file://host/%5C%5Cevil%5Cshare")).toBeNull();
    expect(parseOsc7("file:///%5Cevil%5Cshare")).toBeNull();
  });
});
