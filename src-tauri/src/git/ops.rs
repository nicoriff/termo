//! The actions the user runs on a repository from the panel or the status
//! bar: git itself, in the background, one at a time per repository, with
//! what it prints reported as it comes (`git-op` events).
//!
//! Nothing here can ask a question: there's no terminal behind it. A prompt
//! for a user name or a passphrase fails instead of waiting, and an editor
//! git would open is skipped.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::i18n::{tr, Msg};

use super::{git_command, is_branch_name, read, read_line};

pub const EVENT: &str = "git-op";
/// How long a step that talks to a remote may take.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(60);
/// How many of the last lines explain a failure.
const TAIL_LINES: usize = 12;

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GitOp {
    Fetch,
    Pull,
    Push,
    /// Pull, then push.
    Sync,
    Stage { paths: Vec<String> },
    Unstage { paths: Vec<String> },
    StageAll,
    /// Throws changes away: tracked files go back to what the index has,
    /// untracked ones are deleted.
    Discard { tracked: Vec<String>, untracked: Vec<String> },
    #[serde(rename_all = "camelCase")]
    Commit { message: String, amend: bool, all: bool },
    /// `remote`: a remote's branch, to start a local one that tracks it.
    Checkout { branch: String, remote: bool },
    CreateBranch { name: String },
    StashPush,
    StashPop,
}

impl GitOp {
    pub fn name(&self) -> &'static str {
        match self {
            GitOp::Fetch => "fetch",
            GitOp::Pull => "pull",
            GitOp::Push => "push",
            GitOp::Sync => "sync",
            GitOp::Stage { .. } => "stage",
            GitOp::Unstage { .. } => "unstage",
            GitOp::StageAll => "stageAll",
            GitOp::Discard { .. } => "discard",
            GitOp::Commit { .. } => "commit",
            GitOp::Checkout { .. } => "checkout",
            GitOp::CreateBranch { .. } => "createBranch",
            GitOp::StashPush => "stashPush",
            GitOp::StashPop => "stashPop",
        }
    }
}

/// What the steps of an action depend on, read before it starts.
#[derive(Default, Debug)]
pub struct Context {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    /// The remote a first push goes to.
    pub remote: Option<String>,
    /// Whether there's a commit yet.
    pub has_head: bool,
}

impl Context {
    /// As the repository is right now.
    pub fn read(root: &Path) -> Context {
        let remotes: Vec<String> = read(root, &["remote"]).unwrap_or_default().lines().map(str::to_string).collect();
        Context {
            branch: read_line(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]),
            upstream: read_line(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]),
            remote: remotes.iter().find(|name| *name == "origin").or(remotes.first()).cloned(),
            has_head: read(root, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_some(),
        }
    }
}

/// One run of git.
#[derive(Debug, PartialEq)]
pub struct Step {
    pub args: Vec<String>,
    /// Fed to it and closed (a commit's message).
    pub stdin: Option<String>,
    pub network: bool,
}

fn step(args: &[&str]) -> Step {
    Step { args: args.iter().map(|arg| arg.to_string()).collect(), stdin: None, network: false }
}

fn network(args: &[&str]) -> Step {
    Step { network: true, ..step(args) }
}

/// A step over files: always after `--`, so a file's name is never an option.
fn on_paths(args: &[&str], paths: &[String]) -> Step {
    let mut step = step(args);
    step.args.push("--".into());
    step.args.extend(paths.iter().cloned());
    step
}

fn push_step(ctx: &Context) -> Result<Step, String> {
    if ctx.upstream.is_some() {
        return Ok(network(&["push", "--progress"]));
    }
    let branch = ctx.branch.as_deref().ok_or_else(|| tr(Msg::GitDetached))?;
    let remote = ctx.remote.as_deref().ok_or_else(|| tr(Msg::GitNoRemote))?;
    // Its first push: the branch starts tracking what it creates there.
    Ok(network(&["push", "--progress", "--set-upstream", remote, branch]))
}

fn pull_step(ctx: &Context) -> Result<Step, String> {
    if ctx.upstream.is_none() {
        return Err(tr(Msg::GitNoUpstream));
    }
    Ok(network(&["pull", "--progress"]))
}

/// The runs of git an action comes down to, or why it can't be done.
pub fn steps(op: &GitOp, ctx: &Context) -> Result<Vec<Step>, String> {
    let bad_branch = |name: &str| tr(Msg::GitBranchNameInvalid(name));
    Ok(match op {
        GitOp::Fetch => vec![network(&["fetch", "--all", "--progress"])],
        GitOp::Pull => vec![pull_step(ctx)?],
        GitOp::Push => vec![push_step(ctx)?],
        GitOp::Sync => {
            // A branch never pushed has nothing to pull yet.
            if ctx.upstream.is_some() {
                vec![pull_step(ctx)?, push_step(ctx)?]
            } else {
                vec![push_step(ctx)?]
            }
        }
        GitOp::Stage { paths } if paths.is_empty() => Vec::new(),
        GitOp::Stage { paths } => vec![on_paths(&["add", "-A"], paths)],
        GitOp::Unstage { paths } if paths.is_empty() => Vec::new(),
        // Before the first commit there's no HEAD to restore from.
        GitOp::Unstage { paths } if !ctx.has_head => vec![on_paths(&["rm", "--cached", "-r", "-q"], paths)],
        GitOp::Unstage { paths } => vec![on_paths(&["restore", "--staged"], paths)],
        GitOp::StageAll => vec![step(&["add", "-A"])],
        GitOp::Discard { tracked, untracked } => {
            let mut list = Vec::new();
            if !tracked.is_empty() {
                list.push(on_paths(&["restore", "--worktree"], tracked));
            }
            if !untracked.is_empty() {
                list.push(on_paths(&["clean", "-f", "-q"], untracked));
            }
            list
        }
        GitOp::Commit { message, amend, all } => {
            let message = message.trim();
            let mut list = Vec::new();
            if *all {
                list.push(step(&["add", "-A"]));
            }
            if message.is_empty() {
                if !*amend {
                    return Err(tr(Msg::GitCommitNeedsMessage));
                }
                // Amending with no new words keeps the ones it had.
                list.push(step(&["commit", "--amend", "--no-edit"]));
            } else {
                let mut commit = step(&["commit", "-F", "-"]);
                if *amend {
                    commit.args.push("--amend".into());
                }
                commit.stdin = Some(message.to_string());
                list.push(commit);
            }
            list
        }
        GitOp::Checkout { branch, .. } if !is_branch_name(branch) => return Err(bad_branch(branch)),
        GitOp::Checkout { branch, remote: true } => vec![step(&["switch", "--track", branch])],
        GitOp::Checkout { branch, remote: false } => vec![step(&["switch", branch])],
        GitOp::CreateBranch { name } if !is_branch_name(name.trim()) => return Err(bad_branch(name)),
        GitOp::CreateBranch { name } => vec![step(&["switch", "-c", name.trim()])],
        GitOp::StashPush => vec![step(&["stash", "push", "--include-untracked"])],
        GitOp::StashPop => vec![step(&["stash", "pop"])],
    })
}

/// Whether what git said means it wanted a user name, a password or a key's
/// passphrase and had nobody to ask.
pub fn needs_credentials(output: &str) -> bool {
    let said = output.to_ascii_lowercase();
    [
        "terminal prompts disabled",
        "could not read username",
        "could not read password",
        "authentication failed",
        "permission denied (publickey",
        "host key verification failed",
    ]
    .iter()
    .any(|sign| said.contains(sign))
}

struct Running {
    child: Mutex<Option<Child>>,
    cancelled: AtomicBool,
}

/// The actions under way, by repository.
#[derive(Default)]
pub struct GitOps {
    running: Mutex<HashMap<String, Arc<Running>>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpEvent<'a> {
    root: &'a str,
    op: &'a str,
    /// "started", "line" or "finished".
    stage: &'a str,
    line: Option<&'a str>,
    ok: Option<bool>,
}

impl GitOps {
    /// The repositories with an action under way.
    pub fn busy(&self) -> Vec<String> {
        self.running.lock().unwrap().keys().cloned().collect()
    }

    pub fn cancel(&self, root: &str) {
        let Some(running) = self.running.lock().unwrap().get(root).cloned() else { return };
        running.cancelled.store(true, Ordering::Relaxed);
        let mut child = running.child.lock().unwrap();
        if let Some(child) = child.as_mut() {
            crate::agents::runner::kill_tree(child);
        }
    }

    /// Runs an action to its end, reporting as it goes through `git-op`
    /// events. Blocking: call it off the main thread. Returns what git
    /// printed last, or why it failed.
    pub fn run(&self, app: &tauri::AppHandle, root: &str, op: &GitOp, ctx: &Context) -> Result<String, String> {
        self.run_reporting(root, op, ctx, &|stage, line, ok| {
            let _ = app.emit(EVENT, OpEvent { root, op: op.name(), stage, line, ok });
        })
    }

    /// The same, telling `report` of each stage ("started", every "line",
    /// "finished") instead of the windows.
    pub fn run_reporting(
        &self,
        root: &str,
        op: &GitOp,
        ctx: &Context,
        emit: &dyn Fn(&str, Option<&str>, Option<bool>),
    ) -> Result<String, String> {
        if crate::pty::bin_search::find_binary("git").is_none() {
            return Err(tr(Msg::GitNotFound));
        }
        let steps = steps(op, ctx)?;
        let running = Arc::new(Running { child: Mutex::new(None), cancelled: AtomicBool::new(false) });
        {
            let mut all = self.running.lock().unwrap();
            if all.contains_key(root) {
                return Err(tr(Msg::GitBusy));
            }
            all.insert(root.to_string(), running.clone());
        }
        emit("started", None, None);

        let mut tail: Vec<String> = Vec::new();
        let mut result = Ok(());
        for step in &steps {
            result = run_step(Path::new(root), step, &running, &mut |line| {
                emit("line", Some(line), None);
                tail.push(line.to_string());
                if tail.len() > TAIL_LINES {
                    tail.remove(0);
                }
            });
            if result.is_err() {
                break;
            }
        }
        self.running.lock().unwrap().remove(root);
        emit("finished", None, Some(result.is_ok()));

        let said = tail.join("\n");
        match result {
            Ok(()) => Ok(said),
            Err(Failure::Cancelled) => Err(tr(Msg::GitCancelled)),
            Err(Failure::TimedOut) => Err(tr(Msg::GitTimedOut)),
            Err(Failure::Spawn(error)) => Err(tr(Msg::GitFailed(&error))),
            Err(Failure::Exit) if needs_credentials(&said) => Err(tr(Msg::GitNeedsCredentials(&said))),
            Err(Failure::Exit) if said.is_empty() => Err(tr(Msg::GitFailed("git"))),
            // Git's own words say it best.
            Err(Failure::Exit) => Err(said),
        }
    }
}

enum Failure {
    Cancelled,
    TimedOut,
    Spawn(String),
    Exit,
}

/// What a stream prints, a line at a time. Progress is redrawn with `\r`:
/// each redraw counts as a line, so the last one shown is the latest.
fn pump(stream: impl Read + Send + 'static, lines: std::sync::mpsc::Sender<String>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut stream = stream;
        let mut pending: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 4096];
        let flush = |pending: &mut Vec<u8>| {
            let line = String::from_utf8_lossy(pending).trim().to_string();
            pending.clear();
            if !line.is_empty() {
                let _ = lines.send(line);
            }
        };
        while let Ok(read) = stream.read(&mut buffer) {
            if read == 0 {
                break;
            }
            for byte in &buffer[..read] {
                if *byte == b'\n' || *byte == b'\r' {
                    flush(&mut pending);
                } else {
                    pending.push(*byte);
                }
            }
        }
        flush(&mut pending);
    })
}

fn run_step(root: &Path, step: &Step, running: &Running, on_line: &mut dyn FnMut(&str)) -> Result<(), Failure> {
    if running.cancelled.load(Ordering::Relaxed) {
        return Err(Failure::Cancelled);
    }
    let mut command = git_command(root);
    command
        .args(&step.args)
        .stdin(if step.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Nobody to answer: a question fails instead of hanging.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Nor an editor to write in: a merge takes the message git proposes.
        .env("GIT_EDITOR", ":")
        .env("GIT_SEQUENCE_EDITOR", ":")
        .env("GIT_MERGE_AUTOEDIT", "no");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own group, so cancelling ends what it started too (ssh, a hook).
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|err| Failure::Spawn(err.to_string()))?;
    if let (Some(text), Some(mut stdin)) = (&step.stdin, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let (sender, lines) = std::sync::mpsc::channel::<String>();
    let readers = [
        child.stdout.take().map(|out| pump(out, sender.clone())),
        child.stderr.take().map(|err| pump(err, sender.clone())),
    ];
    drop(sender);
    *running.child.lock().unwrap() = Some(child);

    let deadline = Instant::now() + if step.network { NETWORK_TIMEOUT } else { LOCAL_TIMEOUT };
    let mut timed_out = false;
    loop {
        match lines.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => on_line(&line),
            // Both pipes closed: it's over, or about to be.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if !timed_out && Instant::now() >= deadline {
                    timed_out = true;
                    if let Some(child) = running.child.lock().unwrap().as_mut() {
                        crate::agents::runner::kill_tree(child);
                    }
                }
            }
        }
    }
    for reader in readers.into_iter().flatten() {
        let _ = reader.join();
    }
    let status = running.child.lock().unwrap().take().and_then(|mut child| child.wait().ok());
    if running.cancelled.load(Ordering::Relaxed) {
        Err(Failure::Cancelled)
    } else if timed_out {
        Err(Failure::TimedOut)
    } else if status.is_some_and(|status| status.success()) {
        Ok(())
    } else {
        Err(Failure::Exit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked() -> Context {
        Context { branch: Some("topic".into()), upstream: Some("origin/topic".into()), remote: Some("origin".into()), has_head: true }
    }

    fn unpublished() -> Context {
        Context { upstream: None, ..tracked() }
    }

    fn args(op: GitOp, ctx: &Context) -> Vec<String> {
        steps(&op, ctx).unwrap().iter().map(|step| step.args.join(" ")).collect()
    }

    #[test]
    fn a_first_push_sets_what_the_branch_tracks() {
        assert_eq!(args(GitOp::Push, &tracked()), ["push --progress"]);
        assert_eq!(args(GitOp::Push, &unpublished()), ["push --progress --set-upstream origin topic"]);
        assert!(steps(&GitOp::Push, &Context { remote: None, ..unpublished() }).is_err());
        assert!(steps(&GitOp::Push, &Context { branch: None, ..unpublished() }).is_err());
    }

    #[test]
    fn pulling_needs_something_to_pull_from() {
        assert_eq!(args(GitOp::Pull, &tracked()), ["pull --progress"]);
        assert!(steps(&GitOp::Pull, &unpublished()).is_err());
        assert_eq!(args(GitOp::Sync, &tracked()), ["pull --progress", "push --progress"]);
        assert_eq!(args(GitOp::Sync, &unpublished()), ["push --progress --set-upstream origin topic"]);
    }

    #[test]
    fn a_files_name_is_never_read_as_an_option() {
        let paths = vec!["--force".to_string(), "src/a b.rs".to_string()];
        assert_eq!(args(GitOp::Stage { paths: paths.clone() }, &tracked()), ["add -A -- --force src/a b.rs"]);
        assert_eq!(args(GitOp::Unstage { paths: paths.clone() }, &tracked()), ["restore --staged -- --force src/a b.rs"]);
        let fresh = Context { has_head: false, ..tracked() };
        assert_eq!(args(GitOp::Unstage { paths: paths.clone() }, &fresh), ["rm --cached -r -q -- --force src/a b.rs"]);
        let discard = GitOp::Discard { tracked: vec!["a.rs".into()], untracked: vec!["-x".into()] };
        assert_eq!(args(discard, &tracked()), ["restore --worktree -- a.rs", "clean -f -q -- -x"]);
        assert!(steps(&GitOp::Stage { paths: Vec::new() }, &tracked()).unwrap().is_empty());
    }

    #[test]
    fn a_commits_message_goes_in_through_stdin() {
        let commit = GitOp::Commit { message: "  Fix it\n\n-m injected  ".into(), amend: false, all: true };
        let list = steps(&commit, &tracked()).unwrap();
        assert_eq!(list[0].args, ["add", "-A"]);
        assert_eq!(list[1].args, ["commit", "-F", "-"]);
        assert_eq!(list[1].stdin.as_deref(), Some("Fix it\n\n-m injected"));

        let empty = GitOp::Commit { message: " ".into(), amend: false, all: false };
        assert!(steps(&empty, &tracked()).is_err());
        let amend = GitOp::Commit { message: String::new(), amend: true, all: false };
        assert_eq!(args(amend, &tracked()), ["commit --amend --no-edit"]);
        let reword = GitOp::Commit { message: "Better".into(), amend: true, all: false };
        assert_eq!(args(reword, &tracked()), ["commit -F - --amend"]);
    }

    #[test]
    fn a_branchs_name_cant_be_an_option() {
        assert!(steps(&GitOp::Checkout { branch: "--detach".into(), remote: false }, &tracked()).is_err());
        assert!(steps(&GitOp::CreateBranch { name: "-D main".into() }, &tracked()).is_err());
        assert_eq!(args(GitOp::Checkout { branch: "main".into(), remote: false }, &tracked()), ["switch main"]);
        assert_eq!(args(GitOp::Checkout { branch: "origin/topic".into(), remote: true }, &tracked()), ["switch --track origin/topic"]);
        assert_eq!(args(GitOp::CreateBranch { name: " feature/x ".into() }, &tracked()), ["switch -c feature/x"]);
    }

    #[test]
    fn only_steps_that_reach_a_remote_get_the_long_wait() {
        assert!(steps(&GitOp::Fetch, &tracked()).unwrap()[0].network);
        assert!(!steps(&GitOp::StageAll, &tracked()).unwrap()[0].network);
    }

    #[test]
    fn a_missing_credential_is_told_from_any_other_failure() {
        assert!(needs_credentials("fatal: could not read Username for 'https://github.com': terminal prompts disabled"));
        assert!(needs_credentials("git@github.com: Permission denied (publickey)."));
        assert!(!needs_credentials("error: failed to push some refs to 'origin'"));
    }

    /// The whole way through, on a repository made for it: what the panel
    /// does to one, and what it reads back. Skipped where git isn't around.
    #[test]
    fn a_days_work_on_a_real_repository() {
        use crate::git::graph::{self, GraphScope, RefKind};
        use crate::git::snapshot::snapshot;
        use crate::git::GitMark;

        let Some(git) = crate::pty::bin_search::find_binary("git") else { return };
        let base = std::env::temp_dir().join(format!("termo-git-ops-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(base.join("src")).unwrap();
        let git_in = |args: &[&str]| std::process::Command::new(&git).arg("-C").arg(&base).args(args).output().unwrap();
        if !git_in(&["init", "-q", "-b", "main"]).status.success() {
            return;
        }
        git_in(&["config", "user.name", "Termo Test"]);
        git_in(&["config", "user.email", "test@termo.invalid"]);
        git_in(&["config", "commit.gpgsign", "false"]);
        let root = base.to_string_lossy().into_owned();
        let ops = GitOps::default();
        let lines = std::sync::Mutex::new(Vec::<String>::new());
        let run = |op: GitOp| {
            ops.run_reporting(&root, &op, &Context::read(&base), &|stage, line, _| {
                lines.lock().unwrap().push(format!("{stage}:{}", line.unwrap_or_default()));
            })
        };
        let mark_of = |path: &str| {
            let snap = snapshot(&base);
            snap.files.iter().find(|file| file.path == path).map(|file| (file.staged, file.unstaged))
        };

        // A new file, staged, taken back out, and committed with everything.
        std::fs::write(base.join("src").join("a b.txt"), "one\n").unwrap();
        assert_eq!(mark_of("src/a b.txt"), Some((None, Some(GitMark::Untracked))));
        run(GitOp::Stage { paths: vec!["src/a b.txt".into()] }).unwrap();
        assert_eq!(mark_of("src/a b.txt"), Some((Some(GitMark::Added), None)));
        run(GitOp::Unstage { paths: vec!["src/a b.txt".into()] }).unwrap();
        assert_eq!(mark_of("src/a b.txt"), Some((None, Some(GitMark::Untracked))));
        let message = "First commit\n\n--amend isn't an option here\n- nor is this".to_string();
        run(GitOp::Commit { message: message.clone(), amend: false, all: true }).unwrap();
        let snap = snapshot(&base);
        assert!(snap.files.is_empty());
        assert_eq!(snap.branch.as_deref(), Some("main"));
        assert_eq!(snap.head.as_ref().map(|head| head.subject.as_str()), Some("First commit"));
        let first = snap.head.unwrap().sha;
        assert_eq!(graph::commit_detail(&base, &first).unwrap().message, message);

        // A branch, a change on it, and the history both leave.
        run(GitOp::CreateBranch { name: "topic".into() }).unwrap();
        std::fs::write(base.join("src").join("a b.txt"), "one\ntwo\n").unwrap();
        assert_eq!(mark_of("src/a b.txt"), Some((None, Some(GitMark::Modified))));
        run(GitOp::Commit { message: "Second".into(), amend: false, all: true }).unwrap();
        let commits = graph::graph(&base, GraphScope::All, &[], 0, 50);
        assert_eq!(commits.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["Second", "First commit"]);
        assert_eq!(commits[0].parents, [first.clone()]);
        assert!(commits[0].refs.iter().any(|label| label.name == "topic" && label.kind == RefKind::Head));
        assert!(commits[1].refs.iter().any(|label| label.name == "main" && label.kind == RefKind::Local));
        let branches = graph::branches(&base);
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().any(|branch| branch.name == "topic" && branch.current));
        // No remote here: main is the base, and topic is one commit past it.
        let base_ref = snapshot(&base).base.unwrap();
        assert_eq!((base_ref.name.as_str(), base_ref.ahead, base_ref.behind), ("main", 1, 0));

        // A file as it was, and what a commit changed.
        assert_eq!(graph::blob(&base, &first, "src/a b.txt"), Ok(Some("one\n".into())));
        assert_eq!(graph::blob(&base, "HEAD", "src/a b.txt"), Ok(Some("one\ntwo\n".into())));
        assert_eq!(graph::blob(&base, "HEAD", "nope.txt"), Ok(None));
        let detail = graph::commit_detail(&base, &commits[0].sha).unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!((detail.files[0].path.as_str(), detail.files[0].mark), ("src/a b.txt", GitMark::Modified));

        // Stashing and back, throwing changes away, and switching branch.
        std::fs::write(base.join("src").join("a b.txt"), "changed\n").unwrap();
        std::fs::write(base.join("new.txt"), "new\n").unwrap();
        run(GitOp::StashPush).unwrap();
        assert!(snapshot(&base).files.is_empty());
        assert_eq!(snapshot(&base).stash_count, 1);
        run(GitOp::StashPop).unwrap();
        assert_eq!(snapshot(&base).files.len(), 2);
        run(GitOp::Discard { tracked: vec!["src/a b.txt".into()], untracked: vec!["new.txt".into()] }).unwrap();
        assert!(snapshot(&base).files.is_empty());
        assert!(!base.join("new.txt").exists());
        run(GitOp::Checkout { branch: "main".into(), remote: false }).unwrap();
        assert_eq!(snapshot(&base).branch.as_deref(), Some("main"));

        // What can't be done says why, and leaves nothing running.
        assert!(run(GitOp::Checkout { branch: "no-such-branch".into(), remote: false }).is_err());
        assert!(run(GitOp::Push).is_err());
        assert!(ops.busy().is_empty());
        let said = lines.lock().unwrap();
        assert!(said.iter().any(|line| line == "started:") && said.iter().any(|line| line == "finished:"));
        drop(said);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn actions_arrive_from_the_frontend_by_kind() {
        let op: GitOp = serde_json::from_str(r#"{"kind":"commit","message":"x","amend":false,"all":true}"#).unwrap();
        assert_eq!(op, GitOp::Commit { message: "x".into(), amend: false, all: true });
        let op: GitOp = serde_json::from_str(r#"{"kind":"stashPush"}"#).unwrap();
        assert_eq!(op, GitOp::StashPush);
        let op: GitOp = serde_json::from_str(r#"{"kind":"checkout","branch":"main","remote":false}"#).unwrap();
        assert_eq!(op.name(), "checkout");
    }
}
