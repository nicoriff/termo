import { describe, expect, it } from "vitest";
import { OutputHighlighter } from "./output-highlighter";

/** The prompt of a bash sitting in a deep directory: bash sets the window
 * title (OSC 0) and Termo asks it to report its cwd (OSC 7, see
 * src-tauri/src/pty/cwd_report.rs). Both carry a full path, so both are
 * longer than a single PTY read whenever the directory is deep enough. */
const DEEP = "/c/Users/Nicolas/Documents/Proyectos/termo/src/lib/terminal";
const TITLE_OSC = `\x1b]0;MINGW64:${DEEP}\x07`;
const CWD_OSC = `\x1b]7;file://${DEEP}\x07`;

function feedSplit(chunks: string[]): string {
  const highlighter = new OutputHighlighter();
  return chunks.map((chunk) => highlighter.feed(chunk)).join("");
}

describe("OutputHighlighter", () => {
  it("leaves an OSC sequence alone when it arrives whole", () => {
    expect(feedSplit([`${TITLE_OSC}$ `])).toContain(TITLE_OSC);
  });

  // A PTY read can land anywhere, and a sequence the rules got to touch is
  // one xterm stops reading as a sequence: the injected ESC ends the string
  // early and the rest of the payload prints onto the prompt line.
  it.each([TITLE_OSC, CWD_OSC])("survives a split at any offset: %j", (osc) => {
    for (let cut = 1; cut < osc.length; cut++) {
      const out = feedSplit([osc.slice(0, cut), `${osc.slice(cut)}$ `]);
      expect(out, `split at ${cut}`).toBe(`${osc}$ `);
    }
  });

  it("still highlights the plain text around a split sequence", () => {
    const out = feedSplit([`ok ${TITLE_OSC.slice(0, 12)}`, `${TITLE_OSC.slice(12)}error`]);
    expect(out).toContain(TITLE_OSC);
    expect(out).toContain("\x1b[38;5;203merror\x1b[39m");
  });

  it("gives up on a bare ESC rather than holding output back forever", () => {
    const stray = `\x1b${"x".repeat(2000)}`;
    expect(feedSplit([stray])).toContain("x".repeat(2000));
  });

  describe("restores the shell's own foreground after a match", () => {
    // Captured from a real prompt: the shell paints the line grey, and the
    // path inside it is what the rules match. Closing with `39` would drop
    // the rest of the shell's line to the terminal default instead of grey.
    it("hands back a truecolour foreground", () => {
      const grey = "\x1b[38;2;153;153;153m";
      const out = feedSplit([`${grey}E:\\Proyectos\\Excaliwall tail\x1b[m`]);
      expect(out).toBe(`${grey}\x1b[38;5;176mE:\\Proyectos\\Excaliwall${grey} tail\x1b[m`);
    });

    it("hands back a basic foreground", () => {
      const out = feedSplit(["\x1b[32mvisit https://x.dev now\x1b[m"]);
      expect(out).toBe("\x1b[32mvisit \x1b[38;5;75mhttps://x.dev\x1b[32m now\x1b[m");
    });

    it("uses 39 only when the shell really is at the default", () => {
      expect(feedSplit(["visit https://x.dev now"])).toBe("visit \x1b[38;5;75mhttps://x.dev\x1b[39m now");
    });

    it("treats a reset as back to the default", () => {
      const out = feedSplit(["\x1b[32m\x1b[mvisit https://x.dev now"]);
      expect(out).toBe("\x1b[32m\x1b[mvisit \x1b[38;5;75mhttps://x.dev\x1b[39m now");
    });

    it("is not fooled by a background colour's arguments", () => {
      // 48;5;21 sets the background; its `21` must not read as a foreground.
      const out = feedSplit(["\x1b[31m\x1b[48;5;21mgo to https://x.dev\x1b[m"]);
      expect(out).toBe("\x1b[31m\x1b[48;5;21mgo to \x1b[38;5;75mhttps://x.dev\x1b[31m\x1b[m");
    });

    it("carries the colour across a chunk boundary", () => {
      const out = feedSplit(["\x1b[36mgo to ", "https://x.dev now\x1b[m"]);
      expect(out).toBe("\x1b[36mgo to \x1b[38;5;75mhttps://x.dev\x1b[36m now\x1b[m");
    });
  });
});
