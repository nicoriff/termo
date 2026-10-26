use serde::Serialize;

use super::download::{blocking, fetch, write_atomically};
use crate::pty::starship;

#[derive(Serialize)]
pub struct StarshipStatus {
    path: Option<String>,
    managed: bool,
}

#[tauri::command]
pub fn starship_status(app: tauri::AppHandle) -> StarshipStatus {
    match starship::find(&app) {
        Some((path, managed)) => StarshipStatus {
            path: Some(path.to_string_lossy().into_owned()),
            managed,
        },
        None => StarshipStatus {
            path: None,
            managed: false,
        },
    }
}

#[tauri::command]
pub async fn install_starship(app: tauri::AppHandle) -> Result<String, String> {
    blocking(move || {
        let url = starship::download_url()
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::StarshipNoBinary)))?;
        let binary = starship::extract_binary(&fetch(&url)?)?;
        let target = starship::managed_binary(&app)?;
        write_atomically(&target, &binary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}

/// Writes the preset's config file now, so a failure shows up in
/// Preferences instead of as a session that silently starts without it.
#[tauri::command]
pub async fn install_starship_preset(app: tauri::AppHandle, preset: String) -> Result<(), String> {
    if !starship::is_valid_preset_name(&preset) {
        return Err(crate::i18n::tr(crate::i18n::Msg::InvalidThemeName(&preset)));
    }
    blocking(move || {
        let (binary, _) = starship::find(&app)
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::StarshipNotInstalled)))?;
        starship::ensure_preset(&app, &binary, &preset)?;
        Ok(())
    })
    .await
}
