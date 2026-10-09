import { BUNDLED_THEMES, DARK_THEME, LIGHT_THEME } from "./presets";
import { applyThemeToDocument } from "./theme-loader";
import type { Appearance, Theme } from "./theme-types";

interface ThemePreferences {
  manualTheme: string;
  syncWithOs?: boolean;
  lightTheme?: string | null;
  darkTheme?: string | null;
}

export class ThemeStore {
  themes = $state<Theme[]>(BUNDLED_THEMES);
  current = $state<Theme>(DARK_THEME);
  /** User-selected theme; persisted as `theme` for compatibility with existing configs. */
  manualTheme = $state(DARK_THEME.name);
  syncWithOs = $state(false);
  lightTheme = $state(LIGHT_THEME.name);
  darkTheme = $state(DARK_THEME.name);
  systemAppearance = $state<Appearance>("dark");
  fontSizeOverride = $state<number | null>(null);
  fontFamilyOverride = $state<string | null>(null);
  /** 0 (fully transparent) to 100 (opaque). */
  backgroundOpacity = $state<number>(100);
  /** Colors terminal output (IPs, URLs, paths, errors, etc.) after the fact — see output-highlighter.ts. */
  outputHighlightingEnabled = $state<boolean>(true);

  /** Select a fixed theme and disable OS synchronization. */
  selectTheme(name: string): void {
    const theme = this.themes.find((t) => t.name === name);
    if (!theme) return;
    this.manualTheme = name;
    this.syncWithOs = false;
    if (theme.appearance === "light") this.lightTheme = name;
    else this.darkTheme = name;
    this.applyCurrentTheme();
  }

  applyPreferences(preferences: ThemePreferences): void {
    const manual = this.themes.find((t) => t.name === preferences.manualTheme) ?? DARK_THEME;
    const categoryTheme = (name: string | null | undefined, appearance: Appearance): string => {
      const fallback = appearance === "light" ? LIGHT_THEME : DARK_THEME;
      // Older configs have no category choices. Preserve their theme in its category.
      if (name == null) return manual.appearance === appearance ? manual.name : fallback.name;
      return this.themes.find((t) => t.name === name && t.appearance === appearance)?.name ?? fallback.name;
    };
    this.manualTheme = manual.name;
    this.lightTheme = categoryTheme(preferences.lightTheme, "light");
    this.darkTheme = categoryTheme(preferences.darkTheme, "dark");
    this.syncWithOs = preferences.syncWithOs ?? false;
    this.applyCurrentTheme();
  }

  setSyncWithOs(enabled: boolean): void {
    this.syncWithOs = enabled;
    this.applyCurrentTheme();
  }

  setAppearanceTheme(appearance: Appearance, name: string): void {
    if (!this.themes.some((t) => t.name === name && t.appearance === appearance)) return;
    if (appearance === "light") this.lightTheme = name;
    else this.darkTheme = name;
    this.applyCurrentTheme();
  }

  setSystemAppearance(appearance: Appearance): void {
    this.systemAppearance = appearance;
    this.applyCurrentTheme();
  }

  private applyCurrentTheme(): void {
    const name = this.syncWithOs
      ? (this.systemAppearance === "light" ? this.lightTheme : this.darkTheme)
      : this.manualTheme;
    const theme = this.themes.find((t) => t.name === name) ?? DARK_THEME;
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
