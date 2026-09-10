import type { Theme } from "./theme-types";
import type { ITheme } from "@xterm/xterm";

function hexToRgba(hex: string, alpha: number): string {
  const clean = hex.replace("#", "");
  const full = clean.length === 3
    ? clean
        .split("")
        .map((c) => c + c)
        .join("")
    : clean;
  const value = parseInt(full, 16);
  const r = (value >> 16) & 255;
  const g = (value >> 8) & 255;
  const b = value & 255;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function toXtermTheme(theme: Theme, opacity = 1): ITheme {
  return {
    ...theme.colors,
    background: hexToRgba(theme.colors.background, opacity),
  };
}

export function applyThemeToDocument(theme: Theme, opacity = 1): void {
  const root = document.documentElement.style;
  const bg = hexToRgba(theme.ui.background, opacity);
  const activeBg = hexToRgba(theme.ui.activeBackground, opacity);

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

  const terminalBg = hexToRgba(theme.colors.background, opacity);
  root.setProperty("--terminal-bg", terminalBg);
  document.body.style.background = terminalBg;
}
