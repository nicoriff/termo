import { describe, expect, it } from "vitest";

import { BUNDLED_THEMES } from "./presets";
import { VIVID_THEMES } from "./presets-vivid";
import { contrastRatio } from "./theme-paint";
import { isDarkColor } from "./theme-loader";

const HEX = /^#[0-9a-f]{6}$/i;
const LIGHT_THEMES = new Set([
  "Light",
  "Candy",
  "Catppuccin Latte",
  "GitHub Light",
  "Gruvbox Light",
  "Lime Pop",
  "One Light",
  "Rosé Pine Dawn",
  "Solarized Light",
]);

describe("bundled themes", () => {
  it("have unique names, since a theme is saved and picked by name", () => {
    const names = BUNDLED_THEMES.map((theme) => theme.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it("include every colorful one", () => {
    for (const theme of VIVID_THEMES) expect(BUNDLED_THEMES).toContain(theme);
  });

  // A mistyped color doesn't fail anywhere: xterm and CSS just drop it and
  // fall back, so a broken preset would only show as a wrong-looking console.
  it.each(BUNDLED_THEMES.map((theme) => [theme.name, theme] as const))("%s has only valid colors", (_, theme) => {
    for (const [slot, value] of Object.entries(theme.colors)) {
      expect(value, `colors.${slot}`).toMatch(HEX);
    }
    for (const slot of ["background", "foreground", "activeBackground", "activeForeground", "border", "accent"] as const) {
      expect(theme.ui[slot], `ui.${slot}`).toMatch(HEX);
    }
    const { titlebarGradient, tabbarGradient, statusBar, tabColors, glow } = theme.ui;
    const lists = { titlebarGradient, tabbarGradient, tabColors, statusBar: statusBar?.background };
    for (const [slot, list] of Object.entries(lists)) {
      for (const value of list ?? []) expect(value, `ui.${slot}`).toMatch(HEX);
    }
    if (statusBar) expect(statusBar.foreground, "ui.statusBar.foreground").toMatch(HEX);
    if (glow) expect(glow, "ui.glow").toMatch(HEX);
  });

  // What decides the color-scheme of scrollbars and other native controls.
  it.each(BUNDLED_THEMES.map((theme) => [theme.name, theme] as const))("%s is read as light or dark correctly", (name, theme) => {
    expect(isDarkColor(theme.ui.background)).toBe(!LIGHT_THEMES.has(name));
    expect(theme.appearance).toBe(LIGHT_THEMES.has(name) ? "light" : "dark");
  });
});

// The colorful ones paint text over gradients: every stop has to keep it
// readable (WCAG AA, 4.5:1), not just the plain background.
describe("colorful themes", () => {
  it.each(VIVID_THEMES.map((theme) => [theme.name, theme] as const))("%s keeps its text readable", (_, theme) => {
    const { ui, colors } = theme;
    expect(contrastRatio(colors.foreground, colors.background), "console").toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(ui.activeForeground, ui.activeBackground), "active tab").toBeGreaterThanOrEqual(4.5);
    for (const stop of ui.tabbarGradient ?? [ui.background]) {
      expect(contrastRatio(ui.foreground, stop), `tabs on ${stop}`).toBeGreaterThanOrEqual(4.5);
    }
    for (const stop of ui.titlebarGradient ?? [ui.background]) {
      expect(contrastRatio(ui.foreground, stop), `title bar on ${stop}`).toBeGreaterThanOrEqual(4.5);
    }
    for (const stop of ui.statusBar?.background ?? []) {
      expect(contrastRatio(ui.statusBar!.foreground, stop), `status bar on ${stop}`).toBeGreaterThanOrEqual(4.5);
    }
  });
});
