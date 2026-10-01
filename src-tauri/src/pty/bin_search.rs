//! Finding a CLI that the user installed for their shell, from a GUI app.
//!
//! A process launched from Finder (or the Dock, or `open`) inherits launchd's
//! environment, not a login shell's: its PATH is the bare system one, without
//! the per-user and package-manager directories where anything installed by a
//! shell one-liner actually lives. Both things Termo shells out to land
//! there — oh-my-posh in Homebrew's prefix, Claude Code's `claude` in
//! `~/.local/bin` — so PATH alone finds neither.

use std::path::PathBuf;

/// PATH, plus the directories a GUI app's PATH is missing.
pub fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    // A PATH is fixed when the process starts: a CLI installed since (or
    // while Explorer still had the old PATH) is only found where installers
    // put it.
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(local.join("Programs").join("oh-my-posh").join("bin"));
            dirs.push(local.join("Microsoft").join("WindowsApps"));
            dirs.push(local.join("Microsoft").join("WinGet").join("Links"));
        }
        // The GitHub CLI's installer (winget's too).
        if let Some(programs) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
            dirs.push(programs.join("GitHub CLI"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            dirs.push(profile.join("scoop").join("shims"));
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            dirs.push(home.join(".local").join("bin"));
            dirs.push(home.join("bin"));
        }
        for dir in [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/opt/local/bin",
            "/home/linuxbrew/.linuxbrew/bin",
        ] {
            dirs.push(PathBuf::from(dir));
        }
    }
    dirs
}

/// The extensions worth trying for a bare stem on Windows, in PATHEXT's own
/// order. A shell resolves `codex` to `codex.cmd` via PATHEXT automatically;
/// spawning a process directly skips that, and plenty of npm-installed CLIs
/// (codex among them) ship only a `.cmd` shim, never a `.exe`.
#[cfg(windows)]
const WINDOWS_EXTENSIONS: &[&str] = &["exe", "cmd", "bat"];

/// The names worth trying for `stem` in a single directory, most likely
/// first.
fn candidate_names(stem: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        WINDOWS_EXTENSIONS.iter().map(|ext| format!("{stem}.{ext}")).collect()
    }
    #[cfg(not(windows))]
    {
        vec![stem.to_string()]
    }
}

/// The first `stem` executable found in `candidate_dirs`, if any — trying
/// every extension in a directory before moving to the next, so PATH order
/// still wins over extension order.
pub fn find_binary(stem: &str) -> Option<PathBuf> {
    let names = candidate_names(stem);
    candidate_dirs()
        .into_iter()
        .find_map(|dir| names.iter().map(|name| dir.join(name)).find(|path| path.is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_beyond_path() {
        // Whatever PATH happens to hold, the extra per-user and package
        // manager dirs are always considered — that's the whole point.
        let dirs = candidate_dirs();
        #[cfg(not(windows))]
        assert!(dirs.iter().any(|dir| dir.ends_with("bin")), "dirs: {dirs:?}");
        assert!(!dirs.is_empty());
    }

    #[test]
    fn finds_nothing_for_a_name_that_cannot_exist() {
        assert!(find_binary("termo-no-such-binary-9f3a").is_none());
    }

    /// npm's Windows installer leaves a `.cmd` shim on PATH, never a
    /// `.exe` — codex is exactly this shape, and this regression is why
    /// `find_binary` used to report it as not installed.
    #[test]
    #[cfg(windows)]
    fn finds_a_cmd_shim_on_windows() {
        let dir = std::env::temp_dir().join("termo-bin-search-test-cmd-shim");
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("termo-fake-cli.cmd");
        std::fs::write(&shim, "@echo off\r\n").unwrap();

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths: Vec<PathBuf> = std::env::split_paths(&original_path).collect();
        paths.insert(0, dir.clone());
        std::env::set_var("PATH", std::env::join_paths(&paths).unwrap());

        let found = find_binary("termo-fake-cli");

        std::env::set_var("PATH", &original_path);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(found, Some(shim));
    }
}
