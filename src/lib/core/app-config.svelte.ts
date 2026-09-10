import { getConfig, saveConfig } from "./config-client";
import { getWindowSize, setWindowSize } from "./window-client";
import { themeStore } from "../theme/theme-store.svelte";
import { injectFontFace } from "../theme/google-fonts";

class AppConfigStore {
  defaultShellId = $state<string | null>(null);
  psreadlineMenuComplete = $state(false);
  loaded = $state(false);

  private windowWidth = 1000;
  private windowHeight = 640;
  private installedFontPath: string | null = null;

  async init(): Promise<void> {
    const config = await getConfig();

    this.defaultShellId = config.default_shell_id;
    this.psreadlineMenuComplete = config.psreadline_menu_complete;
    this.installedFontPath = config.installed_font_path;
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
    this.windowWidth = config.window.width;
    this.windowHeight = config.window.height;

    try {
      await setWindowSize(config.window.width, config.window.height);
    } catch {
      // Non-critical — keep the default size from tauri.conf.json.
    }

    this.loaded = true;
  }

  setDefaultShell(id: string): void {
    this.defaultShellId = id;
    void this.persist();
  }

  setPsReadlineMenuComplete(enabled: boolean): void {
    this.psreadlineMenuComplete = enabled;
    void this.persist();
  }

  /** Records that `family` was downloaded to `localPath` so it can be
   * restored offline on the next launch. Pass localPath = null when the
   * user switches to a plain system font instead of a Google Font. */
  setInstalledFont(localPath: string | null): void {
    this.installedFontPath = localPath;
    void this.persist();
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
      font_size_override: themeStore.fontSizeOverride,
      font_family_override: themeStore.fontFamilyOverride,
      installed_font_path: this.installedFontPath,
      background_opacity: themeStore.backgroundOpacity,
      output_highlighting: themeStore.outputHighlightingEnabled,
      window: { width: this.windowWidth, height: this.windowHeight },
    });
  }
}

export const appConfigStore = new AppConfigStore();
