//! What the file explorer and the file tabs need from disk: where the
//! repository a console is in starts, what a folder holds, which files git
//! sees as changed, and reading and saving text files.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::i18n::{tr, Msg};

/// Past this, a file isn't opened: an editor tab isn't the place for it, and
/// the whole thing travels to the webview as one string.
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// How much of a file is checked for a NUL byte to call it binary.
const BINARY_SNIFF_BYTES: usize = 8192;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerRoot {
    pub path: String,
    /// Whether it's a git repository's root, or just the folder asked about.
    pub is_repo: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum GitMark {
    Modified,
    Added,
    Untracked,
    Deleted,
    Renamed,
    Conflict,
}

/// The repository `path` is inside, found by walking up to the folder that
/// holds `.git` (a folder in a normal clone, a file in a worktree). Outside
/// any repository, the folder itself.
#[tauri::command]
pub fn explorer_root(path: String) -> ExplorerRoot {
    let start = PathBuf::from(&path);
    let repo = start.ancestors().find(|dir| dir.join(".git").exists());
    match repo {
        Some(root) => ExplorerRoot { path: root.to_string_lossy().into_owned(), is_repo: true },
        None => ExplorerRoot { path, is_repo: false },
    }
}

#[tauri::command]
pub async fn list_entries(path: String) -> Result<Vec<ExplorerEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || entries(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())?
}

/// Folders first, then files, each by name. `.git` itself is left out: it's
/// git's own bookkeeping, not part of the project.
fn entries(dir: &Path) -> Result<Vec<ExplorerEntry>, String> {
    let read = fs::read_dir(dir)
        .map_err(|err| tr(Msg::FolderUnreadable(&dir.to_string_lossy(), &err.to_string())))?;
    let mut list: Vec<ExplorerEntry> = read
        .flatten()
        .filter(|item| item.file_name() != ".git")
        .map(|item| ExplorerEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            path: item.path().to_string_lossy().into_owned(),
            is_dir: fs::metadata(item.path()).is_ok_and(|meta| meta.is_dir()),
        })
        .collect();
    list.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(list)
}

/// Every changed file git reports under `root`, by absolute path. Empty when
/// it isn't a repository or git can't be run — the explorer then just shows
/// no marks.
#[tauri::command]
pub async fn git_status(root: String) -> HashMap<String, GitMark> {
    tauri::async_runtime::spawn_blocking(move || changed_files(Path::new(&root)))
        .await
        .unwrap_or_default()
}

fn changed_files(root: &Path) -> HashMap<String, GitMark> {
    // A GUI app's PATH often lacks where git lives (see pty/bin_search.rs).
    let git = crate::pty::bin_search::find_binary("git").unwrap_or_else(|| PathBuf::from("git"));
    let mut command = Command::new(git);
    command.arg("-C").arg(root).args(["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up on every refresh.
        command.creation_flags(0x0800_0000);
    }
    let Ok(output) = command.output() else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }
    parse_porcelain(&String::from_utf8_lossy(&output.stdout))
        .into_iter()
        .map(|(relative, mark)| (root.join(relative).to_string_lossy().into_owned(), mark))
        .collect()
}

/// `git status --porcelain=v1 -z`: `XY path` entries split by NUL, where a
/// rename or copy carries its old path as one more entry right after.
fn parse_porcelain(output: &str) -> Vec<(PathBuf, GitMark)> {
    let mut marks = Vec::new();
    let mut parts = output.split('\0').filter(|part| part.len() > 3);
    while let Some(entry) = parts.next() {
        let (code, path) = entry.split_at(3);
        let (x, y) = (code.as_bytes()[0], code.as_bytes()[1]);
        let mark = if x == b'?' {
            GitMark::Untracked
        } else if x == b'U' || y == b'U' || (x == b'A' && y == b'A') || (x == b'D' && y == b'D') {
            GitMark::Conflict
        } else if x == b'R' || x == b'C' {
            // The old path follows; it's gone, nothing to mark.
            parts.next();
            GitMark::Renamed
        } else if x == b'D' || y == b'D' {
            GitMark::Deleted
        } else if x == b'A' {
            GitMark::Added
        } else {
            GitMark::Modified
        };
        marks.push((PathBuf::from(path.replace('/', std::path::MAIN_SEPARATOR_STR)), mark));
    }
    marks
}

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || read_text(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())?
}

fn read_text(path: &Path) -> Result<String, String> {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let fail = |err: std::io::Error| tr(Msg::FileReadFailed(&name, &err.to_string()));
    let size = fs::metadata(path).map_err(fail)?.len();
    if size > MAX_FILE_BYTES {
        return Err(tr(Msg::FileTooLarge(&name, MAX_FILE_BYTES / (1024 * 1024))));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    fs::File::open(path).and_then(|mut file| file.read_to_end(&mut bytes)).map_err(fail)?;
    if bytes.iter().take(BINARY_SNIFF_BYTES).any(|byte| *byte == 0) {
        return Err(tr(Msg::FileNotText(&name)));
    }
    // A UTF-8 byte-order mark stays in the text (as U+FEFF), so saving
    // writes it back unchanged.
    String::from_utf8(bytes).map_err(|_| tr(Msg::FileNotText(&name)))
}

#[tauri::command]
pub async fn write_text_file(path: String, content: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_text(Path::new(&path), &content))
        .await
        .map_err(|err| err.to_string())?
}

/// Written next to the file and renamed over it, so a failed save never
/// leaves it half-written.
fn write_text(path: &Path, content: &str) -> Result<(), String> {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let fail = |err: std::io::Error| tr(Msg::FileWriteFailed(&name, &err.to_string()));
    let staging = path.with_file_name(format!(".{name}.termo-save"));
    fs::write(&staging, content).map_err(fail)?;
    fs::rename(&staging, path).map_err(|err| {
        let _ = fs::remove_file(&staging);
        fail(err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-explorer-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_the_repository_root_from_a_nested_folder() {
        let base = scratch();
        fs::create_dir_all(base.join(".git")).unwrap();
        fs::create_dir_all(base.join("src").join("lib")).unwrap();
        let root = explorer_root(base.join("src").join("lib").to_string_lossy().into_owned());
        let outside = explorer_root(std::env::temp_dir().to_string_lossy().into_owned());
        let _ = fs::remove_dir_all(&base);
        assert!(root.is_repo);
        assert_eq!(PathBuf::from(root.path), base);
        assert!(!outside.is_repo || PathBuf::from(&outside.path) != base);
    }

    #[test]
    fn lists_folders_first_and_leaves_git_out() {
        let base = scratch();
        fs::create_dir_all(base.join(".git")).unwrap();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("b.txt"), "b").unwrap();
        fs::write(base.join("A.md"), "a").unwrap();
        fs::write(base.join(".gitignore"), "x").unwrap();
        let names: Vec<String> = entries(&base).unwrap().into_iter().map(|entry| entry.name).collect();
        let _ = fs::remove_dir_all(&base);
        assert_eq!(names, ["src", ".gitignore", "A.md", "b.txt"]);
    }

    #[test]
    fn parses_every_kind_of_change() {
        let output = " M src/a.rs\0A  new.txt\0?? notes.md\0 D gone.txt\0R  to.txt\0from.txt\0UU both.txt\0";
        let marks: Vec<(String, GitMark)> = parse_porcelain(output)
            .into_iter()
            .map(|(path, mark)| (path.to_string_lossy().replace('\\', "/"), mark))
            .collect();
        assert_eq!(
            marks,
            [
                ("src/a.rs".to_string(), GitMark::Modified),
                ("new.txt".to_string(), GitMark::Added),
                ("notes.md".to_string(), GitMark::Untracked),
                ("gone.txt".to_string(), GitMark::Deleted),
                ("to.txt".to_string(), GitMark::Renamed),
                ("both.txt".to_string(), GitMark::Conflict),
            ]
        );
    }

    #[test]
    fn reads_text_and_refuses_binaries() {
        let base = scratch();
        fs::write(base.join("a.txt"), "hola\r\nmundo").unwrap();
        fs::write(base.join("b.bin"), [0u8, 1, 2, 3]).unwrap();
        let text = read_text(&base.join("a.txt"));
        let binary = read_text(&base.join("b.bin"));
        let _ = fs::remove_dir_all(&base);
        assert_eq!(text.unwrap(), "hola\r\nmundo");
        assert!(binary.is_err());
    }

    #[test]
    fn saves_over_the_file_without_leaving_the_staging_copy() {
        let base = scratch();
        let file = base.join("a.txt");
        fs::write(&file, "old").unwrap();
        write_text(&file, "new").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        let leftovers = fs::read_dir(&base).unwrap().count();
        let _ = fs::remove_dir_all(&base);
        assert_eq!(content, "new");
        assert_eq!(leftovers, 1);
    }
}
