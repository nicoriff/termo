//! The history as the graph draws it: commits with their parents and the
//! refs on them, the branches and how far each is from what it tracks, one
//! commit in full, and a file as it was at some point. Reads only (see
//! `git/mod.rs`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, Msg};

use super::{is_sha, read, read_bytes, read_line, GitMark};

/// Past this, a file's old version isn't shown, like the editor's own limit.
const MAX_BLOB_BYTES: usize = 5 * 1024 * 1024;
const BINARY_SNIFF_BYTES: usize = 8192;
/// One page of the graph.
pub const MAX_PAGE: u32 = 500;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    /// The branch HEAD is on.
    Head,
    Local,
    Remote,
    Tag,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphCommit {
    pub sha: String,
    pub parents: Vec<String>,
    pub refs: Vec<RefLabel>,
    /// HEAD is here without a branch.
    pub detached_head: bool,
    pub subject: String,
    pub author: String,
    /// Seconds since the epoch.
    pub time: i64,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GraphScope {
    /// The branch in hand, what it tracks and the base branch.
    Current,
    /// Every branch, remote branch and tag.
    All,
}

/// `%D` with `--decorate=full`: `HEAD -> refs/heads/main, refs/remotes/
/// origin/main, tag: refs/tags/v1`.
pub fn parse_refs(decoration: &str) -> (Vec<RefLabel>, bool) {
    let mut refs = Vec::new();
    let mut detached = false;
    for part in decoration.split(", ").map(str::trim).filter(|part| !part.is_empty()) {
        let (on_head, name) = match part.strip_prefix("HEAD -> ") {
            Some(name) => (true, name),
            None => (false, part),
        };
        let name = name.strip_prefix("tag: ").unwrap_or(name);
        if name == "HEAD" {
            detached = true;
        } else if let Some(branch) = name.strip_prefix("refs/heads/") {
            refs.push(RefLabel { name: branch.to_string(), kind: if on_head { RefKind::Head } else { RefKind::Local } });
        } else if let Some(remote) = name.strip_prefix("refs/remotes/") {
            // `origin/HEAD` only repeats where the remote's default branch is.
            if !remote.ends_with("/HEAD") {
                refs.push(RefLabel { name: remote.to_string(), kind: RefKind::Remote });
            }
        } else if let Some(tag) = name.strip_prefix("refs/tags/") {
            refs.push(RefLabel { name: tag.to_string(), kind: RefKind::Tag });
        }
    }
    (refs, detached)
}

const LOG_FORMAT: &str = "--format=%H%x1f%P%x1f%D%x1f%s%x1f%an%x1f%at%x1e";

pub fn parse_log(output: &str) -> Vec<GraphCommit> {
    output
        .split('\x1e')
        .filter_map(|record| {
            let mut fields = record.trim_matches(['\n', '\r']).split('\x1f');
            let sha = fields.next().filter(|sha| !sha.is_empty())?.to_string();
            let parents = fields.next()?.split(' ').filter(|parent| !parent.is_empty()).map(str::to_string).collect();
            let (refs, detached_head) = parse_refs(fields.next()?);
            Some(GraphCommit {
                sha,
                parents,
                refs,
                detached_head,
                subject: fields.next()?.to_string(),
                author: fields.next()?.to_string(),
                time: fields.next()?.parse().unwrap_or_default(),
            })
        })
        .collect()
}

/// A page of history, newest first, children always before their parents.
/// `extra` are more places to start from besides HEAD (the upstream, the
/// base branch); ones that don't exist are left out rather than failing.
pub fn graph(root: &Path, scope: GraphScope, extra: &[String], skip: u32, limit: u32) -> Vec<GraphCommit> {
    let limit = limit.clamp(1, MAX_PAGE).to_string();
    let skip = skip.to_string();
    let mut args = vec!["log", "--no-show-signature", "--date-order", "--decorate=full", LOG_FORMAT, "-n", &limit, "--skip", &skip];
    let existing: Vec<&String> = extra
        .iter()
        .filter(|name| super::is_branch_name(name))
        .filter(|name| read(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")]).is_some())
        .collect();
    match scope {
        GraphScope::All => args.extend(["--branches", "--remotes", "--tags", "HEAD"]),
        GraphScope::Current => {
            args.push("HEAD");
            args.extend(existing.iter().map(|name| name.as_str()));
        }
    }
    args.push("--");
    read(root, &args).map(|out| parse_log(&out)).unwrap_or_default()
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub remote: bool,
    pub current: bool,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// It tracks a branch the remote no longer has.
    pub gone: bool,
    /// Seconds since the epoch.
    pub last_commit_time: i64,
    pub subject: String,
}

const BRANCH_FORMAT: &str =
    "--format=%(refname)%1f%(objectname:short)%1f%(upstream:short)%1f%(upstream:track)%1f%(committerdate:unix)%1f%(HEAD)%1f%(subject)";

/// `[ahead 1, behind 2]`, `[gone]`, or nothing when they're level.
fn parse_track(track: &str) -> (u32, u32, bool) {
    let inner = track.trim().trim_start_matches('[').trim_end_matches(']');
    if inner == "gone" {
        return (0, 0, true);
    }
    let number = |word: &str| {
        inner.split(", ").find_map(|part| part.strip_prefix(word)).and_then(|count| count.trim().parse().ok()).unwrap_or(0)
    };
    (number("ahead "), number("behind "), false)
}

pub fn parse_branches(output: &str) -> Vec<Branch> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\x1f');
            let full = fields.next()?;
            let (name, remote) = match full.strip_prefix("refs/heads/") {
                Some(name) => (name, false),
                None => (full.strip_prefix("refs/remotes/")?, true),
            };
            if remote && name.ends_with("/HEAD") {
                return None;
            }
            let sha = fields.next()?.to_string();
            let upstream = fields.next().filter(|upstream| !upstream.is_empty()).map(str::to_string);
            let (ahead, behind, gone) = parse_track(fields.next()?);
            Some(Branch {
                name: name.to_string(),
                remote,
                sha,
                upstream,
                ahead,
                behind,
                gone,
                last_commit_time: fields.next()?.parse().unwrap_or_default(),
                current: fields.next()? == "*",
                subject: fields.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Local branches first (the newest work on top), then the remotes'.
pub fn branches(root: &Path) -> Vec<Branch> {
    let args = ["for-each-ref", "--sort=-committerdate", BRANCH_FORMAT, "refs/heads", "refs/remotes"];
    let mut list = read(root, &args).map(|out| parse_branches(&out)).unwrap_or_default();
    list.sort_by_key(|branch| branch.remote);
    list
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    pub mark: GitMark,
    pub orig_path: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    /// Subject and body, as written.
    pub message: String,
    /// Against the first parent.
    pub files: Vec<CommitFile>,
}

/// `--name-status -z`: `M\0path\0`, and for a rename `R100\0old\0new\0`.
pub fn parse_name_status(output: &str) -> Vec<CommitFile> {
    let mut files = Vec::new();
    let mut parts = output.split('\0').filter(|part| !part.is_empty());
    while let Some(status) = parts.next() {
        let Some(first) = parts.next() else { break };
        let code = status.as_bytes()[0];
        if code == b'R' || code == b'C' {
            let Some(path) = parts.next() else { break };
            let mark = if code == b'R' { GitMark::Renamed } else { GitMark::Added };
            files.push(CommitFile { path: path.to_string(), mark, orig_path: Some(first.to_string()) });
            continue;
        }
        let mark = match code {
            b'A' => GitMark::Added,
            b'D' => GitMark::Deleted,
            b'U' => GitMark::Conflict,
            _ => GitMark::Modified,
        };
        files.push(CommitFile { path: first.to_string(), mark, orig_path: None });
    }
    files
}

pub fn commit_detail(root: &Path, sha: &str) -> Option<CommitDetail> {
    if !is_sha(sha) {
        return None;
    }
    let format = "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%B";
    let out = read(root, &["show", "-s", "--no-show-signature", format, sha, "--"])?;
    let mut fields = out.splitn(6, '\x1f');
    let full = fields.next()?.trim().to_string();
    let parents: Vec<String> = fields.next()?.split(' ').filter(|parent| !parent.is_empty()).map(str::to_string).collect();
    let (author, email) = (fields.next()?.to_string(), fields.next()?.to_string());
    let time = fields.next()?.parse().unwrap_or_default();
    let message = fields.next()?.trim_end().to_string();

    let mut args = vec!["diff-tree", "-r", "-M", "--no-commit-id", "--name-status", "-z", "--no-ext-diff"];
    match parents.first() {
        Some(parent) => args.extend([parent.as_str(), full.as_str()]),
        // The first commit: everything in it is new.
        None => args.extend(["--root", full.as_str()]),
    }
    args.push("--");
    let files = read(root, &args).map(|out| parse_name_status(&out)).unwrap_or_default();
    Some(CommitDetail { sha: full, parents, author, email, time, message, files })
}

/// A file as it is at `rev`: a commit, `HEAD`, or the index when empty.
/// `Ok(None)` when it isn't there (a file that's new since). Read straight
/// from git's store: no filter or text conversion runs on it.
pub fn blob(root: &Path, rev: &str, path: &str) -> Result<Option<String>, String> {
    if !(rev.is_empty() || rev == "HEAD" || is_sha(rev)) || path.is_empty() || path.contains('\0') {
        return Ok(None);
    }
    let spec = format!("{rev}:{path}");
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some(size) = read_line(root, &["cat-file", "-s", &spec]).and_then(|size| size.parse::<usize>().ok()) else {
        return Ok(None);
    };
    if size > MAX_BLOB_BYTES {
        return Err(tr(Msg::FileTooLarge(name, (MAX_BLOB_BYTES / (1024 * 1024)) as u64)));
    }
    let Some(bytes) = read_bytes(root, &["cat-file", "blob", &spec]) else {
        return Ok(None);
    };
    if bytes.iter().take(BINARY_SNIFF_BYTES).any(|byte| *byte == 0) {
        return Err(tr(Msg::FileNotText(name)));
    }
    String::from_utf8(bytes).map(Some).map_err(|_| tr(Msg::FileNotText(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_are_told_apart_by_where_they_live() {
        let (refs, detached) =
            parse_refs("HEAD -> refs/heads/main, refs/remotes/origin/main, refs/remotes/origin/HEAD, tag: refs/tags/v0.20.1, refs/heads/origin/odd");
        assert!(!detached);
        assert_eq!(
            refs,
            [
                RefLabel { name: "main".into(), kind: RefKind::Head },
                RefLabel { name: "origin/main".into(), kind: RefKind::Remote },
                RefLabel { name: "v0.20.1".into(), kind: RefKind::Tag },
                RefLabel { name: "origin/odd".into(), kind: RefKind::Local },
            ]
        );
        assert_eq!(parse_refs("HEAD, refs/heads/main"), (vec![RefLabel { name: "main".into(), kind: RefKind::Local }], true));
        assert_eq!(parse_refs(""), (vec![], false));
    }

    #[test]
    fn a_log_page_keeps_parents_and_subjects_with_anything_in_them() {
        let output = "c3\x1fb2 a1\x1fHEAD -> refs/heads/main\x1fMerge, with: commas | pipes\x1fNico\x1f300\x1e\nb2\x1fa1\x1f\x1ffix\x1fAna\x1f200\x1e\na1\x1f\x1f\x1ffirst\x1fNico\x1f100\x1e\n";
        let commits = parse_log(output);
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].parents, ["b2", "a1"]);
        assert_eq!(commits[0].subject, "Merge, with: commas | pipes");
        assert_eq!(commits[0].refs[0].kind, RefKind::Head);
        assert!(commits[2].parents.is_empty());
        assert_eq!(commits[1].time, 200);
    }

    #[test]
    fn branches_say_how_far_they_are_from_what_they_track() {
        let output = [
            "refs/heads/main\x1f4dcc2f6\x1forigin/main\x1f[ahead 2, behind 1]\x1f1760000000\x1f*\x1f0.20.1",
            "refs/heads/old\x1f1111111\x1forigin/old\x1f[gone]\x1f1750000000\x1f \x1fold work",
            "refs/heads/local\x1f2222222\x1f\x1f\x1f1755000000\x1f \x1fnever pushed",
            "refs/remotes/origin/HEAD\x1f4dcc2f6\x1f\x1f\x1f1760000000\x1f \x1f0.20.1",
            "refs/remotes/origin/main\x1f3333333\x1f\x1f\x1f1759000000\x1f \x1fearlier",
        ]
        .join("\n");
        let list = parse_branches(&output);
        assert_eq!(list.len(), 4);
        assert_eq!((list[0].ahead, list[0].behind, list[0].current, list[0].gone), (2, 1, true, false));
        assert!(list[1].gone);
        assert!(list[2].upstream.is_none());
        assert!(list[3].remote);
        assert_eq!(list[3].name, "origin/main");
        assert_eq!(parse_track("[behind 4]"), (0, 4, false));
    }

    #[test]
    fn a_commits_files_come_with_their_renames() {
        let files = parse_name_status("M\0src/a.rs\0A\0new.txt\0D\0gone.txt\0R087\0old name.rs\0new name.rs\0");
        assert_eq!(
            files,
            [
                CommitFile { path: "src/a.rs".into(), mark: GitMark::Modified, orig_path: None },
                CommitFile { path: "new.txt".into(), mark: GitMark::Added, orig_path: None },
                CommitFile { path: "gone.txt".into(), mark: GitMark::Deleted, orig_path: None },
                CommitFile { path: "new name.rs".into(), mark: GitMark::Renamed, orig_path: Some("old name.rs".into()) },
            ]
        );
    }

    #[test]
    fn a_blob_is_asked_only_at_a_commit_head_or_the_index() {
        let nowhere = Path::new("termo-no-such-repo");
        assert_eq!(blob(nowhere, "--help", "a.txt"), Ok(None));
        assert_eq!(blob(nowhere, "main~1", "a.txt"), Ok(None));
        assert_eq!(blob(nowhere, "HEAD", ""), Ok(None));
    }
}
