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
}

export interface AppConfig {
  theme: string;
  default_shell_id: string | null;
  font_size_override: number | null;
  font_family_override: string | null;
  installed_font_path: string | null;
  background_opacity: number;
  output_highlighting: boolean;
  psreadline_menu_complete: boolean;
  bash_prompt_colors: boolean;
  oh_my_posh_enabled: boolean;
  oh_my_posh_theme: string;
  zsh_menu_complete: boolean;
  ai_mode_enabled: boolean;
  window: WindowConfig;
  shortcuts: ShortcutsConfig;
}
