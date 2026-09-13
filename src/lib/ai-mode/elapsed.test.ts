import { describe, expect, it } from "vitest";
import { formatElapsed } from "./elapsed";

describe("formatElapsed", () => {
  it("uses seconds under a minute", () => {
    expect(formatElapsed(0)).toBe("0 s");
    expect(formatElapsed(59_999)).toBe("59 s");
  });

  it("uses minutes under an hour", () => {
    expect(formatElapsed(60_000)).toBe("1 min");
    expect(formatElapsed(59 * 60_000)).toBe("59 min");
  });

  it("uses hours and leftover minutes under a day", () => {
    expect(formatElapsed(60 * 60_000)).toBe("1 h");
    expect(formatElapsed(65 * 60_000)).toBe("1 h 5 min");
  });

  it("uses days beyond that", () => {
    expect(formatElapsed(49 * 60 * 60_000)).toBe("2 d");
  });

  it("never goes negative from clock skew", () => {
    expect(formatElapsed(-5_000)).toBe("0 s");
  });
});
