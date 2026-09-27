use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowConfig {
    pub width: f64,
    pub height: f64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1000.0,
            height: 640.0,
        }
    }
}

/// User-remapped shortcuts, `None` per field meaning "use the built-in
/// default for that platform" (see `src/lib/core/shortcuts.ts`, which owns
/// the actual default bindings and matching logic — this struct only
/// persists overrides). Two buckets per action (`_mac` / `_other`) rather
/// than three: no shortcut in this app distinguishes Windows from Linux
/// today, only mac vs. not-mac, so a third bucket would have nothing to
/// hold.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ShortcutsConfig {
    pub new_tab_mac: Option<String>,
    pub new_tab_other: Option<String>,
    pub new_claude_mac: Option<String>,
    pub new_claude_other: Option<String>,
    pub new_codex_mac: Option<String>,
    pub new_codex_other: Option<String>,
    pub cycle_tab_mac: Option<String>,
    pub cycle_tab_other: Option<String>,
    pub zoom_tile_mac: Option<String>,
    pub zoom_tile_other: Option<String>,
    pub command_palette_mac: Option<String>,
    pub command_palette_other: Option<String>,
    pub marketplace_mac: Option<String>,
    pub marketplace_other: Option<String>,
    pub copy_mac: Option<String>,
    pub copy_other: Option<String>,
    pub paste_mac: Option<String>,
    pub paste_other: Option<String>,
    pub select_all_mac: Option<String>,
    pub select_all_other: Option<String>,
    pub ai_running_mac: Option<String>,
    pub ai_running_other: Option<String>,
    pub ai_awaiting_mac: Option<String>,
    pub ai_awaiting_other: Option<String>,
    pub ai_error_mac: Option<String>,
    pub ai_error_other: Option<String>,
    pub ai_finished_mac: Option<String>,
    pub ai_finished_other: Option<String>,
    pub ai_next_pending_mac: Option<String>,
    pub ai_next_pending_other: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    /// UI language: "system" (follow the OS), "es" or "en".
    pub language: String,
    pub theme: String,
    pub default_shell_id: Option<String>,
    pub font_size_override: Option<u16>,
    pub font_family_override: Option<String>,
    /// Local file path of a downloaded Google Font woff2, if the current
    /// font_family_override was installed rather than typed as a plain
    /// system font name. Lets startup re-apply it with zero network access.
    pub installed_font_path: Option<String>,
    /// Window/terminal background opacity, 0 (fully transparent) to 100 (opaque).
    pub background_opacity: u8,
    /// Native blur of whatever is behind the window's translucent areas
    /// (macOS vibrancy, Windows Acrylic); no effect on Linux.
    pub background_blur: bool,
    /// Whether terminal output gets colorized after the fact (IPs, URLs,
    /// paths, errors, etc.) — see src/lib/terminal/output-highlighter.ts.
    pub output_highlighting: bool,
    /// Opt-in: new PowerShell/pwsh sessions get a PSReadLine key handler
    /// (Tab completes, Tab,Tab opens a navigable menu) injected at startup.
    /// Left off by default since it changes shell startup behavior.
    pub psreadline_menu_complete: bool,
    /// Opt-in (macOS/Linux): new bash sessions get a colored prompt, colored
    /// `ls`, and no macOS "default shell is now zsh" notice — see
    /// pty/bash_colors.rs.
    pub bash_prompt_colors: bool,
    /// Opt-in: new PowerShell/zsh/bash 4.2+ sessions get an Oh My Posh
    /// prompt — see pty/oh_my_posh.rs.
    pub oh_my_posh_enabled: bool,
    /// Official Oh My Posh theme name, e.g. "jandedobbeleer".
    pub oh_my_posh_theme: String,
    /// Opt-in (macOS/Linux): new zsh sessions get a navigable completion
    /// menu on Tab, Tab — see pty/zsh_init.rs.
    pub zsh_menu_complete: bool,
    /// Opt-in: tracks Claude Code/Codex session state (running/awaiting/
    /// error/finished) via terminal screen-buffer pattern matching, shown
    /// in a top panel, per-tab status dots, and the app status bar.
    pub ai_mode_enabled: bool,
    /// Modo IA system notifications, per status transition.
    pub ai_notify_awaiting: bool,
    pub ai_notify_error: bool,
    pub ai_notify_finished: bool,
    /// Whether each agent's sounds play at all; switching them off keeps the
    /// picks below for when they're switched back on.
    pub ai_sound_claude_enabled: bool,
    pub ai_sound_codex_enabled: bool,
    /// Sound played when an agent's console starts waiting on the user /
    /// finishes a turn, per agent so they can be told apart by ear: one of
    /// the ids in src/lib/ai-mode/sounds.ts, or "none".
    pub ai_sound_claude_awaiting: String,
    pub ai_sound_claude_finished: String,
    pub ai_sound_codex_awaiting: String,
    pub ai_sound_codex_finished: String,
    /// 0–100.
    pub ai_sound_volume: u8,
    /// App icon badge with the number of sessions waiting on the user.
    pub ai_dock_badge: bool,
    /// The user's own Claude Code `statusLine` command, saved when Termo
    /// chained onto it so uninstalling can put it back (pty/status_line.rs).
    pub status_line_wrapped: Option<String>,
    /// Name of the saved layout to open on launch (see layouts.rs), or None
    /// to start with a single console. A name that no longer exists is
    /// ignored rather than treated as an error.
    pub default_layout: Option<String>,
    pub window: WindowConfig,
    pub shortcuts: ShortcutsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "system".into(),
            theme: "Dark".into(),
            default_shell_id: None,
            font_size_override: None,
            font_family_override: None,
            installed_font_path: None,
            background_opacity: 100,
            background_blur: false,
            output_highlighting: true,
            psreadline_menu_complete: false,
            bash_prompt_colors: false,
            oh_my_posh_enabled: false,
            oh_my_posh_theme: "jandedobbeleer".into(),
            zsh_menu_complete: false,
            ai_mode_enabled: false,
            ai_notify_awaiting: true,
            ai_notify_error: true,
            ai_notify_finished: true,
            ai_sound_claude_enabled: true,
            ai_sound_codex_enabled: true,
            ai_sound_claude_awaiting: "alert".into(),
            ai_sound_claude_finished: "chime".into(),
            ai_sound_codex_awaiting: "pop".into(),
            ai_sound_codex_finished: "marimba".into(),
            ai_sound_volume: 70,
            ai_dock_badge: true,
            status_line_wrapped: None,
            default_layout: None,
            window: WindowConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}
