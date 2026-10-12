export type PtyEvent =
  | { type: "data"; data: string }
  | { type: "exit"; code: number }
  | { type: "foreground_process"; path: string | null };

export interface ShellInfo {
  id: string;
  display_name: string;
  path: string;
  args: string[];
}

export interface WindowConfig {
  width: number;
  height: number;
}

export interface ShortcutsConfig {
  new_tab_mac: string | null;
  new_tab_other: string | null;
  new_claude_mac: string | null;
  new_claude_other: string | null;
  new_codex_mac: string | null;
  new_codex_other: string | null;
  new_gemini_mac: string | null;
  new_gemini_other: string | null;
  new_copilot_mac: string | null;
  new_copilot_other: string | null;
  new_grok_mac: string | null;
  new_grok_other: string | null;
  favorite_tab_mac: string | null;
  favorite_tab_other: string | null;
  new_window_mac: string | null;
  new_window_other: string | null;
  toggle_explorer_mac: string | null;
  toggle_explorer_other: string | null;
  toggle_agents_mac: string | null;
  toggle_agents_other: string | null;
  cycle_tab_mac: string | null;
  cycle_tab_other: string | null;
  zoom_tile_mac: string | null;
  zoom_tile_other: string | null;
  command_palette_mac: string | null;
  command_palette_other: string | null;
  marketplace_mac: string | null;
  marketplace_other: string | null;
  diagnostics_mac: string | null;
  diagnostics_other: string | null;
  companion_mac: string | null;
  companion_other: string | null;
  quick_ask_mac: string | null;
  quick_ask_other: string | null;
  copy_mac: string | null;
  copy_other: string | null;
  paste_mac: string | null;
  paste_other: string | null;
  select_all_mac: string | null;
  select_all_other: string | null;
  ai_running_mac: string | null;
  ai_running_other: string | null;
  ai_awaiting_mac: string | null;
  ai_awaiting_other: string | null;
  ai_error_mac: string | null;
  ai_error_other: string | null;
  ai_finished_mac: string | null;
  ai_finished_other: string | null;
  ai_next_pending_mac: string | null;
  ai_next_pending_other: string | null;
}

export interface AppConfig {
  language: "system" | "es" | "en";
  /** User-selected theme, restored when OS synchronization is disabled. */
  theme: string;
  sync_with_os: boolean;
  light_theme: string | null;
  dark_theme: string | null;
  default_shell_id: string | null;
  font_size_override: number | null;
  font_family_override: string | null;
  installed_font_path: string | null;
  background_opacity: number;
  background_blur: boolean;
  output_highlighting: boolean;
  psreadline_menu_complete: boolean;
  console_utf8: boolean;
  bash_prompt_colors: boolean;
  oh_my_posh_enabled: boolean;
  oh_my_posh_theme: string;
  zsh_menu_complete: boolean;
  ai_mode_enabled: boolean;
  ai_notify_awaiting: boolean;
  ai_notify_error: boolean;
  ai_notify_finished: boolean;
  ai_sound_claude_enabled: boolean;
  ai_sound_codex_enabled: boolean;
  ai_sound_gemini_enabled: boolean;
  ai_sound_copilot_enabled: boolean;
  ai_sound_grok_enabled: boolean;
  ai_sound_claude_awaiting: string;
  ai_sound_claude_finished: string;
  ai_sound_codex_awaiting: string;
  ai_sound_codex_finished: string;
  ai_sound_gemini_awaiting: string;
  ai_sound_gemini_finished: string;
  ai_sound_copilot_awaiting: string;
  ai_sound_copilot_finished: string;
  ai_sound_grok_awaiting: string;
  ai_sound_grok_finished: string;
  ai_sound_volume: number;
  ai_dock_badge: boolean;
  agents_in_activity_bar: boolean;
  companion_enabled?: boolean;
  companion_nudges?: boolean;
  quick_ask_enabled?: boolean;
  quick_ask_engine?: string;
  quick_ask_with_exchange?: boolean;
  companion_muted?: string[];
  agents_from_any_console?: boolean;
  marketplace_source?: string;
  keep_awake_button: boolean;
  /** Where the user installed an agent's CLI when not npm's own folder, by
   * agent. Absent in configs from before Termo could install them. */
  agent_install_dirs?: Record<string, string>;
  tabs_fill_width: boolean;
  /** The user's own Claude Code statusLine command, saved when Termo chained
   * onto it. Never edited here — carried through so saving doesn't drop it. */
  status_line_wrapped: string | null;
  /** The version that last ran (see WhatsNewDialog). */
  last_seen_version: string | null;
  /** Saved layout to open on launch, or null for a single console. */
  default_layout: string | null;
  window: WindowConfig;
  shortcuts: ShortcutsConfig;
}
