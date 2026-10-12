//! Git for the side panel, the status bar and the diff tabs: what the
//! repository a tab is in looks like, and the actions the user runs on it.
//!
//! Two kinds of call, kept apart on purpose:
//!
//! - **Reads** (`read`) happen on their own, just from a folder being open —
//!   one that came in a zip too, whose `.git/config` nobody here wrote. They
//!   never start a program that config names: `core.fsmonitor` is turned off,
//!   nothing is asked that runs a filter, a diff driver or gpg, and the index
//!   isn't rewritten (so no hook fires on it either).
//! - **Actions** (`ops`) are what the user asked for by name, and run git the
//!   way a console would: hooks and all.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

pub mod forge;
pub mod graph;
pub mod ops;
pub mod pr;
pub mod review;
pub mod snapshot;

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

/// The repository `path` is inside: the folder that holds `.git` (a folder in
/// a normal clone, a file in a worktree), found by walking up.
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|dir| dir.join(".git").exists()).map(Path::to_path_buf)
}

/// No console window flashing up on every call (Windows).
pub fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

pub fn git_command(root: &Path) -> Command {
    // A GUI app's PATH often lacks where git lives (see pty/bin_search.rs).
    let git = crate::pty::bin_search::find_binary("git").unwrap_or_else(|| PathBuf::from("git"));
    let mut command = Command::new(git);
    command.arg("-C").arg(root);
    no_window(&mut command);
    command
}

/// What every read starts with: no fsmonitor program, and no taking the
/// index's lock to refresh it — which also keeps a poll from getting in the
/// way of a git the user is running in the console.
pub const READ_ARGS: &[&str] = &["--no-optional-locks", "-c", "core.fsmonitor=false"];

/// A read's standard output, or `None` when git isn't there or said no.
pub fn read(root: &Path, args: &[&str]) -> Option<String> {
    let output = git_command(root)
        .args(READ_ARGS)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The same, as bytes: a file's content, which may not be text.
pub fn read_bytes(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = git_command(root)
        .args(READ_ARGS)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// One line of a read, trimmed; `None` when empty.
pub fn read_line(root: &Path, args: &[&str]) -> Option<String> {
    read(root, args).map(|out| out.trim().to_string()).filter(|line| !line.is_empty())
}

/// Whether the repository's own config (`git config --local --list`, its
/// includes followed) defines a filter driver: a program `git status` could
/// start. Reading the config never runs one.
pub fn defines_filters(local_config: &str) -> bool {
    local_config
        .lines()
        .filter_map(|line| line.split_once('=').map_or(Some(line), |(key, _)| Some(key)))
        .any(|key| key.trim().to_ascii_lowercase().starts_with("filter."))
}

/// Whether `git status` is left unasked here: see `defines_filters`.
pub fn brings_filters(root: &Path) -> bool {
    let config = git_command(root).args(["config", "--local", "--includes", "--list"]).stdin(Stdio::null()).output();
    config.is_ok_and(|output| defines_filters(&String::from_utf8_lossy(&output.stdout)))
}

/// Whether a repository's own config (`git config --local --list`) names a
/// program git would start to look at a change: a filter, an external diff,
/// or a diff driver's converter. Reading the config never runs one.
pub fn names_programs(local_config: &str) -> bool {
    local_config.lines().map(|line| line.split_once('=').map_or(line, |(key, _)| key).trim().to_ascii_lowercase()).any(|key| {
        key.starts_with("filter.") || key == "diff.external" || (key.starts_with("diff.") && (key.ends_with(".textconv") || key.ends_with(".command")))
    })
}

/// Whether an agent that may only look is kept out of `folder`: it's in a
/// repository whose config names such a program. An agent like that runs
/// `git diff` and `git status` the way git runs them, programs and all — so
/// in a folder that came from elsewhere, looking would be running its code.
pub fn runs_programs_to_look(folder: &Path) -> bool {
    let Some(root) = repo_root(folder) else { return false };
    let config = git_command(&root).args(["config", "--local", "--includes", "--list"]).stdin(Stdio::null()).output();
    config.is_ok_and(|output| names_programs(&String::from_utf8_lossy(&output.stdout)))
}

/// A commit's id as git prints it: hex, and nothing an option could hide in.
pub fn is_sha(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A name git would take for a branch, and that can't be read as an option
/// or reach outside `refs/`. Stricter than git where it's cheap to be.
pub fn is_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('-')
        && !name.starts_with('/')
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && name != "@"
        && name.chars().all(|c| !c.is_whitespace() && !c.is_control() && !"~^:?*[\\".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_the_config_names_for_looking_at_changes_are_spotted() {
        assert!(names_programs("core.bare=false\nfilter.crypt.clean=git-crypt clean\n"));
        assert!(names_programs("Diff.External=/tmp/x.sh"));
        assert!(names_programs("diff.pdf.textconv=pdftotext"));
        assert!(names_programs("diff.img.command=compare"));
        // A driver that only names how to find a function's header runs nothing.
        assert!(!names_programs("diff.rust.xfuncname=^fn\ndiff.renames=true\ndiff.algorithm=histogram\n"));
        assert!(!names_programs("remote.origin.url=https://x/diff.external.git\n"));
        assert!(!names_programs(""));
    }

    #[test]
    fn a_folder_is_judged_by_the_repository_it_is_in() {
        let Some(git) = crate::pty::bin_search::find_binary("git") else { return };
        let base = std::env::temp_dir().join(format!("termo-git-guard-{}", uuid::Uuid::new_v4()));
        let inside = base.join("src").join("deep");
        std::fs::create_dir_all(&inside).unwrap();
        // Not a repository: nothing of git's to run.
        assert!(!runs_programs_to_look(&inside));
        let run = |args: &[&str]| assert!(std::process::Command::new(&git).arg("-C").arg(&base).args(args).output().unwrap().status.success());
        run(&["init", "-q"]);
        assert!(!runs_programs_to_look(&base) && !runs_programs_to_look(&inside));
        run(&["config", "diff.external", "anything"]);
        // From anywhere inside it, not only its top.
        assert!(runs_programs_to_look(&base) && runs_programs_to_look(&inside));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn filter_drivers_in_the_repositorys_config_are_spotted() {
        assert!(defines_filters("core.bare=false\nfilter.crypt.clean=git-crypt clean\n"));
        assert!(defines_filters("Filter.X.Process=x"));
        assert!(!defines_filters("core.bare=false\nremote.origin.url=https://x/filter.git\n"));
        assert!(!defines_filters(""));
    }

    #[test]
    fn reads_never_refresh_the_index_or_start_fsmonitor() {
        let args = READ_ARGS.join(" ");
        assert!(args.contains("--no-optional-locks"));
        assert!(args.contains("core.fsmonitor=false"));
    }

    #[test]
    fn shas_are_hex_only() {
        assert!(is_sha("4dcc2f6"));
        assert!(is_sha("1e69c77a0b1c2d3e4f5061728394a5b6c7d8e9f0"));
        assert!(!is_sha("--all"));
        assert!(!is_sha("HEAD"));
        assert!(!is_sha("abc"));
    }

    #[test]
    fn branch_names_cant_pass_for_options_or_revisions() {
        for good in ["main", "feature/git-panel", "fix-123", "release/1.2.x", "origin/main"] {
            assert!(is_branch_name(good), "{good}");
        }
        for bad in ["", "-D", "--force", "a b", "a..b", "a~1", "a^", "a:b", "x.lock", "a/", "/a", "a@{1}", "@", "a\\b", "a*"] {
            assert!(!is_branch_name(bad), "{bad}");
        }
    }
}
