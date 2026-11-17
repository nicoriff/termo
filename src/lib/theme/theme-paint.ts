import type { Theme } from "./theme-types";

/**
 * The colorful parts of a theme (see ThemeUi's optional fields) as CSS: the
 * gradients the title bar, tab strips and status bar paint with, the colors
 * tabs take, the focused tile's glow. Pure, so it's tested in plain Node.
 */

export type TabColorsMode = "theme" | "always" | "never";

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const clean = hex.replace("#", "");
  const full = clean.length === 3
    ? clean
        .split("")
        .map((c) => c + c)
        .join("")
    : clean;
  const value = parseInt(full, 16);
  return { r: (value >> 16) & 255, g: (value >> 8) & 255, b: value & 255 };
}

export function hexToRgba(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** What a bar's `background` is: its gradient, at the window's opacity, or
 * the plain color when the theme has none. One stop is a plain color too. */
export function barPaint(stops: string[] | undefined, fallback: string, opacity: number): string {
  if (!stops || stops.length === 0) return hexToRgba(fallback, opacity);
  if (stops.length === 1) return hexToRgba(stops[0], opacity);
  return `linear-gradient(90deg, ${stops.map((stop) => hexToRgba(stop, opacity)).join(", ")})`;
}

/** The colors tabs take, in turn; empty when tabs aren't colored. "theme"
 * colors them only for a theme that has colors of its own for them. */
export function tabPalette(theme: Theme, mode: TabColorsMode): string[] {
  if (mode === "never") return [];
  if (theme.ui.tabColors && theme.ui.tabColors.length > 0) return theme.ui.tabColors;
  if (mode === "theme") return [];
  const { red, yellow, green, cyan, blue, magenta } = theme.colors;
  return [blue, magenta, green, yellow, cyan, red];
}

/** The focused tile's glow, as a box-shadow; "none" without one. Inset, so
 * it doesn't spill onto the tile next to it. */
export function glowShadow(color: string | undefined): string {
  if (!color) return "none";
  return `inset 0 0 0 1px ${hexToRgba(color, 0.55)}, inset 0 0 18px ${hexToRgba(color, 0.18)}`;
}

/** WCAG's contrast ratio between two colors, 1 to 21. */
export function contrastRatio(a: string, b: string): number {
  const luminance = (hex: string) => {
    const { r, g, b: blue } = hexToRgb(hex);
    const channel = (value: number) => {
      const c = value / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(blue);
  };
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}
