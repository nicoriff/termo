import { describe, expect, it } from "vitest";
import { neutralizePaintedControls } from "./conpty-controls";

describe("neutralizePaintedControls", () => {
  // Recorded from a Claude Code console: the SI cell, then a backspace to
  // overwrite it. With the SI taking its column, the "a" lands where it was
  // typed instead of on top of the prompt.
  it("gives a painted SI the column ConPTY counted", () => {
    expect(neutralizePaintedControls("\x0f")).toBe(" ");
    expect(neutralizePaintedControls("\x1b[25l\x08a\x1b[K")).toBe("\x1b[25l\x08a\x1b[K");
  });

  it("leaves the controls ConPTY itself sends alone", () => {
    const text = "\x1b]0;title\x07line\r\n\tx\x08y\x07";
    expect(neutralizePaintedControls(text)).toBe(text);
  });

  it("replaces every other C0 control one for one", () => {
    expect(neutralizePaintedControls("a\x00b\x0ec\x1fd")).toBe("a b c d");
  });
});
