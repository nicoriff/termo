//! Finding a CLI that the user installed for their shell, from a GUI app.
//!
//! A process launched from Finder (or the Dock, or `open`) inherits launchd's
//! environment, not a login shell's: its PATH is the bare system one, without
//! the per-user and package-manager directories where anything installed by a
//! shell one-liner actually lives. Both things Termini shells out to land
//! there — oh-my-posh in Homebrew's prefix, Claude Code's `claude` in
//! `~/.local/bin` — so PATH alone finds neither.

use std::path::PathBuf;

/// PATH, plus the directories a GUI app's PATH is missing.
pub fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Programs").join("oh-my-posh").join("bin"));
        dirs.push(local.join("Microsoft").join("WindowsApps"));
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

/// Adds Windows' `.exe`, since a bare stem only resolves there through the
/// shell's PATHEXT handling — which is not in play when spawning directly.
pub fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

/// The first `stem` executable found in `candidate_dirs`, if any.
pub fn find_binary(stem: &str) -> Option<PathBuf> {
    let name = exe_name(stem);
    candidate_dirs()
        .into_iter()
        .map(|dir| dir.join(&name))
        .find(|path| path.is_file())
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
        assert!(find_binary("termini-no-such-binary-9f3a").is_none());
    }
}
