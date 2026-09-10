export type PtyEvent = { type: "data"; data: string } | { type: "exit"; code: number };

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

export interface AppConfig {
  theme: string;
  default_shell_id: string | null;
  font_size_override: number | null;
  font_family_override: string | null;
  installed_font_path: string | null;
  background_opacity: number;
  output_highlighting: boolean;
  psreadline_menu_complete: boolean;
  window: WindowConfig;
}
