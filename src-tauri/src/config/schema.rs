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
    /// Opt-in: tracks Claude Code/Codex session state (running/awaiting/
    /// error/finished) via terminal screen-buffer pattern matching, shown
    /// in a top panel, per-tab status dots, and the app status bar.
    pub ai_mode_enabled: bool,
    pub window: WindowConfig,
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
            ai_mode_enabled: false,
            window: WindowConfig::default(),
        }
    }
}
