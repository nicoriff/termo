import { describe, expect, it } from "vitest";
import { BUNDLED_THEMES } from "$lib/theme/presets";
import { visibleSelection } from "./editor-theme";

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

describe("visibleSelection", () => {
  it("keeps a theme's own selection when it already stands out", () => {
    expect(visibleSelection("#1e1e1e", "#264f78", "#2472c8")).toBe("#264f78");
  });

  it("stands out from the background in every bundled theme", () => {
    for (const theme of BUNDLED_THEMES) {
      const { background, selectionBackground, blue } = theme.colors;
      const selection = visibleSelection(background, selectionBackground, blue);
      expect(contrast(background, selection), theme.name).toBeGreaterThanOrEqual(1.5);
    }
  });
});
