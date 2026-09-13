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
    pub cycle_tab_mac: Option<String>,
    pub cycle_tab_other: Option<String>,
    pub zoom_tile_mac: Option<String>,
    pub zoom_tile_other: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
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
    pub window: WindowConfig,
    pub shortcuts: ShortcutsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "Dark".into(),
            default_shell_id: None,
            font_size_override: None,
            font_family_override: None,
            installed_font_path: None,
            background_opacity: 100,
            output_highlighting: true,
            psreadline_menu_complete: false,
            bash_prompt_colors: false,
            oh_my_posh_enabled: false,
            oh_my_posh_theme: "jandedobbeleer".into(),
            zsh_menu_complete: false,
            ai_mode_enabled: false,
            window: WindowConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}
