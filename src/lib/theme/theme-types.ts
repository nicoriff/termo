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
  /* What makes a theme colorful, all optional: without them it paints with
     the plain colors above, as every theme did before (see theme-paint.ts). */
  /** The title bar's background as a left-to-right gradient, 2 or 3 stops. */
  titlebarGradient?: string[];
  /** The tab strips' background, the same way. */
  tabbarGradient?: string[];
  /** The status bar: its background (one stop is a plain color) and text. */
  statusBar?: { background: string[]; foreground: string };
  /** The colors tabs take one after the other when tabs are colored
   * (Preferences → Appearance → Tabs). Without them, the console's own. */
  tabColors?: string[];
  /** A soft light around the focused tile. */
  glow?: string;
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
  /** Termo's own colorful or retro themes, set apart in the theme picker;
   * absent for the classic ones. */
  kind?: "colorful" | "retro";
  colors: ThemeAnsiColors;
  ui: ThemeUi;
  font: ThemeFont;
}
