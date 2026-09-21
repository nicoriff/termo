import type { Theme } from "./theme-types";
import type { ITheme } from "@xterm/xterm";

function hexToRgb(hex: string): { r: number; g: number; b: number } {
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

/** Whether a theme's own background is dark, by perceived brightness (the
 * usual sRGB weighting: green reads brightest, blue dimmest). What it's for
 * is `color-scheme`, so the native controls Termini doesn't paint itself —
 * scrollbars above all — come out dark on a dark theme and light on a light
 * one, instead of the engine's assumption that every page is light. */
export function isDarkColor(hex: string): boolean {
  const { r, g, b } = hexToRgb(hex);
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 < 0.5;
}

function hexToRgba(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** xterm's own background stays fully transparent: the terminal's container
 * already paints the translucent console background, and a second
 * translucent layer here made consoles more opaque than the setting. */
export function toXtermTheme(theme: Theme): ITheme {
  return {
    ...theme.colors,
    background: hexToRgba(theme.colors.background, 0),
  };
}

export function applyThemeToDocument(theme: Theme, opacity = 1): void {
  const root = document.documentElement.style;
  const bg = hexToRgba(theme.ui.background, opacity);
  const activeBg = hexToRgba(theme.ui.activeBackground, opacity);

  // Anything the engine paints rather than Termini — a scrollbar, a native
  // control — follows this. Without it every window is assumed light, which
  // on the default dark theme is where Preferences' pale scrollbar came from.
  root.setProperty("color-scheme", isDarkColor(theme.ui.background) ? "dark" : "light");

  root.setProperty("--titlebar-bg", bg);
  root.setProperty("--titlebar-fg", theme.ui.foreground);
  root.setProperty("--titlebar-hover-bg", theme.ui.hoverBackground);
  root.setProperty("--titlebar-close-hover-bg", theme.ui.closeHoverBackground);

  root.setProperty("--tabbar-bg", bg);
  root.setProperty("--tabbar-border", theme.ui.border);
  root.setProperty("--tabbar-fg", theme.ui.foreground);
  root.setProperty("--tabbar-active-bg", activeBg);
  root.setProperty("--tabbar-active-fg", theme.ui.activeForeground);

  root.setProperty("--pane-active-border", theme.ui.accent);
  root.setProperty("--tab-focus-border", theme.ui.accent);
  root.setProperty("--tab-focus-bg", hexToRgba(theme.ui.accent, 0.35));
  root.setProperty("--divider-bg", theme.ui.border);
  root.setProperty("--divider-hover-bg", theme.ui.accent);

  // Deliberately only a variable, never a background on <body>: the rounded
  // window corners are a border-radius on .window-chrome, and anything
  // painted on an ancestor of it fills those corners back in as a square.
  // Windows hid that because SetWindowRgn clips the HWND itself.
  root.setProperty("--terminal-bg", hexToRgba(theme.colors.background, opacity));
}
