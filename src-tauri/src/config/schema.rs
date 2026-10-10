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
    pub new_gemini_mac: Option<String>,
    pub new_gemini_other: Option<String>,
    pub new_copilot_mac: Option<String>,
    pub new_copilot_other: Option<String>,
    pub new_grok_mac: Option<String>,
    pub new_grok_other: Option<String>,
    pub favorite_tab_mac: Option<String>,
    pub favorite_tab_other: Option<String>,
    pub new_window_mac: Option<String>,
    pub new_window_other: Option<String>,
    pub toggle_explorer_mac: Option<String>,
    pub toggle_explorer_other: Option<String>,
    pub toggle_agents_mac: Option<String>,
    pub toggle_agents_other: Option<String>,
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
    /// Follow the OS appearance using one saved theme per category.
    pub sync_with_os: bool,
    /// None in older configs: the frontend seeds the matching category from `theme`.
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
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
    /// Windows: new PowerShell and CMD sessions start in UTF-8 (see
    /// pty/utf8_console.rs). On by default — also for configs from before it.
    pub console_utf8: bool,
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
    /// On by default: the master switch for everything AI (see ai_mode.rs) —
    /// tracking the agent CLIs' sessions (running/awaiting/error/finished) in
    /// tabs, the panel and the status bar, plus agents and extensions.
    pub ai_mode_enabled: bool,
    /// Modo IA system notifications, per status transition.
    pub ai_notify_awaiting: bool,
    pub ai_notify_error: bool,
    pub ai_notify_finished: bool,
    /// Whether each agent's sounds play at all; switching them off keeps the
    /// picks below for when they're switched back on.
    pub ai_sound_claude_enabled: bool,
    pub ai_sound_codex_enabled: bool,
    pub ai_sound_gemini_enabled: bool,
    pub ai_sound_copilot_enabled: bool,
    pub ai_sound_grok_enabled: bool,
    /// Sound played when an agent's console starts waiting on the user /
    /// finishes a turn, per agent so they can be told apart by ear: one of
    /// the ids in src/lib/ai-mode/sounds.ts, or "none".
    pub ai_sound_claude_awaiting: String,
    pub ai_sound_claude_finished: String,
    pub ai_sound_codex_awaiting: String,
    pub ai_sound_codex_finished: String,
    pub ai_sound_gemini_awaiting: String,
    pub ai_sound_gemini_finished: String,
    pub ai_sound_copilot_awaiting: String,
    pub ai_sound_copilot_finished: String,
    pub ai_sound_grok_awaiting: String,
    pub ai_sound_grok_finished: String,
    /// 0–100.
    pub ai_sound_volume: u8,
    /// App icon badge with the number of sessions waiting on the user.
    pub ai_dock_badge: bool,
    /// The Agents button in the window's vertical bar.
    pub agents_in_activity_bar: bool,
    /// The user's agents also in consoles opened outside Termo: Termo's MCP
    /// server in each CLI's own settings (see agents/console_bridge.rs).
    pub agents_from_any_console: bool,
    /// Where the marketplace searches MCP servers first: "official" or
    /// "smithery" (see marketplace/sources.rs).
    pub marketplace_source: String,
    /// Where the user installed an agent's CLI when not npm's own folder,
    /// by agent ("grok" → the folder): see marketplace/agent_install.rs.
    pub agent_install_dirs: std::collections::BTreeMap<String, String>,
    /// The keep-awake button in the title bar (see keep_awake.rs).
    pub keep_awake_button: bool,
    /// Tabs share their strip's whole width instead of keeping their own size.
    pub tabs_fill_width: bool,
    /// The user's own Claude Code `statusLine` command, saved when Termo
    /// chained onto it so uninstalling can put it back (pty/status_line.rs).
    pub status_line_wrapped: Option<String>,
    /// The version that last ran, so the first launch after an update can
    /// show what it brought (src/lib/app/WhatsNewDialog.svelte).
    pub last_seen_version: Option<String>,
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
            sync_with_os: false,
            light_theme: None,
            dark_theme: None,
            default_shell_id: None,
            font_size_override: None,
            font_family_override: None,
            installed_font_path: None,
            background_opacity: 100,
            background_blur: false,
            output_highlighting: true,
            psreadline_menu_complete: false,
            console_utf8: true,
            bash_prompt_colors: false,
            oh_my_posh_enabled: false,
            oh_my_posh_theme: "jandedobbeleer".into(),
            zsh_menu_complete: false,
            ai_mode_enabled: true,
            ai_notify_awaiting: true,
            ai_notify_error: true,
            ai_notify_finished: true,
            ai_sound_claude_enabled: true,
            ai_sound_codex_enabled: true,
            ai_sound_gemini_enabled: true,
            ai_sound_copilot_enabled: true,
            ai_sound_grok_enabled: true,
            ai_sound_claude_awaiting: "alert".into(),
            ai_sound_claude_finished: "chime".into(),
            ai_sound_codex_awaiting: "pop".into(),
            ai_sound_codex_finished: "marimba".into(),
            // Each agent gets its own pair, so they are told apart by ear.
            ai_sound_gemini_awaiting: "doorbell".into(),
            ai_sound_gemini_finished: "harp".into(),
            ai_sound_copilot_awaiting: "knock".into(),
            ai_sound_copilot_finished: "success".into(),
            ai_sound_grok_awaiting: "blip".into(),
            ai_sound_grok_finished: "bell".into(),
            ai_sound_volume: 70,
            ai_dock_badge: true,
            agents_in_activity_bar: true,
            agents_from_any_console: false,
            marketplace_source: "official".into(),
            agent_install_dirs: std::collections::BTreeMap::new(),
            keep_awake_button: true,
            tabs_fill_width: false,
            status_line_wrapped: None,
            last_seen_version: None,
            default_layout: None,
            window: WindowConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn old_configs_keep_their_manual_theme_and_leave_categories_unset() {
        let config: AppConfig =
            serde_json::from_str(r#"{"theme":"Dracula","language":"es","background_opacity":78}"#)
                .unwrap();
        assert_eq!(config.theme, "Dracula");
        assert_eq!(config.language, "es");
        assert_eq!(config.background_opacity, 78);
        assert!(!config.sync_with_os);
        assert!(config.light_theme.is_none());
        assert!(config.dark_theme.is_none());
    }

    #[test]
    fn appearance_preferences_round_trip_without_replacing_the_manual_theme() {
        let config = AppConfig {
            theme: "Nord".into(),
            sync_with_os: true,
            light_theme: Some("Solarized Light".into()),
            dark_theme: Some("Dracula".into()),
            ..AppConfig::default()
        };
        let restored: AppConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.theme, "Nord");
        assert!(restored.sync_with_os);
        assert_eq!(restored.light_theme.as_deref(), Some("Solarized Light"));
        assert_eq!(restored.dark_theme.as_deref(), Some("Dracula"));
    }
}
