import { describe, expect, it } from "vitest";
import { DRACULA_THEME } from "./presets";
import { NEON_NIGHT_THEME } from "./presets-vivid";
import { barPaint, contrastRatio, glowShadow, tabPalette } from "./theme-paint";

describe("barPaint", () => {
  it("paints a gradient at the window's opacity, or the plain color", () => {
    expect(barPaint(["#000000", "#ffffff"], "#111111", 0.5)).toBe(
      "linear-gradient(90deg, rgba(0, 0, 0, 0.5), rgba(255, 255, 255, 0.5))",
    );
    expect(barPaint(["#ff0000"], "#111111", 1)).toBe("rgba(255, 0, 0, 1)");
    expect(barPaint(undefined, "#111111", 1)).toBe("rgba(17, 17, 17, 1)");
  });
});

describe("tabPalette", () => {
  it("colors tabs as the theme says unless told otherwise", () => {
    expect(tabPalette(NEON_NIGHT_THEME, "theme")).toEqual(NEON_NIGHT_THEME.ui.tabColors);
    expect(tabPalette(DRACULA_THEME, "theme")).toEqual([]);
    expect(tabPalette(NEON_NIGHT_THEME, "never")).toEqual([]);
  });

  it("borrows the console's colors for a theme without its own", () => {
    const { blue, magenta, green, yellow, cyan, red } = DRACULA_THEME.colors;
    expect(tabPalette(DRACULA_THEME, "always")).toEqual([blue, magenta, green, yellow, cyan, red]);
  });
});

describe("glowShadow", () => {
  it("is nothing without a glow color", () => {
    expect(glowShadow(undefined)).toBe("none");
    expect(glowShadow("#ff0000")).toContain("rgba(255, 0, 0, 0.55)");
  });
});

describe("contrastRatio", () => {
  it("measures as WCAG does", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrastRatio("#777777", "#777777")).toBeCloseTo(1, 5);
  });
});
