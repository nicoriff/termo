export interface ThemeAnsiColors {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

export interface ThemeUi {
  background: string;
  foreground: string;
  activeBackground: string;
  activeForeground: string;
  border: string;
  accent: string;
  hoverBackground: string;
  closeHoverBackground: string;
}

export interface ThemeFont {
  family: string;
  size: number;
  lineHeight: number;
}

export type Appearance = "light" | "dark";

export interface Theme {
  name: string;
  appearance: Appearance;
  colors: ThemeAnsiColors;
  ui: ThemeUi;
  font: ThemeFont;
}
