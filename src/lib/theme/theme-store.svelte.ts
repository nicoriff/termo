import { BUNDLED_THEMES } from "./presets";
import { applyThemeToDocument } from "./theme-loader";
import type { Theme } from "./theme-types";

class ThemeStore {
  themes = $state<Theme[]>(BUNDLED_THEMES);
  current = $state<Theme>(BUNDLED_THEMES[0]);
  fontSizeOverride = $state<number | null>(null);
  fontFamilyOverride = $state<string | null>(null);
  /** 0 (fully transparent) to 100 (opaque). */
  backgroundOpacity = $state<number>(100);
  /** Colors terminal output (IPs, URLs, paths, errors, etc.) after the fact — see output-highlighter.ts. */
  outputHighlightingEnabled = $state<boolean>(true);

  setTheme(name: string): void {
    const theme = this.themes.find((t) => t.name === name);
    if (!theme) return;
    this.current = theme;
    this.applyToDocument();
  }

  setFontSize(size: number | null): void {
    this.fontSizeOverride = size;
  }

  setFontFamily(family: string | null): void {
    this.fontFamilyOverride = family && family.trim() !== "" ? family : null;
  }

  setBackgroundOpacity(opacity: number): void {
    this.backgroundOpacity = Math.min(100, Math.max(0, Math.round(opacity)));
    this.applyToDocument();
  }

  setOutputHighlighting(enabled: boolean): void {
    this.outputHighlightingEnabled = enabled;
  }

  get effectiveFontSize(): number {
    return this.fontSizeOverride ?? this.current.font.size;
  }

  get effectiveFontFamily(): string {
    return this.fontFamilyOverride ?? this.current.font.family;
  }

  private applyToDocument(): void {
    applyThemeToDocument(this.current, this.backgroundOpacity / 100);
  }

  init(): void {
    this.applyToDocument();
  }
}

export const themeStore = new ThemeStore();
