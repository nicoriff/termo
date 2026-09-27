import { describe, expect, it } from "vitest";

import { isDarkColor } from "./theme-loader";

describe("isDarkColor", () => {
  it("tells Termo's own presets apart", () => {
    expect(isDarkColor("#1e1e1e")).toBe(true); // Dark
    expect(isDarkColor("#ffffff")).toBe(false); // Light
    expect(isDarkColor("#282a36")).toBe(true); // Dracula
  });

  it("goes by perceived brightness, not by the raw channel sum", () => {
    // Same sum, very different brightness: green reads far lighter than blue.
    expect(isDarkColor("#00ff00")).toBe(false);
    expect(isDarkColor("#0000ff")).toBe(true);
  });

  it("accepts the three-digit form", () => {
    expect(isDarkColor("#000")).toBe(true);
    expect(isDarkColor("#fff")).toBe(false);
  });
});
