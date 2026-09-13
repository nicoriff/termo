import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getConfig, saveConfig } from "./config-client";
import { getWindowSize, setWindowSize } from "./window-client";
import { themeStore } from "../theme/theme-store.svelte";
import { injectFontFace } from "../theme/google-fonts";
import { isMac } from "./platform";
import { resolveKeybinding, SHORTCUT_ACTIONS, type ShortcutAction } from "./shortcuts";
import type { AppConfig, ShortcutsConfig } from "./types";

const EMPTY_SHORTCUTS: ShortcutsConfig = {
  new_tab_mac: null,
  new_tab_other: null,
  cycle_tab_mac: null,
  cycle_tab_other: null,
  zoom_tile_mac: null,
  zoom_tile_other: null,
  ai_running_mac: null,
  ai_running_other: null,
  ai_awaiting_mac: null,
  ai_awaiting_other: null,
  ai_error_mac: null,
  ai_error_other: null,
  ai_finished_mac: null,
  ai_finished_other: null,
};

const SHORTCUT_FIELDS: Record<ShortcutAction, { mac: keyof ShortcutsConfig; other: keyof ShortcutsConfig }> = {
  newTab: { mac: "new_tab_mac", other: "new_tab_other" },
  cycleTab: { mac: "cycle_tab_mac", other: "cycle_tab_other" },
  zoomTile: { mac: "zoom_tile_mac", other: "zoom_tile_other" },
  aiRunning: { mac: "ai_running_mac", other: "ai_running_other" },
  aiAwaiting: { mac: "ai_awaiting_mac", other: "ai_awaiting_other" },
  aiError: { mac: "ai_error_mac", other: "ai_error_other" },
  aiFinished: { mac: "ai_finished_mac", other: "ai_finished_other" },
};

interface InitOptions {
  /** MainWindow restores the persisted window size on launch; Preferences
   * opens at its own fixed size and must not fight that. Defaults to true. */
  restoreWindowSize?: boolean;
}

class AppConfigStore {
  defaultShellId = $state<string | null>(null);
  psreadlineMenuComplete = $state(false);
  bashPromptColors = $state(false);
  ohMyPoshEnabled = $state(false);
  ohMyPoshTheme = $state("jandedobbeleer");
  zshMenuComplete = $state(false);
  aiModeEnabled = $state(false);
  shortcuts = $state<ShortcutsConfig>(EMPTY_SHORTCUTS);
  loaded = $state(false);

  private windowWidth = 1000;
  private windowHeight = 640;
  private installedFontPath: string | null = null;

  async init(options?: InitOptions): Promise<void> {
    const config = await getConfig();
    this.applyFields(config);

    if (options?.restoreWindowSize !== false) {
      try {
        await setWindowSize(config.window.width, config.window.height);
      } catch {
        // Non-critical — keep the default size from tauri.conf.json.
      }
    }

    this.loaded = true;

    // Cross-window sync: each Tauri window is a separate webview with its
    // own JS module graph, so appConfigStore/themeStore here are NOT shared
    // with the other window's copies — the backend re-broadcasts the full
    // config after every save so both can stay in sync live. applyFields()
    // never persists, so this can never turn into a save -> emit -> apply ->
    // save loop.
    const ownLabel = getCurrentWindow().label;
    void listen<{ source: string; config: AppConfig }>("config-updated", (event) => {
      if (event.payload.source !== ownLabel) this.applyFields(event.payload.config);
    });
  }

  /** Applies a config snapshot to local state + the theme store, without
   * persisting — used both by init() and by the config-updated listener, so
   * that reacting to another window's change never triggers a save of our
   * own. Setters below are the only paths that persist. */
  private applyFields(config: AppConfig): void {
    this.defaultShellId = config.default_shell_id;
    this.psreadlineMenuComplete = config.psreadline_menu_complete;
    this.bashPromptColors = config.bash_prompt_colors;
    this.ohMyPoshEnabled = config.oh_my_posh_enabled;
    this.ohMyPoshTheme = config.oh_my_posh_theme;
    this.zshMenuComplete = config.zsh_menu_complete;
    this.aiModeEnabled = config.ai_mode_enabled;
    this.shortcuts = config.shortcuts;
    this.installedFontPath = config.installed_font_path;
    // Kept current even in Preferences (which never resizes to it) so its
    // saves don't write back a stale main-window size.
    this.windowWidth = config.window.width;
    this.windowHeight = config.window.height;
    themeStore.setTheme(config.theme);
    themeStore.setFontSize(config.font_size_override);
    if (config.font_family_override && config.installed_font_path) {
      // A previously-downloaded Google Font — re-apply from the local file,
      // no network needed.
      injectFontFace(config.font_family_override, config.installed_font_path);
    }
    themeStore.setFontFamily(config.font_family_override);
    themeStore.setBackgroundOpacity(config.background_opacity);
    themeStore.setOutputHighlighting(config.output_highlighting);
  }

  setDefaultShell(id: string): void {
    this.defaultShellId = id;
    void this.persist();
  }

  setPsReadlineMenuComplete(enabled: boolean): void {
    this.psreadlineMenuComplete = enabled;
    void this.persist();
  }

  setBashPromptColors(enabled: boolean): void {
    this.bashPromptColors = enabled;
    void this.persist();
  }

  setOhMyPoshEnabled(enabled: boolean): void {
    this.ohMyPoshEnabled = enabled;
    void this.persist();
  }

  setOhMyPoshTheme(theme: string): void {
    this.ohMyPoshTheme = theme;
    void this.persist();
  }

  setZshMenuComplete(enabled: boolean): void {
    this.zshMenuComplete = enabled;
    void this.persist();
  }

  setAiModeEnabled(enabled: boolean): void {
    this.aiModeEnabled = enabled;
    void this.persist();
  }

  /** Records that `family` was downloaded to `localPath` so it can be
   * restored offline on the next launch. Pass localPath = null when the
   * user switches to a plain system font instead of a Google Font. */
  setInstalledFont(localPath: string | null): void {
    this.installedFontPath = localPath;
    void this.persist();
  }

  getEffectiveShortcut(action: ShortcutAction): string {
    return resolveKeybinding(action, this.shortcutOverridesFor(action), isMac);
  }

  getShortcutOverride(action: ShortcutAction): string | null {
    const field = SHORTCUT_FIELDS[action][isMac ? "mac" : "other"];
    return this.shortcuts[field];
  }

  /** Every action's current override, keyed by action — what `findConflict`
   * needs to check a candidate binding against all the *other* shortcuts. */
  get allShortcutOverrides(): Partial<Record<ShortcutAction, string | null>> {
    return Object.fromEntries(SHORTCUT_ACTIONS.map((action) => [action, this.getShortcutOverride(action)]));
  }

  setShortcutOverride(action: ShortcutAction, binding: string | null): void {
    const field = SHORTCUT_FIELDS[action][isMac ? "mac" : "other"];
    this.shortcuts = { ...this.shortcuts, [field]: binding };
    void this.persist();
  }

  /** `resolveKeybinding` takes a full action->binding map; build the
   * single-entry slice it needs for whichever action is being resolved. */
  private shortcutOverridesFor(action: ShortcutAction): Partial<Record<ShortcutAction, string | null>> {
    return { [action]: this.getShortcutOverride(action) };
  }

  async recordWindowSize(): Promise<void> {
    if (!this.loaded) return;
    const size = await getWindowSize();
    this.windowWidth = size.width;
    this.windowHeight = size.height;
    void this.persist();
  }

  async persist(): Promise<void> {
    if (!this.loaded) return;
    await saveConfig({
      theme: themeStore.current.name,
      default_shell_id: this.defaultShellId,
      psreadline_menu_complete: this.psreadlineMenuComplete,
      bash_prompt_colors: this.bashPromptColors,
      oh_my_posh_enabled: this.ohMyPoshEnabled,
      oh_my_posh_theme: this.ohMyPoshTheme,
      zsh_menu_complete: this.zshMenuComplete,
      ai_mode_enabled: this.aiModeEnabled,
      font_size_override: themeStore.fontSizeOverride,
      font_family_override: themeStore.fontFamilyOverride,
      installed_font_path: this.installedFontPath,
      background_opacity: themeStore.backgroundOpacity,
      output_highlighting: themeStore.outputHighlightingEnabled,
      window: { width: this.windowWidth, height: this.windowHeight },
      shortcuts: this.shortcuts,
    });
  }
}

export const appConfigStore = new AppConfigStore();
