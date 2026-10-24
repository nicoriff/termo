//! One file of a repository, for the editor's toolbar: where it stands
//! (changed, staged, how many lines), the last commit that touched it, and
//! who wrote each of its lines. Reads only (see `git/mod.rs`): what would
//! compare the file on disk is left unasked where the repository's config
//! names a program for that (see `runs_programs_to_look`).

use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use serde::Serialize;

use super::snapshot::{parse_status, STATUS_ARGS};
use super::{git_command, read, read_line, repo_root, runs_programs_to_look, GitMark, READ_ARGS};

/// Past this the editor's text isn't handed to blame.
const MAX_BLAME_BYTES: usize = 5 * 1024 * 1024;
/// What blame calls the lines that aren't in any commit yet.
const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LastCommit {
    pub sha: String,
    /// The first parent: what the commit's change to the file is shown against.
    pub parent: Option<String>,
    pub author: String,
    /// Seconds since the epoch.
    pub time: i64,
    pub subject: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub root: String,
    /// Relative to the repository, with forward slashes.
    pub path: String,
    pub branch: Option<String>,
    /// In some commit or in the index; false for a file git doesn't follow.
    pub tracked: bool,
    /// In the index against the last commit.
    pub staged: Option<GitMark>,
    /// On disk against the index.
    pub unstaged: Option<GitMark>,
    /// Lines added and removed on disk against HEAD; None when unknown.
    pub added: Option<u32>,
    pub removed: Option<u32>,
    /// The repository's config names programs to compare files with, so the
    /// file on disk wasn't compared: `staged`, `unstaged`, `added` and
    /// `removed` say nothing.
    pub changes_unavailable: bool,
    pub last: Option<LastCommit>,
}

/// `file`'s path in `root`, with forward slashes.
fn relative(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    (!rel.is_empty() && !rel.contains('\0')).then_some(rel)
}

pub fn parse_last(line: &str) -> Option<LastCommit> {
    let mut fields = line.trim_end_matches('\n').splitn(5, '\x1f');
    let sha = fields.next()?.to_string();
    let parent = fields.next()?.split(' ').find(|parent| !parent.is_empty()).map(str::to_string);
    let author = fields.next()?.to_string();
    let time = fields.next()?.parse().unwrap_or_default();
    let subject = fields.next()?.to_string();
    (!sha.is_empty()).then_some(LastCommit { sha, parent, author, time, subject })
}

/// `diff --numstat -z` of one file: its lines added and removed, or None for
/// a binary file (git prints `-`) or none at all.
fn parse_numstat_one(output: &str) -> (Option<u32>, Option<u32>) {
    let Some(record) = output.split('\0').find(|record| !record.is_empty()) else { return (Some(0), Some(0)) };
    let mut parts = record.splitn(3, '\t');
    (parts.next().and_then(|n| n.parse().ok()), parts.next().and_then(|n| n.parse().ok()))
}

/// Where `file` stands in the repository it's in; None outside one.
pub fn file_info(file: &Path) -> Option<FileInfo> {
    let root = repo_root(file.parent()?)?;
    let path = relative(&root, file)?;
    let mut info = FileInfo { root: root.to_string_lossy().into_owned(), path: path.clone(), ..FileInfo::default() };
    info.branch = read_line(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    info.tracked = read(&root, &["ls-files", "-z", "--", &path]).is_some_and(|out| !out.is_empty());

    info.changes_unavailable = runs_programs_to_look(&root);
    if !info.changes_unavailable {
        let mut status = STATUS_ARGS.to_vec();
        status.extend(["--", &path]);
        if let Some((_, staged, unstaged, _, _)) = read(&root, &status).and_then(|out| parse_status(&out).files.into_iter().next()) {
            info.staged = staged;
            info.unstaged = unstaged;
        }
        if info.tracked {
            let numstat = ["diff", "--numstat", "-z", "--no-ext-diff", "--no-textconv", "--no-renames", "HEAD", "--", &path];
            (info.added, info.removed) = read(&root, &numstat).map(|out| parse_numstat_one(&out)).unwrap_or_default();
        }
    }

    let format = "--format=%H%x1f%P%x1f%an%x1f%at%x1f%s";
    info.last = read(&root, &["log", "-1", "--no-show-signature", format, "--", &path]).and_then(|out| parse_last(&out));
    Some(info)
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub sha: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    /// Where the file was in this commit.
    pub path: String,
    /// The line isn't in any commit yet: changed in the editor or on disk.
    pub uncommitted: bool,
    /// The commit before, and where the file was in it: what this commit's
    /// change is shown against. None for the commit that brought the file.
    pub previous: Option<String>,
    pub previous_path: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Blame {
    pub root: String,
    /// The file's path in the repository now, with forward slashes.
    pub path: String,
    pub commits: Vec<BlameCommit>,
    /// For each line of the text, from the first: its commit in `commits`
    /// and the line it was in that commit's version of the file (from 1).
    pub lines: Vec<(u32, u32)>,
}

/// `git blame --porcelain`: per group of lines a header (`<sha> <orig>
/// <final> [<count>]`), the commit's fields the first time it shows up, and
/// each line itself after a tab.
pub fn parse_blame(output: &str) -> Blame {
    let mut blame = Blame::default();
    let mut index = std::collections::HashMap::<String, u32>::new();
    let mut current: Option<(u32, u32, u32)> = None;
    for line in output.lines() {
        if line.starts_with('\t') {
            if let Some((commit, orig, final_line)) = current {
                let at = final_line.saturating_sub(1) as usize;
                if blame.lines.len() <= at {
                    blame.lines.resize(at + 1, (commit, orig));
                }
                blame.lines[at] = (commit, orig);
            }
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let header = key.len() == 40 && key.bytes().all(|byte| byte.is_ascii_hexdigit());
        if header {
            let mut numbers = value.split(' ').filter_map(|n| n.parse::<u32>().ok());
            let (orig, final_line) = (numbers.next().unwrap_or(1), numbers.next().unwrap_or(1));
            let commit = *index.entry(key.to_string()).or_insert_with(|| {
                blame.commits.push(BlameCommit { sha: key.to_string(), uncommitted: key == UNCOMMITTED, ..BlameCommit::default() });
                (blame.commits.len() - 1) as u32
            });
            current = Some((commit, orig, final_line));
            continue;
        }
        let Some(commit) = current.and_then(|(commit, _, _)| blame.commits.get_mut(commit as usize)) else { continue };
        match key {
            "author" => commit.author = value.to_string(),
            "author-time" => commit.time = value.parse().unwrap_or_default(),
            "summary" => commit.summary = value.to_string(),
            "filename" => commit.path = value.to_string(),
            "previous" => {
                if let Some((sha, path)) = value.split_once(' ') {
                    commit.previous = Some(sha.to_string());
                    commit.previous_path = Some(path.to_string());
                }
            }
            _ => {}
        }
    }
    blame
}

/// Who wrote each line of `contents`, `file`'s text as the editor has it
/// (so lines changed and not saved yet come back as uncommitted). None for
/// a file git doesn't follow, or where blame would run the repository's
/// programs on the text (a filter's `clean`, which `--contents` goes through).
pub fn blame(file: &Path, contents: &str) -> Option<Blame> {
    if contents.len() > MAX_BLAME_BYTES {
        return None;
    }
    let root = repo_root(file.parent()?)?;
    let path = relative(&root, file)?;
    if runs_programs_to_look(&root) {
        return None;
    }
    let mut child = git_command(&root)
        .args(READ_ARGS)
        .args(["blame", "--porcelain", "--no-textconv", "--contents", "-", "--", &path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Written from another thread: git could fill its output pipe before
    // it's done reading this one.
    let mut stdin = child.stdin.take()?;
    let text = contents.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(text.as_bytes());
    });
    let output = child.wait_with_output().ok()?;
    let _ = writer.join();
    output.status.success().then(|| Blame {
        root: root.to_string_lossy().into_owned(),
        path,
        ..parse_blame(&String::from_utf8_lossy(&output.stdout))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn blame_gives_each_line_its_commit_and_its_line_there() {
        let output = format!(
            "{A} 1 1 2\nauthor Ana\nauthor-time 100\nsummary First\nfilename f.txt\n\tone\n{A} 2 2\n\ttwo\n\
             {UNCOMMITTED} 3 3 1\nauthor Not Committed Yet\nauthor-time 300\nsummary Version of f.txt from f.txt\nprevious {B} f.txt\nfilename f.txt\n\tthree\n\
             {B} 3 4 1\nauthor Beto\nauthor-time 200\nsummary Second\nprevious {A} old.txt\nfilename f.txt\n\tfour\n"
        );
        let blame = parse_blame(&output);
        assert_eq!(blame.lines, vec![(0, 1), (0, 2), (1, 3), (2, 3)]);
        assert_eq!(blame.commits[0].author, "Ana");
        assert_eq!(blame.commits[0].summary, "First");
        assert_eq!(blame.commits[0].path, "f.txt");
        assert!(blame.commits[0].previous.is_none());
        assert!(blame.commits[1].uncommitted);
        assert_eq!(blame.commits[2].time, 200);
        assert_eq!(blame.commits[2].previous.as_deref(), Some(A));
        assert_eq!(blame.commits[2].previous_path.as_deref(), Some("old.txt"));
    }

    #[test]
    fn a_line_that_starts_like_a_header_is_still_a_line() {
        let output = format!("{A} 1 1 1\nauthor Ana\nsummary x\n\t{B} 9 9\n");
        let blame = parse_blame(&output);
        assert_eq!(blame.commits.len(), 1);
        assert_eq!(blame.lines, vec![(0, 1)]);
    }

    #[test]
    fn the_last_commit_reads_its_first_parent() {
        let last = parse_last(&format!("{A}\x1f{B} {A}\x1fAna\x1f100\x1fFix: a | b\n")).unwrap();
        assert_eq!(last.parent.as_deref(), Some(B));
        assert_eq!(last.subject, "Fix: a | b");
        assert!(parse_last(&format!("{A}\x1f\x1fAna\x1f100\x1fFirst")).unwrap().parent.is_none());
    }

    /// A repository of its own in the temp folder, gone when dropped.
    struct Repo(std::path::PathBuf);

    impl Repo {
        fn new() -> Option<Repo> {
            crate::pty::bin_search::find_binary("git")?;
            let repo = Repo(std::env::temp_dir().join(format!("termo-git-file-{}", uuid::Uuid::new_v4())));
            std::fs::create_dir_all(&repo.0).unwrap();
            repo.git(&["init", "-q"]);
            repo.git(&["config", "user.email", "t@example.com"]);
            repo.git(&["config", "user.name", "T"]);
            repo.git(&["config", "commit.gpgsign", "false"]);
            repo.git(&["config", "core.autocrlf", "false"]);
            Some(repo)
        }

        fn git(&self, args: &[&str]) -> String {
            let output = git_command(&self.0).args(args).output().unwrap();
            assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }

        fn commit(&self, name: &str, text: &str, message: &str) -> String {
            std::fs::write(self.0.join(name), text).unwrap();
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
            self.git(&["rev-parse", "HEAD"])
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_real_file_its_info_and_its_blame_with_unsaved_lines() {
        let Some(repo) = Repo::new() else { return };
        let first = repo.commit("f.txt", "a\nb\n", "First");
        let second = repo.commit("f.txt", "a\nB\n", "Second");
        std::fs::write(repo.0.join("f.txt"), "a\nB\nc\n").unwrap();
        let file = repo.0.join("f.txt");

        let info = file_info(&file).unwrap();
        assert_eq!(info.path, "f.txt");
        assert!(info.tracked);
        assert_eq!(info.unstaged, Some(GitMark::Modified));
        assert_eq!((info.added, info.removed), (Some(1), Some(0)));
        let last = info.last.unwrap();
        assert_eq!((last.sha.as_str(), last.parent.as_deref(), last.subject.as_str()), (second.as_str(), Some(first.as_str()), "Second"));

        // The editor's text, with a line typed and not saved.
        let found = blame(&file, "a\nB\nc\nd\n").unwrap();
        assert_eq!(found.path, "f.txt");
        let shas: Vec<&str> = found.lines.iter().map(|(commit, _)| found.commits[*commit as usize].sha.as_str()).collect();
        assert_eq!(shas, vec![first.as_str(), second.as_str(), UNCOMMITTED, UNCOMMITTED]);
        let second_commit = &found.commits[found.lines[1].0 as usize];
        assert_eq!(second_commit.previous.as_deref(), Some(first.as_str()));
        assert_eq!(second_commit.path, "f.txt");

        // A file git doesn't follow has no blame, but still has its info.
        std::fs::write(repo.0.join("new.txt"), "x\n").unwrap();
        assert!(blame(&repo.0.join("new.txt"), "x\n").is_none());
        let info = file_info(&repo.0.join("new.txt")).unwrap();
        assert!(!info.tracked);
        assert_eq!(info.unstaged, Some(GitMark::Untracked));
        assert!(info.last.is_none());
    }

    #[test]
    fn numstat_of_one_file() {
        assert_eq!(parse_numstat_one("12\t3\tsrc/a.ts\0"), (Some(12), Some(3)));
        assert_eq!(parse_numstat_one("-\t-\timg.png\0"), (None, None));
        assert_eq!(parse_numstat_one(""), (Some(0), Some(0)));
    }
}
