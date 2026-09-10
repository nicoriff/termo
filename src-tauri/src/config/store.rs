use std::fs;
use std::path::PathBuf;

use tauri::Manager;

use super::schema::AppConfig;

fn config_path(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_config_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
}

pub fn load(app: &tauri::AppHandle) -> AppConfig {
    let Ok(path) = config_path(app) else {
        return AppConfig::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn save(app: &tauri::AppHandle, config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path(app)?;
    let tmp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(config)?;
    fs::write(&tmp_path, contents)?;
    fs::rename(&tmp_path, &path)?;
    Ok(())
}
