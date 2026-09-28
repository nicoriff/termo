//! Running an agent on a git event: a marked block in the repository's
//! `post-commit` or `pre-push` hook that calls `termo --run-agent <id>`.
//!
//! The block lives alongside whatever the hook already does — it's added and
//! removed by its markers, never by rewriting the file — and it can't hold
//! git up: the call only leaves a note for the running app (see queue.rs)
//! and never fails.

use std::fs;
use std::path::{Path, PathBuf};

use crate::i18n::{tr, Msg};

use super::github::git_output;
use super::spec::{AgentSpec, GitEvent};

/// The flag the hook calls Termo with.
pub const RUN_FLAG: &str = "--run-agent";

fn start_marker(id: &str) -> String {
    format!("# >>> termo agent {id}")
}

fn end_marker(id: &str) -> String {
    format!("# <<< termo agent {id}")
}

/// The hook with the agent's block added (or refreshed). `exe` is Termo's
/// own path.
pub fn add_block(existing: Option<&str>, id: &str, event: GitEvent, exe: &str) -> String {
    let base = existing.map(|text| remove_block(text, id)).unwrap_or_default();
    let mut out = if base.trim().is_empty() { "#!/bin/sh\n".to_string() } else { base };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    // git runs hooks through sh, Git for Windows included: forward slashes,
    // single quotes (so no `$` or backquote in the path is ever expanded),
    // and `|| true` so the hook's own exit code never changes.
    let exe = format!("'{}'", exe.replace('\\', "/").replace('\'', "'\\''"));
    let event = match event {
        GitEvent::Commit => "commit",
        GitEvent::Push => "push",
    };
    out.push_str(&format!(
        "{}\n{exe} {RUN_FLAG} {id} {event} >/dev/null 2>&1 || true\n{}\n",
        start_marker(id),
        end_marker(id)
    ));
    out
}

/// The hook without the agent's block.
pub fn remove_block(existing: &str, id: &str) -> String {
    let (start, end) = (start_marker(id), end_marker(id));
    let mut out = Vec::new();
    let mut inside = false;
    for line in existing.lines() {
        if line.trim() == start {
            inside = true;
        } else if line.trim() == end {
            inside = false;
        } else if !inside {
            out.push(line);
        }
    }
    let mut text = out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// Whether what's left of a hook does nothing (only a shebang).
fn is_empty_hook(text: &str) -> bool {
    text.lines().all(|line| line.trim().is_empty() || line.starts_with("#!"))
}

/// Where the repository's hooks live, respecting `core.hooksPath` and
/// worktrees.
fn hooks_dir(repo: &Path) -> Option<PathBuf> {
    let dir = git_output(repo, &["rev-parse", "--git-path", "hooks"])?;
    let dir = PathBuf::from(dir);
    Some(if dir.is_absolute() { dir } else { repo.join(dir) })
}

fn write_hook(path: &Path, text: &str) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().expect("a hook has a folder"))?;
    fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn remove_from(repo: &Path, id: &str) {
    let Some(dir) = hooks_dir(repo) else { return };
    for event in [GitEvent::Commit, GitEvent::Push] {
        let path = dir.join(event.hook_name());
        let Ok(text) = fs::read_to_string(&path) else { continue };
        if !text.contains(&start_marker(id)) {
            continue;
        }
        let left = remove_block(&text, id);
        let _ = if is_empty_hook(&left) { fs::remove_file(&path) } else { write_hook(&path, &left) };
    }
}

/// Brings the hooks in line with an agent being saved (`new`) or deleted
/// (`new` = None): its block leaves wherever it was and goes where it's
/// wanted now.
pub fn sync(old: Option<&AgentSpec>, new: Option<&AgentSpec>) -> Result<(), String> {
    let id = new.or(old).map(|spec| spec.id.clone()).unwrap_or_default();
    if let Some(trigger) = old.and_then(|spec| spec.triggers.git.as_ref()) {
        remove_from(Path::new(trigger.repo_path.trim()), &id);
    }
    let Some(spec) = new.filter(|spec| spec.enabled) else { return Ok(()) };
    let Some(trigger) = spec.triggers.git.as_ref() else { return Ok(()) };
    let repo = Path::new(trigger.repo_path.trim());
    // A block left in the other hook (the event changed) goes too.
    remove_from(repo, &id);
    let dir = hooks_dir(repo).ok_or_else(|| tr(Msg::AgentGitNotRepo(trigger.repo_path.trim())))?;
    let path = dir.join(trigger.event.hook_name());
    let exe = std::env::current_exe().map_err(|err| tr(Msg::AgentHookFailed(&err.to_string())))?;
    let existing = fs::read_to_string(&path).ok();
    let text = add_block(existing.as_deref(), &spec.id, trigger.event, &exe.to_string_lossy());
    write_hook(&path, &text).map_err(|err| tr(Msg::AgentHookFailed(&err.to_string())))
}

/// The commit a hook fired for, as context for the run.
pub fn describe_head(repo: &Path, event: &str) -> String {
    let head = git_output(repo, &["log", "-1", "--format=%h %s%n%an, %ar"]).unwrap_or_default();
    let branch = git_output(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    match event {
        "push" => format!("git push from branch {branch}; latest commit: {head}"),
        _ => format!("New commit on branch {branch}: {head}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "C:\\Program Files\\Termo\\termo.exe";

    #[test]
    fn a_new_hook_gets_a_shebang_and_the_block() {
        let text = add_block(None, "a1", GitEvent::Commit, EXE);
        assert!(text.starts_with("#!/bin/sh\n"));
        assert!(text.contains("'C:/Program Files/Termo/termo.exe' --run-agent a1 commit"));
        assert!(text.contains("|| true"));
    }

    #[test]
    fn nothing_in_the_path_is_expanded_by_the_shell() {
        let text = add_block(None, "a1", GitEvent::Commit, "C:\\Users\\$(rm -rf)`x`\\it's\\termo.exe");
        assert!(text.contains("'C:/Users/$(rm -rf)`x`/it'\\''s/termo.exe' --run-agent"));
    }

    #[test]
    fn an_existing_hook_keeps_its_own_lines() {
        let mine = "#!/bin/sh\nnpm test\n";
        let text = add_block(Some(mine), "a1", GitEvent::Push, EXE);
        assert!(text.starts_with("#!/bin/sh\nnpm test\n# >>> termo agent a1\n"));
        assert_eq!(remove_block(&text, "a1"), mine);
    }

    #[test]
    fn adding_twice_leaves_one_block() {
        let once = add_block(None, "a1", GitEvent::Commit, EXE);
        let twice = add_block(Some(&once), "a1", GitEvent::Commit, EXE);
        assert_eq!(twice.matches("# >>> termo agent a1").count(), 1);
    }

    #[test]
    fn other_agents_blocks_are_left_alone() {
        let both = add_block(Some(&add_block(None, "a1", GitEvent::Commit, EXE)), "b2", GitEvent::Commit, EXE);
        let left = remove_block(&both, "a1");
        assert!(!left.contains("a1") && left.contains("--run-agent b2"));
    }

    /// Against a real repository, when git is around: the hook goes in where
    /// git looks for it, keeps the user's own, and leaves no trace when
    /// removed.
    #[test]
    fn sync_installs_and_removes_in_a_real_repository() {
        use crate::agents::spec::GitTrigger;
        let Some(git) = crate::pty::bin_search::find_binary("git") else { return };
        let repo = std::env::temp_dir().join(format!("termo-hooks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        let ok = std::process::Command::new(git).arg("init").arg("-q").arg(&repo).status().is_ok_and(|s| s.success());
        if !ok {
            return;
        }
        let hooks = hooks_dir(&repo).unwrap();
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("post-commit"), "#!/bin/sh\necho mine\n").unwrap();

        let mut spec = AgentSpec {
            id: "a1".into(),
            ..AgentSpec::default()
        };
        spec.triggers.git = Some(GitTrigger {
            repo_path: repo.to_string_lossy().into_owned(),
            event: GitEvent::Commit,
        });
        sync(None, Some(&spec)).unwrap();
        let text = fs::read_to_string(hooks.join("post-commit")).unwrap();
        assert!(text.contains("echo mine") && text.contains("--run-agent a1 commit"));

        // Moving to push leaves the commit hook as the user had it.
        let mut moved = spec.clone();
        moved.triggers.git.as_mut().unwrap().event = GitEvent::Push;
        sync(Some(&spec), Some(&moved)).unwrap();
        assert_eq!(fs::read_to_string(hooks.join("post-commit")).unwrap(), "#!/bin/sh\necho mine\n");
        assert!(fs::read_to_string(hooks.join("pre-push")).unwrap().contains("--run-agent a1 push"));

        // Deleting the agent removes the hook file it created.
        sync(Some(&moved), None).unwrap();
        assert!(!hooks.join("pre-push").exists());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_hook_with_only_our_block_counts_as_empty_once_its_gone() {
        let text = add_block(None, "a1", GitEvent::Commit, EXE);
        assert!(is_empty_hook(&remove_block(&text, "a1")));
        assert!(!is_empty_hook("#!/bin/sh\nnpm test\n"));
    }
}
