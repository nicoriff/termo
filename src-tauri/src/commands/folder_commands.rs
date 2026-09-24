//! What the folder browser in the agent-launch dialog walks: the places to
//! start from, and the folders inside any one of them. Folders only — the
//! browser picks where an agent runs, never a file.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRoots {
    /// Drives on Windows; `/` elsewhere.
    pub drives: Vec<FolderEntry>,
    pub home: Option<FolderEntry>,
}

#[tauri::command]
pub async fn list_folder_roots() -> FolderRoots {
    // Probing a drive letter can wait on the hardware (an empty card reader,
    // a sleeping network drive), so it stays off the IPC thread.
    tauri::async_runtime::spawn_blocking(folder_roots)
        .await
        .unwrap_or(FolderRoots { drives: Vec::new(), home: None })
}

#[tauri::command]
pub async fn list_folders(path: String) -> Result<Vec<FolderEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || subfolders(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())?
}

fn folder_roots() -> FolderRoots {
    FolderRoots {
        drives: drives(),
        home: dirs::home_dir().map(|home| entry(display_name(&home), home)),
    }
}

#[cfg(windows)]
fn drives() -> Vec<FolderEntry> {
    // A: and B: are floppy letters: probing one that's mapped to nothing can
    // stall, and nobody runs an agent there.
    ('C'..='Z')
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|root| root.exists())
        .map(|root| entry(root.to_string_lossy().trim_end_matches('\\').to_string(), root))
        .collect()
}

#[cfg(not(windows))]
fn drives() -> Vec<FolderEntry> {
    vec![entry("/".to_string(), PathBuf::from("/"))]
}

fn subfolders(dir: &Path) -> Result<Vec<FolderEntry>, String> {
    let entries = fs::read_dir(dir).map_err(|err| {
        crate::i18n::tr(crate::i18n::Msg::FolderUnreadable(&dir.to_string_lossy(), &err.to_string()))
    })?;
    let mut folders: Vec<FolderEntry> = entries
        .flatten()
        .filter(|item| !is_hidden(item))
        // Through links too: a junction like OneDrive's is a folder to the user.
        .filter(|item| fs::metadata(item.path()).is_ok_and(|meta| meta.is_dir()))
        .map(|item| entry(item.file_name().to_string_lossy().into_owned(), item.path()))
        .collect();
    folders.sort_by_key(|folder| folder.name.to_lowercase());
    Ok(folders)
}

/// Dot-folders everywhere — on Windows too, where tools like cargo, npm and
/// the agents themselves leave `.cache`, `.claude`… in the home folder
/// without marking them hidden — plus, on Windows, anything flagged hidden or
/// system ($Recycle.Bin, System Volume Information, AppData…).
fn is_hidden(item: &fs::DirEntry) -> bool {
    item.file_name().to_string_lossy().starts_with('.') || has_hidden_attribute(item)
}

#[cfg(windows)]
fn has_hidden_attribute(item: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    item.metadata().is_ok_and(|meta| meta.file_attributes() & (HIDDEN | SYSTEM) != 0)
}

#[cfg(not(windows))]
fn has_hidden_attribute(_item: &fs::DirEntry) -> bool {
    false
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn entry(name: String, path: PathBuf) -> FolderEntry {
    FolderEntry { name, path: path.to_string_lossy().into_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_visible_folders_sorted_by_name() {
        let base = std::env::temp_dir().join(format!("termini-folders-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(base.join("beta")).unwrap();
        fs::create_dir_all(base.join("Alpha")).unwrap();
        fs::write(base.join("file.txt"), "not a folder").unwrap();
        fs::create_dir_all(base.join(".cache")).unwrap();

        let names: Vec<String> = subfolders(&base).unwrap().into_iter().map(|folder| folder.name).collect();
        let _ = fs::remove_dir_all(&base);
        assert_eq!(names, ["Alpha", "beta"]);
    }

    #[test]
    fn a_missing_folder_is_an_error_not_an_empty_list() {
        assert!(subfolders(Path::new("/definitely/not/here/termini")).is_err());
    }

    #[test]
    fn there_is_always_somewhere_to_start() {
        assert!(!folder_roots().drives.is_empty());
    }
}
