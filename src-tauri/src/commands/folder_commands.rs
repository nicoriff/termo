//! What the folder browser in the agent-launch dialog walks: the places to
//! start from, and the folders inside any one of them. Folders only — the
//! browser picks where an agent runs, never a file. The web client's
//! browser walks the same, through the sessions daemon (see mux/server.rs).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Creates `name` inside `parent` — the browser's "new folder" — and returns
/// it. Never reuses an existing folder: a name that's taken is an error, so a
/// typo can't quietly land the agent in some other project.
#[tauri::command]
pub async fn create_folder(parent: String, name: String) -> Result<FolderEntry, String> {
    tauri::async_runtime::spawn_blocking(move || new_folder(Path::new(&parent), &name))
        .await
        .map_err(|err| err.to_string())?
}

pub(crate) fn new_folder(parent: &Path, name: &str) -> Result<FolderEntry, String> {
    use crate::i18n::{tr, Msg};

    let name = name.trim();
    if !is_valid_folder_name(name) {
        return Err(tr(Msg::FolderNameInvalid(name)));
    }
    let path = parent.join(name);
    match fs::create_dir(&path) {
        Ok(()) => Ok(entry(name.to_string(), path)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Err(tr(Msg::FolderExists(name))),
        Err(err) => Err(tr(Msg::FolderCreateFailed(name, &err.to_string()))),
    }
}

/// A single path component every platform accepts: no separators or
/// characters Windows forbids, not `.`/`..`, not a reserved device name
/// (`CON`, `NUL`, `COM1`…), and no trailing dot or space, which Windows
/// silently strips.
pub(crate) fn is_valid_folder_name(name: &str) -> bool {
    const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    const RESERVED: &[&str] = &["CON", "PRN", "AUX", "NUL"];
    if name.is_empty() || name == "." || name == ".." || name.len() > 255 {
        return false;
    }
    if name.chars().any(|c| FORBIDDEN.contains(&c) || c.is_control()) || name.ends_with(['.', ' ']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let numbered = (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.len() == 4
        && stem.as_bytes()[3].is_ascii_digit();
    !(RESERVED.contains(&stem.as_str()) || numbered)
}

pub(crate) fn folder_roots() -> FolderRoots {
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

pub(crate) fn subfolders(dir: &Path) -> Result<Vec<FolderEntry>, String> {
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
        let base = std::env::temp_dir().join(format!("termo-folders-{}", uuid::Uuid::new_v4()));
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
        assert!(subfolders(Path::new("/definitely/not/here/termo")).is_err());
    }

    #[test]
    fn creates_a_folder_and_refuses_to_reuse_one() {
        let base = std::env::temp_dir().join(format!("termo-new-folder-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let created = new_folder(&base, "  my-project  ");
        let again = new_folder(&base, "my-project");
        let exists = base.join("my-project").is_dir();
        let _ = fs::remove_dir_all(&base);
        assert_eq!(created.unwrap().name, "my-project");
        assert!(exists);
        assert!(again.is_err());
    }

    #[test]
    fn rejects_names_that_are_not_a_plain_folder_name() {
        for name in ["", ".", "..", "a/b", "a\\b", "what?", "C:", "trailing.", "trailing ", "CON", "nul.txt", "com1"] {
            assert!(!is_valid_folder_name(name), "{name:?} should be rejected");
        }
        for name in ["project", "my project", "v1.2", "Console", "com10", ".config"] {
            assert!(is_valid_folder_name(name), "{name:?} should be accepted");
        }
    }

    #[test]
    fn there_is_always_somewhere_to_start() {
        assert!(!folder_roots().drives.is_empty());
    }
}
