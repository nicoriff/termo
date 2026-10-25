#[cfg(unix)]
use std::fs;

use serde::Serialize;
use tauri::Manager;

use super::download::{blocking, download_to};
use crate::pty::oh_my_posh;

const NERD_FONT_URL: &str =
    "https://raw.githubusercontent.com/ryanoasis/nerd-fonts/master/patched-fonts/Meslo/S/MesloLGSNerdFontMono-Regular.ttf";

#[derive(Serialize)]
pub struct OhMyPoshStatus {
    path: Option<String>,
    managed: bool,
}

#[tauri::command]
pub fn oh_my_posh_status(app: tauri::AppHandle) -> OhMyPoshStatus {
    match oh_my_posh::find(&app) {
        Some((path, managed)) => OhMyPoshStatus {
            path: Some(path.to_string_lossy().into_owned()),
            managed,
        },
        None => OhMyPoshStatus {
            path: None,
            managed: false,
        },
    }
}

#[tauri::command]
pub async fn install_oh_my_posh(app: tauri::AppHandle) -> Result<String, String> {
    blocking(move || {
        let url = oh_my_posh::download_url()
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::OhMyPoshNoBinary)))?;
        let target = oh_my_posh::managed_binary(&app)?;
        download_to(&url, &target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        }
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}

#[tauri::command]
pub async fn install_oh_my_posh_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    if !oh_my_posh::is_valid_theme_name(&theme) {
        return Err(crate::i18n::tr(crate::i18n::Msg::InvalidThemeName(&theme)));
    }
    blocking(move || {
        let url = format!("https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/{theme}.omp.json");
        download_to(&url, &oh_my_posh::theme_file(&app, &theme)?)
    })
    .await
}

/// Saved next to the Google Fonts installs so it goes through the same
/// asset-protocol scope and startup restore path.
#[tauri::command]
pub async fn install_nerd_font(app: tauri::AppHandle) -> Result<String, String> {
    blocking(move || {
        let target = app
            .path()
            .app_config_dir()?
            .join("fonts")
            .join("MesloLGS_Nerd_Font_Mono.ttf");
        download_to(NERD_FONT_URL, &target)?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}
