import type { Theme } from "./theme-types";
import type { ITheme } from "@xterm/xterm";
import { barPaint, glowShadow, hexToRgb, hexToRgba } from "./theme-paint";

/** Whether a theme's own background is dark, by perceived brightness (the
 * usual sRGB weighting: green reads brightest, blue dimmest). What it's for
 * is `color-scheme`, so the native controls Termo doesn't paint itself —
 * scrollbars above all — come out dark on a dark theme and light on a light
 * one, instead of the engine's assumption that every page is light. */
export function isDarkColor(hex: string): boolean {
  const { r, g, b } = hexToRgb(hex);
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 < 0.5;
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

  // Anything the engine paints rather than Termo — a scrollbar, a native
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
  // For panels floating over a console: with the translucent one the text
  // underneath shows through and competes with the panel's own.
  root.setProperty("--tabbar-active-bg-solid", theme.ui.activeBackground);
  root.setProperty("--tabbar-active-fg", theme.ui.activeForeground);

  root.setProperty("--pane-active-border", theme.ui.accent);
  root.setProperty("--tab-focus-border", theme.ui.accent);
  root.setProperty("--tab-focus-bg", hexToRgba(theme.ui.accent, 0.35));
  root.setProperty("--divider-bg", theme.ui.border);
  root.setProperty("--divider-hover-bg", theme.ui.accent);

  // What the bars paint with: a theme's gradients, or the plain colors
  // above. Separate from --titlebar-bg / --tabbar-bg, which panels and
  // dialogs use as a plain color (a gradient wouldn't do there).
  root.setProperty("--titlebar-paint", barPaint(theme.ui.titlebarGradient, theme.ui.background, opacity));
  root.setProperty("--tabbar-paint", barPaint(theme.ui.tabbarGradient, theme.ui.background, opacity));
  root.setProperty("--statusbar-paint", barPaint(theme.ui.statusBar?.background, theme.ui.background, opacity));
  root.setProperty("--statusbar-fg", theme.ui.statusBar?.foreground ?? theme.ui.foreground);
  root.setProperty("--pane-glow", glowShadow(theme.ui.glow));

  // Deliberately only a variable, never a background on <body>: the rounded
  // window corners are a border-radius on .window-chrome, and anything
  // painted on an ancestor of it fills those corners back in as a square.
  // Windows hid that because SetWindowRgn clips the HWND itself.
  root.setProperty("--terminal-bg", hexToRgba(theme.colors.background, opacity));
}
