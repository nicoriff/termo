//! A repository as the panel and the status bar show it: the branch and how
//! far it is from its upstream and from the base branch, what changed, what
//! git is in the middle of, and its remotes. Reads only (see `git/mod.rs`).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use super::forge::{self, Forge};
use super::{brings_filters, read, read_line, GitMark};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// As git names it: relative to the repository, with forward slashes.
    pub path: String,
    /// The same file on disk.
    pub abs_path: String,
    /// What's in the index against the last commit.
    pub staged: Option<GitMark>,
    /// What's on disk against the index.
    pub unstaged: Option<GitMark>,
    pub conflict: bool,
    /// Where a renamed file was.
    pub orig_path: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeadCommit {
    pub sha: String,
    pub subject: String,
    pub author: String,
    /// Seconds since the epoch.
    pub time: i64,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RepoState {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
    pub forge: Option<Forge>,
}

/// The branch work here usually ends up in, and how far HEAD is from it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaseRef {
    pub name: String,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshot {
    pub root: String,
    /// None on a detached HEAD.
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    /// The upstream is configured but no longer exists on the remote.
    pub upstream_gone: bool,
    pub ahead: u32,
    pub behind: u32,
    /// None before the first commit.
    pub head: Option<HeadCommit>,
    pub base: Option<BaseRef>,
    pub files: Vec<FileChange>,
    /// The repository's config brings filter programs, so `git status` was
    /// left unasked (see `git/mod.rs`): `files` says nothing.
    pub changes_unavailable: bool,
    pub stash_count: u32,
    pub state: Option<RepoState>,
    /// `index.lock` is there: some git is at work on this repository.
    pub locked: bool,
    /// When something was last fetched, in ms since the epoch.
    pub last_fetch: Option<u64>,
    pub remotes: Vec<Remote>,
}

pub(crate) const STATUS_ARGS: &[&str] =
    &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all", "--ignore-submodules=all"];

#[derive(Default, Debug, PartialEq)]
pub struct Status {
    pub oid: Option<String>,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    /// None with an upstream set means it's gone.
    pub ahead_behind: Option<(u32, u32)>,
    pub files: Vec<(String, Option<GitMark>, Option<GitMark>, bool, Option<String>)>,
}

fn staged_mark(code: u8) -> Option<GitMark> {
    match code {
        b'M' | b'T' => Some(GitMark::Modified),
        b'A' | b'C' => Some(GitMark::Added),
        b'D' => Some(GitMark::Deleted),
        b'R' => Some(GitMark::Renamed),
        _ => None,
    }
}

fn unstaged_mark(code: u8) -> Option<GitMark> {
    match code {
        b'M' | b'T' => Some(GitMark::Modified),
        // Marked to be added (`git add -N`): new on disk, nothing in the index.
        b'A' => Some(GitMark::Added),
        b'D' => Some(GitMark::Deleted),
        b'R' | b'C' => Some(GitMark::Renamed),
        _ => None,
    }
}

/// `git status --porcelain=v2 --branch -z`: NUL-separated records, the branch
/// in `# branch.*` headers, and one record per file whose first word says
/// its shape (`1` changed, `2` renamed with the old path as one more record,
/// `u` unmerged, `?` untracked).
pub fn parse_status(output: &str) -> Status {
    let mut status = Status::default();
    let mut records = output.split('\0').filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" if value != "(initial)" => status.oid = Some(value.to_string()),
                "branch.head" if value != "(detached)" => status.branch = Some(value.to_string()),
                "branch.upstream" => status.upstream = Some(value.to_string()),
                "branch.ab" => {
                    let mut numbers = value.split(' ').filter_map(|part| part[1..].parse::<u32>().ok());
                    if let (Some(ahead), Some(behind)) = (numbers.next(), numbers.next()) {
                        status.ahead_behind = Some((ahead, behind));
                    }
                }
                _ => {}
            }
            continue;
        }
        let (shape, rest) = record.split_once(' ').unwrap_or((record, ""));
        match shape {
            "?" => status.files.push((rest.to_string(), None, Some(GitMark::Untracked), false, None)),
            "1" | "2" => {
                // The path is the last field, and may hold spaces itself.
                let fields = if shape == "1" { 8 } else { 9 };
                let mut parts = rest.splitn(fields, ' ');
                let xy = parts.next().unwrap_or("..").as_bytes();
                let Some(path) = parts.last() else { continue };
                let orig = if shape == "2" { records.next().map(str::to_string) } else { None };
                if xy.len() < 2 {
                    continue;
                }
                status.files.push((path.to_string(), staged_mark(xy[0]), unstaged_mark(xy[1]), false, orig));
            }
            "u" => {
                if let Some(path) = rest.splitn(10, ' ').last() {
                    status.files.push((path.to_string(), None, Some(GitMark::Conflict), true, None));
                }
            }
            _ => {}
        }
    }
    status
}

/// `name<TAB>url (fetch|push)` lines of `git remote -v`.
pub fn parse_remotes(output: &str) -> Vec<Remote> {
    let mut remotes: Vec<Remote> = Vec::new();
    for line in output.lines() {
        let Some((name, rest)) = line.split_once('\t') else { continue };
        let (url, role) = rest.rsplit_once(' ').unwrap_or((rest, "(fetch)"));
        let shown = forge::redact(url);
        let at = match remotes.iter().position(|remote| remote.name == name) {
            Some(at) => at,
            None => {
                remotes.push(Remote {
                    name: name.to_string(),
                    fetch_url: shown.clone(),
                    push_url: shown.clone(),
                    forge: forge::parse(url),
                });
                remotes.len() - 1
            }
        };
        if role == "(push)" {
            remotes[at].push_url = shown;
        } else {
            remotes[at].fetch_url = shown;
            remotes[at].forge = forge::parse(url);
        }
    }
    remotes
}

/// Git's own folder for this working tree, which in a worktree isn't `.git`.
pub fn git_dir(root: &Path) -> PathBuf {
    read_line(root, &["rev-parse", "--absolute-git-dir"]).map_or_else(|| root.join(".git"), PathBuf::from)
}

fn state_of(git_dir: &Path) -> Option<RepoState> {
    let has = |name: &str| git_dir.join(name).exists();
    if has("rebase-merge") || has("rebase-apply") {
        Some(RepoState::Rebase)
    } else if has("MERGE_HEAD") {
        Some(RepoState::Merge)
    } else if has("CHERRY_PICK_HEAD") {
        Some(RepoState::CherryPick)
    } else if has("REVERT_HEAD") {
        Some(RepoState::Revert)
    } else if has("BISECT_LOG") {
        Some(RepoState::Bisect)
    } else {
        None
    }
}

fn modified_ms(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// `A<TAB>B` of `rev-list --left-right --count HEAD...other`: what HEAD has
/// that the other doesn't, and the other way around.
pub fn ahead_behind(root: &Path, other: &str) -> Option<(u32, u32)> {
    let range = format!("HEAD...{other}");
    let line = read_line(root, &["rev-list", "--left-right", "--count", &range, "--"])?;
    let mut numbers = line.split_whitespace().filter_map(|part| part.parse::<u32>().ok());
    Some((numbers.next()?, numbers.next()?))
}

/// The remote a branch's work goes to: its upstream's, else `origin`, else
/// the first one there is.
pub fn main_remote<'a>(upstream: Option<&str>, remotes: &'a [Remote]) -> Option<&'a Remote> {
    let of_upstream = upstream
        .and_then(|upstream| remotes.iter().filter(|remote| upstream.starts_with(&format!("{}/", remote.name))).max_by_key(|remote| remote.name.len()));
    of_upstream.or_else(|| remotes.iter().find(|remote| remote.name == "origin")).or_else(|| remotes.first())
}

/// The branch the remote calls its default (`origin/HEAD`), or the usual
/// names when the clone doesn't say.
pub fn base_ref(root: &Path, remote: Option<&str>) -> Option<String> {
    let exists = |name: &str| read(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")]).is_some();
    if let Some(remote) = remote {
        let head = format!("refs/remotes/{remote}/HEAD");
        if let Some(target) = read_line(root, &["symbolic-ref", "--quiet", "--short", &head]) {
            return Some(target);
        }
        for name in ["main", "master", "develop"] {
            let candidate = format!("{remote}/{name}");
            if exists(&format!("refs/remotes/{candidate}")) {
                return Some(candidate);
            }
        }
    }
    ["main", "master"].into_iter().find(|name| exists(&format!("refs/heads/{name}"))).map(str::to_string)
}

pub fn snapshot(root: &Path) -> RepoSnapshot {
    let mut snap = RepoSnapshot { root: root.to_string_lossy().into_owned(), ..RepoSnapshot::default() };

    // A repository whose own config brings filters goes without its changes
    // rather than have Termo start its programs just by being looked at.
    snap.changes_unavailable = brings_filters(root);
    if snap.changes_unavailable {
        snap.branch = read_line(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
        snap.detached = snap.branch.is_none();
        snap.upstream = read_line(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]);
        match snap.upstream.as_deref().and_then(|upstream| ahead_behind(root, upstream)) {
            Some((ahead, behind)) => (snap.ahead, snap.behind) = (ahead, behind),
            None => snap.upstream_gone = snap.upstream.is_some(),
        }
    } else if let Some(output) = read(root, STATUS_ARGS) {
        let status = parse_status(&output);
        snap.detached = status.branch.is_none();
        snap.branch = status.branch;
        snap.upstream_gone = status.upstream.is_some() && status.ahead_behind.is_none();
        snap.upstream = status.upstream;
        (snap.ahead, snap.behind) = status.ahead_behind.unwrap_or_default();
        snap.files = status
            .files
            .into_iter()
            .map(|(path, staged, unstaged, conflict, orig_path)| FileChange {
                abs_path: root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)).to_string_lossy().into_owned(),
                path,
                staged,
                unstaged,
                conflict,
                orig_path,
            })
            .collect();
    }

    let format = "--format=%H%x1f%s%x1f%an%x1f%at";
    if let Some(line) = read_line(root, &["log", "-1", "--no-show-signature", format]) {
        let mut parts = line.split('\x1f');
        if let (Some(sha), Some(subject), Some(author), Some(time)) = (parts.next(), parts.next(), parts.next(), parts.next()) {
            snap.head = Some(HeadCommit {
                sha: sha.to_string(),
                subject: subject.to_string(),
                author: author.to_string(),
                time: time.parse().unwrap_or_default(),
            });
        }
    }

    snap.remotes = read(root, &["remote", "-v"]).map(|out| parse_remotes(&out)).unwrap_or_default();

    if snap.head.is_some() {
        let remote = main_remote(snap.upstream.as_deref(), &snap.remotes).map(|remote| remote.name.clone());
        let base = base_ref(root, remote.as_deref());
        // Not when it's what the branch already tracks, or the branch itself:
        // that distance is shown as the upstream's, or is none.
        let same = |name: &str| snap.upstream.as_deref() == Some(name) || snap.branch.as_deref() == Some(name);
        if let Some(name) = base.filter(|name| !same(name)) {
            if let Some((ahead, behind)) = ahead_behind(root, &name) {
                snap.base = Some(BaseRef { name, ahead, behind });
            }
        }
    }

    let dir = git_dir(root);
    snap.state = state_of(&dir);
    snap.locked = dir.join("index.lock").exists();
    snap.last_fetch = modified_ms(&dir.join("FETCH_HEAD"));
    snap.stash_count = read_line(root, &["rev-list", "--walk-reflogs", "--count", "refs/stash", "--"])
        .and_then(|count| count.parse().ok())
        .unwrap_or(0);
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_branch_and_how_far_it_is_from_its_upstream() {
        let status = parse_status("# branch.oid abc123\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -3\0");
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.upstream.as_deref(), Some("origin/main"));
        assert_eq!(status.ahead_behind, Some((2, 3)));
        assert_eq!(status.oid.as_deref(), Some("abc123"));
    }

    #[test]
    fn a_detached_head_and_a_first_commit_have_no_branch_or_commit() {
        let status = parse_status("# branch.oid (initial)\0# branch.head (detached)\0");
        assert!(status.branch.is_none());
        assert!(status.oid.is_none());
    }

    #[test]
    fn an_upstream_without_counts_is_gone() {
        let status = parse_status("# branch.oid abc\0# branch.head topic\0# branch.upstream origin/topic\0");
        assert!(status.upstream.is_some());
        assert!(status.ahead_behind.is_none());
    }

    #[test]
    fn parses_every_shape_of_change() {
        let output = [
            "1 .M N... 100644 100644 100644 aaa bbb src/a b.rs",
            "1 M. N... 100644 100644 100644 aaa bbb staged.rs",
            "1 MM N... 100644 100644 100644 aaa bbb both.rs",
            "1 A. N... 000000 100644 100644 000 bbb new.rs",
            "1 .D N... 100644 100644 000000 aaa aaa gone.rs",
            "2 R. N... 100644 100644 100644 aaa aaa R100 to.rs",
            "from.rs",
            "u UU N... 100644 100644 100644 100644 aaa bbb ccc clash.rs",
            "? notes with space.md",
        ]
        .join("\0");
        let files = parse_status(&output).files;
        let expect = [
            ("src/a b.rs", None, Some(GitMark::Modified), false, None),
            ("staged.rs", Some(GitMark::Modified), None, false, None),
            ("both.rs", Some(GitMark::Modified), Some(GitMark::Modified), false, None),
            ("new.rs", Some(GitMark::Added), None, false, None),
            ("gone.rs", None, Some(GitMark::Deleted), false, None),
            ("to.rs", Some(GitMark::Renamed), None, false, Some("from.rs")),
            ("clash.rs", None, Some(GitMark::Conflict), true, None),
            ("notes with space.md", None, Some(GitMark::Untracked), false, None),
        ];
        assert_eq!(files.len(), expect.len());
        for (file, (path, staged, unstaged, conflict, orig)) in files.iter().zip(expect) {
            assert_eq!(file, &(path.to_string(), staged, unstaged, conflict, orig.map(str::to_string)));
        }
    }

    #[test]
    fn remotes_keep_both_urls_and_never_a_password() {
        let remotes = parse_remotes(
            "origin\thttps://nico:secret@github.com/nicoriff/termo.git (fetch)\norigin\tgit@github.com:nicoriff/termo.git (push)\nfork\t/srv/git/termo.git (fetch)\nfork\t/srv/git/termo.git (push)\n",
        );
        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].fetch_url, "https://nico@github.com/nicoriff/termo.git");
        assert_eq!(remotes[0].push_url, "git@github.com:nicoriff/termo.git");
        assert_eq!(remotes[0].forge.as_ref().map(|forge| forge.repo.as_str()), Some("termo"));
        assert!(remotes[1].forge.is_none());
    }

    #[test]
    fn the_upstreams_remote_wins_over_origin() {
        let remotes = parse_remotes("origin\thttps://github.com/a/b (fetch)\nfork\thttps://github.com/c/b (fetch)\n");
        assert_eq!(main_remote(Some("fork/topic"), &remotes).map(|remote| remote.name.as_str()), Some("fork"));
        assert_eq!(main_remote(None, &remotes).map(|remote| remote.name.as_str()), Some("origin"));
        assert_eq!(main_remote(Some("gone/topic"), &remotes).map(|remote| remote.name.as_str()), Some("origin"));
        assert!(main_remote(None, &[]).is_none());
    }

    /// A folder that came from elsewhere, whose `.git/config` names programs:
    /// looking at it must not start them. Skipped where git isn't around.
    #[test]
    fn a_snapshot_never_starts_the_programs_the_config_names() {
        let Some(git) = crate::pty::bin_search::find_binary("git") else { return };
        let base = std::env::temp_dir().join(format!("termo-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let git_in = |args: &[&str]| std::process::Command::new(&git).arg("-C").arg(&base).args(args).output().unwrap();
        if !git_in(&["init", "-q"]).status.success() {
            return;
        }
        std::fs::write(base.join("a.txt"), "a").unwrap();
        let marker = base.join("ran.txt");
        let command = format!("echo ran > '{}'", marker.to_string_lossy().replace('\\', "/"));

        git_in(&["config", "core.fsmonitor", &command]);
        git_in(&["config", "gpg.program", &command]);
        git_in(&["config", "log.showSignature", "true"]);
        let snap = snapshot(&base);
        assert!(!marker.exists(), "the snapshot started a program");
        assert!(snap.files.iter().any(|file| file.path == "a.txt" && file.unstaged == Some(GitMark::Untracked)));

        git_in(&["config", "filter.x.clean", &command]);
        std::fs::write(base.join(".gitattributes"), "* filter=x\n").unwrap();
        let snap = snapshot(&base);
        assert!(snap.changes_unavailable);
        assert!(snap.files.is_empty());
        assert!(!marker.exists(), "the snapshot started a filter");
        let _ = std::fs::remove_dir_all(&base);
    }
}
