import { describe, expect, it } from "vitest";

import { BUNDLED_THEMES } from "./presets";
import { isDarkColor } from "./theme-loader";

const HEX = /^#[0-9a-f]{6}$/i;
const LIGHT_THEMES = new Set(["Light", "Solarized Light"]);

describe("bundled themes", () => {
  it("have unique names, since a theme is saved and picked by name", () => {
    const names = BUNDLED_THEMES.map((theme) => theme.name);
    expect(new Set(names).size).toBe(names.length);
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
  });

  // What decides the color-scheme of scrollbars and other native controls.
  it.each(BUNDLED_THEMES.map((theme) => [theme.name, theme] as const))("%s is read as light or dark correctly", (name, theme) => {
    expect(isDarkColor(theme.ui.background)).toBe(!LIGHT_THEMES.has(name));
    expect(theme.appearance).toBe(LIGHT_THEMES.has(name) ? "light" : "dark");
  });
});
