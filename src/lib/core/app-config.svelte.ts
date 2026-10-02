import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getConfig, saveConfig } from "./config-client";
import { getWindowSize, setWindowSize } from "./window-client";
import { themeStore } from "../theme/theme-store.svelte";
import { i18n, type LanguagePreference } from "../i18n/i18n.svelte";
import { injectFontFace } from "../theme/google-fonts";
import { isMac } from "./platform";
import { resolveKeybinding, SHORTCUT_ACTIONS, type ShortcutAction } from "./shortcuts";
import type { AppConfig, ShortcutsConfig } from "./types";
import { DEFAULT_SOUNDS, isSoundChoice, type SoundChoice, type SoundEvent } from "$lib/ai-mode/sounds";
import type { ToolId } from "$lib/ai-mode/pattern-table";

const EMPTY_SHORTCUTS: ShortcutsConfig = {
  new_tab_mac: null,
  new_tab_other: null,
  new_claude_mac: null,
  new_claude_other: null,
  new_codex_mac: null,
  new_codex_other: null,
  new_gemini_mac: null,
  new_gemini_other: null,
  new_copilot_mac: null,
  new_copilot_other: null,
  favorite_tab_mac: null,
  favorite_tab_other: null,
  new_window_mac: null,
  new_window_other: null,
  toggle_explorer_mac: null,
  toggle_explorer_other: null,
  toggle_agents_mac: null,
  toggle_agents_other: null,
  cycle_tab_mac: null,
  cycle_tab_other: null,
  zoom_tile_mac: null,
  zoom_tile_other: null,
  command_palette_mac: null,
  command_palette_other: null,
  marketplace_mac: null,
  marketplace_other: null,
  copy_mac: null,
  copy_other: null,
  paste_mac: null,
  paste_other: null,
  select_all_mac: null,
  select_all_other: null,
  ai_running_mac: null,
  ai_running_other: null,
  ai_awaiting_mac: null,
  ai_awaiting_other: null,
  ai_error_mac: null,
  ai_error_other: null,
  ai_finished_mac: null,
  ai_finished_other: null,
  ai_next_pending_mac: null,
  ai_next_pending_other: null,
};

const SHORTCUT_FIELDS: Record<ShortcutAction, { mac: keyof ShortcutsConfig; other: keyof ShortcutsConfig }> = {
  newTab: { mac: "new_tab_mac", other: "new_tab_other" },
  newClaude: { mac: "new_claude_mac", other: "new_claude_other" },
  newCodex: { mac: "new_codex_mac", other: "new_codex_other" },
  newGemini: { mac: "new_gemini_mac", other: "new_gemini_other" },
  newCopilot: { mac: "new_copilot_mac", other: "new_copilot_other" },
  favoriteTab: { mac: "favorite_tab_mac", other: "favorite_tab_other" },
  newWindow: { mac: "new_window_mac", other: "new_window_other" },
  toggleExplorer: { mac: "toggle_explorer_mac", other: "toggle_explorer_other" },
  toggleAgents: { mac: "toggle_agents_mac", other: "toggle_agents_other" },
  cycleTab: { mac: "cycle_tab_mac", other: "cycle_tab_other" },
  zoomTile: { mac: "zoom_tile_mac", other: "zoom_tile_other" },
  commandPalette: { mac: "command_palette_mac", other: "command_palette_other" },
  marketplace: { mac: "marketplace_mac", other: "marketplace_other" },
  copy: { mac: "copy_mac", other: "copy_other" },
  paste: { mac: "paste_mac", other: "paste_other" },
  selectAll: { mac: "select_all_mac", other: "select_all_other" },
  aiRunning: { mac: "ai_running_mac", other: "ai_running_other" },
  aiAwaiting: { mac: "ai_awaiting_mac", other: "ai_awaiting_other" },
  aiError: { mac: "ai_error_mac", other: "ai_error_other" },
  aiFinished: { mac: "ai_finished_mac", other: "ai_finished_other" },
  aiNextPending: { mac: "ai_next_pending_mac", other: "ai_next_pending_other" },
};

interface InitOptions {
  /** MainWindow restores the persisted window size on launch; Preferences
   * opens at its own fixed size and must not fight that. Defaults to true. */
  restoreWindowSize?: boolean;
}

class AppConfigStore {
  language = $state<LanguagePreference>("system");
  defaultShellId = $state<string | null>(null);
  psreadlineMenuComplete = $state(false);
  /** Windows: new PowerShell and CMD sessions start in UTF-8. */
  consoleUtf8 = $state(true);
  bashPromptColors = $state(false);
  ohMyPoshEnabled = $state(false);
  ohMyPoshTheme = $state("jandedobbeleer");
  zshMenuComplete = $state(false);
  backgroundBlur = $state(false);
  aiModeEnabled = $state(true);
  aiNotifyAwaiting = $state(true);
  aiNotifyError = $state(true);
  aiNotifyFinished = $state(true);
  /** Whether each agent's sounds play at all. */
  aiSoundsEnabled = $state<Record<ToolId, boolean>>({
    claude: true,
    codex: true,
    gemini: true,
    copilot: true,
  });
  /** Per agent, so each can be told apart by ear. */
  aiSounds = $state<Record<ToolId, Record<SoundEvent, SoundChoice>>>(structuredClone(DEFAULT_SOUNDS));
  aiSoundVolume = $state(70);
  aiDockBadge = $state(true);
  /** The Agents button in the window's vertical bar. */
  agentsInActivityBar = $state(true);
  /** The keep-awake button in the title bar. */
  keepAwakeButton = $state(true);
  tabsFillWidth = $state(false);
  /** Saved layout to open on launch, or null for a single console. */
  defaultLayout = $state<string | null>(null);
  /** Set by the backend when it chains onto the user's Claude Code status
   * line; kept here only so persisting doesn't drop it. */
  private statusLineWrapped: string | null = null;
  /** The version that last ran, to show what an update brought (see
   * WhatsNewDialog). Null until the first launch that knows about it. */
  lastSeenVersion: string | null = null;
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
    this.language = config.language === "es" || config.language === "en" ? config.language : "system";
    i18n.setPreference(this.language);
    this.defaultShellId = config.default_shell_id;
    this.psreadlineMenuComplete = config.psreadline_menu_complete;
    this.consoleUtf8 = config.console_utf8 ?? true;
    this.bashPromptColors = config.bash_prompt_colors;
    this.ohMyPoshEnabled = config.oh_my_posh_enabled;
    this.ohMyPoshTheme = config.oh_my_posh_theme;
    this.zshMenuComplete = config.zsh_menu_complete;
    this.backgroundBlur = config.background_blur;
    this.aiModeEnabled = config.ai_mode_enabled;
    this.aiNotifyAwaiting = config.ai_notify_awaiting;
    this.aiNotifyError = config.ai_notify_error;
    this.aiNotifyFinished = config.ai_notify_finished;
    // A name no longer in the bank (or a hand-edited typo) falls back to the
    // default rather than to silence.
    this.aiSoundsEnabled = {
      claude: config.ai_sound_claude_enabled,
      codex: config.ai_sound_codex_enabled,
      // Absent in configs from before Gemini CLI was supported.
      gemini: config.ai_sound_gemini_enabled ?? true,
      copilot: config.ai_sound_copilot_enabled ?? true,
    };
    const sound = (value: string, fallback: SoundChoice) => (isSoundChoice(value) ? value : fallback);
    this.aiSounds = {
      claude: {
        awaiting: sound(config.ai_sound_claude_awaiting, DEFAULT_SOUNDS.claude.awaiting),
        finished: sound(config.ai_sound_claude_finished, DEFAULT_SOUNDS.claude.finished),
      },
      codex: {
        awaiting: sound(config.ai_sound_codex_awaiting, DEFAULT_SOUNDS.codex.awaiting),
        finished: sound(config.ai_sound_codex_finished, DEFAULT_SOUNDS.codex.finished),
      },
      gemini: {
        awaiting: sound(config.ai_sound_gemini_awaiting ?? "", DEFAULT_SOUNDS.gemini.awaiting),
        finished: sound(config.ai_sound_gemini_finished ?? "", DEFAULT_SOUNDS.gemini.finished),
      },
      copilot: {
        awaiting: sound(config.ai_sound_copilot_awaiting ?? "", DEFAULT_SOUNDS.copilot.awaiting),
        finished: sound(config.ai_sound_copilot_finished ?? "", DEFAULT_SOUNDS.copilot.finished),
      },
    };
    this.aiSoundVolume = config.ai_sound_volume;
    this.aiDockBadge = config.ai_dock_badge;
    this.agentsInActivityBar = config.agents_in_activity_bar ?? true;
    this.keepAwakeButton = config.keep_awake_button ?? true;
    // Absent in configs from before it was an option: as tabs looked then.
    this.tabsFillWidth = config.tabs_fill_width ?? false;
    this.defaultLayout = config.default_layout;
    this.statusLineWrapped = config.status_line_wrapped;
    this.lastSeenVersion = config.last_seen_version ?? null;
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

  setLanguage(language: LanguagePreference): void {
    this.language = language;
    i18n.setPreference(language);
    void this.persist();
  }

  setDefaultShell(id: string): void {
    this.defaultShellId = id;
    void this.persist();
  }

  setConsoleUtf8(enabled: boolean): void {
    this.consoleUtf8 = enabled;
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

  setBackgroundBlur(enabled: boolean): void {
    this.backgroundBlur = enabled;
    void this.persist();
  }

  setAiModeEnabled(enabled: boolean): void {
    this.aiModeEnabled = enabled;
    void this.persist();
  }

  setAiNotifyAwaiting(enabled: boolean): void {
    this.aiNotifyAwaiting = enabled;
    void this.persist();
  }

  setAiSoundsEnabled(tool: ToolId, enabled: boolean): void {
    this.aiSoundsEnabled = { ...this.aiSoundsEnabled, [tool]: enabled };
    void this.persist();
  }

  setAiSound(tool: ToolId, event: SoundEvent, choice: SoundChoice): void {
    this.aiSounds = { ...this.aiSounds, [tool]: { ...this.aiSounds[tool], [event]: choice } };
    void this.persist();
  }

  setAiSoundVolume(volume: number): void {
    this.aiSoundVolume = Math.round(Math.min(100, Math.max(0, volume)));
    void this.persist();
  }

  setAiNotifyError(enabled: boolean): void {
    this.aiNotifyError = enabled;
    void this.persist();
  }

  setAiNotifyFinished(enabled: boolean): void {
    this.aiNotifyFinished = enabled;
    void this.persist();
  }

  setAiDockBadge(enabled: boolean): void {
    this.aiDockBadge = enabled;
    void this.persist();
  }

  setAgentsInActivityBar(visible: boolean): void {
    this.agentsInActivityBar = visible;
    void this.persist();
  }

  setKeepAwakeButton(visible: boolean): void {
    this.keepAwakeButton = visible;
    void this.persist();
  }

  setTabsFillWidth(fill: boolean): void {
    this.tabsFillWidth = fill;
    void this.persist();
  }

  setLastSeenVersion(version: string): void {
    this.lastSeenVersion = version;
    void this.persist();
  }

  /** Null means launching into a single console, as before. */
  setDefaultLayout(name: string | null): void {
    this.defaultLayout = name;
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
      language: this.language,
      theme: themeStore.current.name,
      default_shell_id: this.defaultShellId,
      psreadline_menu_complete: this.psreadlineMenuComplete,
      console_utf8: this.consoleUtf8,
      bash_prompt_colors: this.bashPromptColors,
      oh_my_posh_enabled: this.ohMyPoshEnabled,
      oh_my_posh_theme: this.ohMyPoshTheme,
      zsh_menu_complete: this.zshMenuComplete,
      background_blur: this.backgroundBlur,
      ai_mode_enabled: this.aiModeEnabled,
      ai_notify_awaiting: this.aiNotifyAwaiting,
      ai_notify_error: this.aiNotifyError,
      ai_notify_finished: this.aiNotifyFinished,
      ai_sound_claude_enabled: this.aiSoundsEnabled.claude,
      ai_sound_codex_enabled: this.aiSoundsEnabled.codex,
      ai_sound_gemini_enabled: this.aiSoundsEnabled.gemini,
      ai_sound_copilot_enabled: this.aiSoundsEnabled.copilot,
      ai_sound_claude_awaiting: this.aiSounds.claude.awaiting,
      ai_sound_claude_finished: this.aiSounds.claude.finished,
      ai_sound_codex_awaiting: this.aiSounds.codex.awaiting,
      ai_sound_codex_finished: this.aiSounds.codex.finished,
      ai_sound_gemini_awaiting: this.aiSounds.gemini.awaiting,
      ai_sound_gemini_finished: this.aiSounds.gemini.finished,
      ai_sound_copilot_awaiting: this.aiSounds.copilot.awaiting,
      ai_sound_copilot_finished: this.aiSounds.copilot.finished,
      ai_sound_volume: this.aiSoundVolume,
      ai_dock_badge: this.aiDockBadge,
      agents_in_activity_bar: this.agentsInActivityBar,
      keep_awake_button: this.keepAwakeButton,
      tabs_fill_width: this.tabsFillWidth,
      default_layout: this.defaultLayout,
      status_line_wrapped: this.statusLineWrapped,
      last_seen_version: this.lastSeenVersion,
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
