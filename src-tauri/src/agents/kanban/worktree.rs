//! A board in worktree mode: each task works on a branch of its own,
//! checked out in a folder of its own, so several can run at once without
//! stepping on each other. Done, the user merges the branch into the
//! board's base branch from the card.
//!
//! These are actions the user set up by choosing the mode: git runs the way
//! a console would (see git/mod.rs), hooks and all.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::git::{self, git_command};
use crate::i18n::{tr, Msg};

use super::model::valid_branch;

/// The branch a task works on.
pub fn branch_for(number: u32, id: &str) -> String {
    let short: String = id.chars().filter(char::is_ascii_alphanumeric).take(6).collect();
    format!("termo/kanban/{number}-{short}")
}

/// Where a task's worktree goes: beside the board's file, out of the repo.
pub fn folder_for(board_dir: &Path, task_id: &str) -> PathBuf {
    board_dir.join("worktrees").join(task_id)
}

pub fn is_repo(dir: &Path) -> bool {
    git::repo_root(dir).is_some()
}

pub fn current_branch(repo: &Path) -> Option<String> {
    git::read_line(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|branch| branch != "HEAD")
}

fn run(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_command(repo)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if said.is_empty() { format!("git {}", args.join(" ")) } else { said })
    }
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok()
}

/// Checks out `branch` in `folder`: a new branch from `base` (the current
/// one when empty), or the branch as it was when it's there already — a
/// task that comes back keeps its work.
pub fn create(repo: &Path, folder: &Path, branch: &str, base: &str) -> Result<(), String> {
    if folder.join(".git").exists() {
        return Ok(());
    }
    // Saved by an older build, or by hand: never as an option to git.
    if !valid_branch(branch) || !valid_branch(base) {
        return Err(tr(Msg::KanbanBadBranch));
    }
    if let Some(parent) = folder.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    // A folder left by a worktree git forgot about.
    let _ = run(repo, &["worktree", "prune"]);
    let folder_text = folder.to_string_lossy();
    if branch_exists(repo, branch) {
        run(repo, &["worktree", "add", &folder_text, branch]).map(|_| ())
    } else {
        let base = if base.trim().is_empty() { "HEAD" } else { base.trim() };
        run(repo, &["worktree", "add", "-b", branch, &folder_text, base]).map(|_| ())
    }
}

/// Takes the worktree away, and the branch with it when `drop_branch`.
pub fn remove(repo: &Path, folder: &Path, branch: &str, drop_branch: bool) {
    if folder.exists() {
        let _ = run(repo, &["worktree", "remove", "--force", &folder.to_string_lossy()]);
        let _ = std::fs::remove_dir_all(folder);
    }
    let _ = run(repo, &["worktree", "prune"]);
    if drop_branch && !branch.is_empty() {
        let _ = run(repo, &["branch", "-D", branch]);
    }
}

/// Merges a task's branch into `base` in the board's own folder, which has
/// to be on that branch. A merge that conflicts is undone: the folder is
/// left as it was and the error says why.
pub fn merge(repo: &Path, branch: &str, base: &str, message: &str) -> Result<(), String> {
    if !valid_branch(branch) || !valid_branch(base) {
        return Err(tr(Msg::KanbanBadBranch));
    }
    let current = current_branch(repo).unwrap_or_default();
    let base = if base.trim().is_empty() { current.as_str() } else { base.trim() };
    if current != base {
        return Err(format!("{} → {base}", if current.is_empty() { "HEAD" } else { current.as_str() }));
    }
    match run(repo, &["merge", "--no-ff", "-m", message, branch]) {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = run(repo, &["merge", "--abort"]);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_here() -> bool {
        crate::pty::bin_search::find_binary("git").is_some()
    }

    fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-kanban-wt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for args in [
            // Without `init -b`: older gits don't have it.
            vec!["init", "-q"],
            vec!["symbolic-ref", "HEAD", "refs/heads/main"],
            vec!["config", "user.email", "t@t"],
            vec!["config", "user.name", "t"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            run(&dir, &args).unwrap();
        }
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        run(&dir, &["add", "."]).unwrap();
        run(&dir, &["commit", "-q", "-m", "one"]).unwrap();
        dir
    }

    #[test]
    fn branch_names_are_safe() {
        assert_eq!(branch_for(3, "ab-cd/ef12"), "termo/kanban/3-abcdef");
        assert!(valid_branch(&branch_for(3, "x")));
        assert!(valid_branch("main") && valid_branch(""));
        assert!(!valid_branch("--upload-pack=x") && !valid_branch("a b"));
    }

    #[test]
    fn a_base_like_an_option_never_reaches_git() {
        let dir = std::env::temp_dir().join(format!("termo-kanban-wt-opt-{}", std::process::id()));
        assert!(create(&dir, &dir.join("t"), "termo/kanban/1-x", "--orphan").is_err());
        assert!(merge(&dir, "termo/kanban/1-x", "-x", "m").is_err());
        assert!(!dir.join("t").exists());
    }

    #[test]
    fn a_task_works_on_its_branch_and_merges_back() {
        if !git_here() {
            return;
        }
        let repo = temp_repo("merge");
        let folder = repo.with_extension("wt").join("t1");
        let branch = branch_for(1, "t1");
        create(&repo, &folder, &branch, "").unwrap();
        assert!(folder.join("a.txt").exists());
        // Again: already there.
        create(&repo, &folder, &branch, "").unwrap();
        std::fs::write(folder.join("b.txt"), "b\n").unwrap();
        run(&folder, &["add", "."]).unwrap();
        run(&folder, &["commit", "-q", "-m", "two"]).unwrap();
        assert!(merge(&repo, &branch, "other", "x").is_err());
        merge(&repo, &branch, "main", "Merge task 1").unwrap();
        assert!(repo.join("b.txt").exists());
        remove(&repo, &folder, &branch, true);
        assert!(!folder.exists());
        assert!(!branch_exists(&repo, &branch));
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(repo.with_extension("wt"));
    }
}
