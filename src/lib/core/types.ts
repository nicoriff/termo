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
  cycle_tab_mac: string | null;
  cycle_tab_other: string | null;
  zoom_tile_mac: string | null;
  zoom_tile_other: string | null;
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
  theme: string;
  default_shell_id: string | null;
  font_size_override: number | null;
  font_family_override: string | null;
  installed_font_path: string | null;
  background_opacity: number;
  background_blur: boolean;
  output_highlighting: boolean;
  psreadline_menu_complete: boolean;
  bash_prompt_colors: boolean;
  oh_my_posh_enabled: boolean;
  oh_my_posh_theme: string;
  zsh_menu_complete: boolean;
  ai_mode_enabled: boolean;
  ai_notify_awaiting: boolean;
  ai_notify_error: boolean;
  ai_notify_finished: boolean;
  ai_dock_badge: boolean;
  window: WindowConfig;
  shortcuts: ShortcutsConfig;
}
