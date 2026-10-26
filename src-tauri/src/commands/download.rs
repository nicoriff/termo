//! Downloads for the prompt integrations (Oh My Posh, Starship, Nerd Font).

use std::fs;
use std::io::Read;
use std::path::Path;

/// Downloads are several MB; running them off the async runtime's blocking
/// pool keeps the webview's IPC responsive meanwhile.
pub async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

pub fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ureq::get(url).call()?.into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn download_to(url: &str, target: &Path) -> anyhow::Result<()> {
    write_atomically(target, &fetch(url)?)
}

/// Writes to a sibling temp file first so an interrupted download never
/// leaves a truncated binary where `find` would pick it up.
pub fn write_atomically(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = target.with_extension("download");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, target)?;
    Ok(())
}
