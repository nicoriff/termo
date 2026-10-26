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
    pub tab_menu_mac: Option<String>,
    pub tab_menu_other: Option<String>,
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
    pub toggle_git_mac: Option<String>,
    pub toggle_git_other: Option<String>,
    pub toggle_harness_mac: Option<String>,
    pub toggle_harness_other: Option<String>,
    pub toggle_agents_mac: Option<String>,
    pub toggle_agents_other: Option<String>,
    pub toggle_orchestration_mac: Option<String>,
    pub toggle_orchestration_other: Option<String>,
    pub cycle_tab_mac: Option<String>,
    pub cycle_tab_other: Option<String>,
    pub zoom_tile_mac: Option<String>,
    pub zoom_tile_other: Option<String>,
    pub command_palette_mac: Option<String>,
    pub command_palette_other: Option<String>,
    pub marketplace_mac: Option<String>,
    pub marketplace_other: Option<String>,
    pub diagnostics_mac: Option<String>,
    pub diagnostics_other: Option<String>,
    pub companion_mac: Option<String>,
    pub companion_other: Option<String>,
    pub quick_ask_mac: Option<String>,
    pub quick_ask_other: Option<String>,
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

/// One item of the status bar, as the user placed it: which one, and what
/// its settings say. The ids and options belong to the frontend
/// (src/lib/statusbar/layout.ts), which checks them on reading; here they're
/// only kept.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StatusBarItem {
    pub id: String,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub options: std::collections::BTreeMap<String, String>,
}

/// The status bar arranged in Preferences: what goes on each end, in order.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StatusBarLayout {
    pub left: Vec<StatusBarItem>,
    pub right: Vec<StatusBarItem>,
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
    /// Which tool draws the prompt of new sessions: "none", "oh_my_posh" or
    /// "starship" — only one, since each replaces the shell's prompt. None in
    /// configs from before Starship: see `prompt_engine()`.
    pub prompt_engine: Option<String>,
    /// The switch from before `prompt_engine`, still read from older configs
    /// and kept in step with it for an older Termo reading this one.
    pub oh_my_posh_enabled: bool,
    /// Official Oh My Posh theme name, e.g. "jandedobbeleer".
    pub oh_my_posh_theme: String,
    /// A preset built into starship, e.g. "nerd-font-symbols" — see
    /// pty/starship.rs.
    pub starship_preset: String,
    /// Starship uses the user's own starship.toml instead of the preset.
    pub starship_use_own_config: bool,
    /// Opt-in (macOS/Linux): new zsh sessions get a navigable completion
    /// menu on Tab, Tab — see pty/zsh_init.rs.
    pub zsh_menu_complete: bool,
    /// Opt-in: new consoles run in the sessions daemon (see mux/), so they
    /// keep running when their window closes or Termo quits or updates.
    pub persistent_sessions: bool,
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
    /// The Companion at all: its panel, its notices and the quick question.
    pub companion_enabled: bool,
    /// The Companion's notices on the status bar, as a turn ends.
    pub companion_nudges: bool,
    /// The quick question popup (see companion/quick.rs). Off unless the
    /// user turns it on.
    pub quick_ask_enabled: bool,
    /// Whose CLI answers a quick question: an agent's id, or empty for the
    /// first one installed.
    pub quick_ask_engine: String,
    /// Whether a quick question starts with the session's last exchange
    /// ticked to go along with it.
    pub quick_ask_with_exchange: bool,
    /// The kinds of notice the user asked not to see again.
    pub companion_muted: Vec<String>,
    /// The user's agents also in consoles opened outside Termo: Termo's MCP
    /// server in each CLI's own settings (see agents/console_bridge.rs).
    pub agents_from_any_console: bool,
    /// Consoles may plan and run orchestrations too (see
    /// agents/console_orchestration.rs). Off unless the user turns it on.
    pub orchestrations_from_consoles: bool,
    /// Where the marketplace searches MCP servers first: "official" or
    /// "smithery" (see marketplace/sources.rs).
    pub marketplace_source: String,
    /// Where the user installed an agent's CLI when not npm's own folder,
    /// by agent ("grok" → the folder): see marketplace/agent_install.rs.
    pub agent_install_dirs: std::collections::BTreeMap<String, String>,
    /// Which Node.js each agent's CLI was last found under, by agent
    /// ("codex" → "v20.20.1"): how the frontend tells one that went missing
    /// with a change of Node.js (src/lib/agent-install/missing-agents.ts).
    pub agent_clis_known: std::collections::BTreeMap<String, String>,
    /// The keep-awake button in the title bar (see keep_awake.rs).
    pub keep_awake_button: bool,
    /// The remote access button in the title bar (Tailscale, who's
    /// connected, the address to open from a phone).
    pub remote_button: bool,
    /// The browser links open with (a `browser::Browser::path`); none for
    /// the system's default one.
    pub link_browser: Option<String>,
    /// The agents run in a process of their own that keeps going with Termo
    /// closed (see agents/host/).
    pub background_agents: bool,
    /// That process starts when the user signs in, with no Termo opened.
    pub background_agents_at_login: bool,
    /// Tabs share their strip's whole width instead of keeping their own size.
    pub tabs_fill_width: bool,
    /// Everything git: the panel, the status bar's item, the diff and graph
    /// tabs (see src-tauri/src/git).
    pub git_enabled: bool,
    /// The Git button in the window's vertical bar.
    pub git_in_activity_bar: bool,
    /// The repository's item on the status bar, and what it shows: the
    /// branch, how far it is from its upstream, what changed, its request.
    pub git_status_bar: bool,
    pub git_status_branch: bool,
    pub git_status_sync: bool,
    pub git_status_changes: bool,
    pub git_status_pr: bool,
    /// The status bar as the user arranged it; None for the default one,
    /// which then follows whatever later versions add to it. The git_status_*
    /// fields above are what that default reads for the repository's item,
    /// from before the bar could be arranged.
    pub status_bar_layout: Option<StatusBarLayout>,
    /// Asking gh / glab for the branch's pull request.
    pub git_pr_enabled: bool,
    /// Fetching on its own every so many minutes while a repository is in
    /// view; 0 for never.
    pub git_auto_fetch_minutes: u16,
    /// Code reviews by an agent CLI, in the Git panel (see git/review.rs).
    pub git_review_enabled: bool,
    /// The agent of the user's that reviews; empty for the built-in one.
    pub git_review_agent_id: String,
    /// The agent of the user's a finding is handed to to fix; empty for none.
    pub git_review_fix_agent_id: String,
    /// What a review looks at unless told otherwise: "changes" or "branch".
    pub git_review_scope: String,
    /// The least serious finding shown: "serious", "warning" or "suggestion".
    pub git_review_min_severity: String,
    /// Reviewing the branch before a push from Termo: "off" or "ask".
    pub git_review_before_push: String,
    /// Reviewing the branch when a pull request is opened for it.
    pub git_review_on_pr_create: bool,
    /// What every review is asked to mind, in the user's words.
    pub git_review_instructions: String,
    /// How long a review may take, in minutes.
    pub git_review_timeout_min: u16,
    /// Who wrote each line, in the file editor: "line" (the cursor's line),
    /// "gutter" (every line, in a column) or "off".
    pub git_editor_blame: String,
    /// The commit's message next to its author and date.
    pub git_editor_blame_message: bool,
    /// The file editor's toolbar stays open instead of folding back into its
    /// button when the mouse leaves it.
    pub editor_toolbar_pinned: bool,
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
            prompt_engine: None,
            oh_my_posh_enabled: false,
            oh_my_posh_theme: "jandedobbeleer".into(),
            starship_preset: "nerd-font-symbols".into(),
            starship_use_own_config: false,
            zsh_menu_complete: false,
            persistent_sessions: false,
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
            companion_enabled: true,
            companion_nudges: true,
            quick_ask_enabled: false,
            quick_ask_engine: String::new(),
            quick_ask_with_exchange: false,
            companion_muted: Vec::new(),
            agents_from_any_console: false,
            orchestrations_from_consoles: false,
            marketplace_source: "official".into(),
            agent_install_dirs: std::collections::BTreeMap::new(),
            agent_clis_known: std::collections::BTreeMap::new(),
            keep_awake_button: true,
            remote_button: true,
            link_browser: None,
            background_agents: false,
            background_agents_at_login: false,
            tabs_fill_width: false,
            git_enabled: true,
            git_in_activity_bar: true,
            git_status_bar: true,
            git_status_branch: true,
            git_status_sync: true,
            git_status_changes: true,
            git_status_pr: true,
            status_bar_layout: None,
            git_pr_enabled: true,
            git_auto_fetch_minutes: 0,
            git_review_enabled: true,
            git_review_agent_id: String::new(),
            git_review_fix_agent_id: String::new(),
            git_review_scope: "changes".into(),
            git_review_min_severity: "suggestion".into(),
            git_review_before_push: "off".into(),
            git_review_on_pr_create: false,
            git_review_instructions: String::new(),
            git_review_timeout_min: 10,
            git_editor_blame: "line".into(),
            git_editor_blame_message: true,
            editor_toolbar_pinned: false,
            status_line_wrapped: None,
            last_seen_version: None,
            default_layout: None,
            window: WindowConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

impl AppConfig {
    /// The prompt tool for new sessions; a config from before Starship has
    /// only the Oh My Posh switch, which picked between it and none.
    pub fn prompt_engine(&self) -> &str {
        match self.prompt_engine.as_deref() {
            Some(engine) => engine,
            None if self.oh_my_posh_enabled => "oh_my_posh",
            None => "none",
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

    #[test]
    fn old_configs_keep_oh_my_posh_as_their_prompt() {
        let on: AppConfig = serde_json::from_str(r#"{"oh_my_posh_enabled":true}"#).unwrap();
        assert_eq!(on.prompt_engine(), "oh_my_posh");
        let off: AppConfig = serde_json::from_str(r#"{"oh_my_posh_enabled":false}"#).unwrap();
        assert_eq!(off.prompt_engine(), "none");
        let picked: AppConfig =
            serde_json::from_str(r#"{"oh_my_posh_enabled":true,"prompt_engine":"starship"}"#).unwrap();
        assert_eq!(picked.prompt_engine(), "starship");
    }
}
