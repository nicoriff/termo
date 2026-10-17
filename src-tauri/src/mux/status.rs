//! What the status bar of a page showing a console says of it, as far as
//! the daemon can tell by itself — so it says it with Termo closed too:
//! the folder its shell is in, and that folder's repository. The agent's
//! state comes from the app (see Op::Agent).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::protocol::GitInfo;

/// How long a repository's branch and changes are taken as still good:
/// every page polls the list, and `git status` isn't free.
const GIT_FOR: Duration = Duration::from_secs(10);

/// The last folder a shell said it's in (OSC 7: `ESC ] 7 ; file://host/path`,
/// see pty/cwd_report.rs) in what a console printed, if it said one. The
/// same reading as the app's (src/lib/terminal/osc7.ts).
pub fn last_cwd(bytes: &[u8]) -> Option<String> {
    const START: &[u8] = b"\x1b]7;";
    let at = bytes.windows(START.len()).rposition(|window| window == START)?;
    let rest = &bytes[at + START.len()..];
    let end = rest.iter().position(|&byte| byte == 0x07 || byte == 0x1b)?;
    let payload = std::str::from_utf8(&rest[..end]).ok()?.trim();
    if !payload.get(..7).is_some_and(|scheme| scheme.eq_ignore_ascii_case("file://")) {
        return None;
    }
    let after = &payload[7..];
    let path = percent_decode(&after[after.find('/')?..]);
    if path.is_empty() || is_network_path(&path) {
        return None;
    }
    // "/C:/Users/x" is a Windows path in a URL's clothes; "/home/x" isn't.
    let windows = path.len() >= 4 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' && path.as_bytes()[3] == b'/'
        && path.as_bytes()[1].is_ascii_alphabetic();
    Some(if windows { path[1..].replace('/', "\\") } else { path })
}

/// `//server/share` (or with backslashes): looking in it would have this
/// computer reach a server named by whatever the console printed — and
/// Windows hand that server the user's credentials (NTLM) on the way.
fn is_network_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && matches!(bytes[0], b'/' | b'\\') && matches!(bytes[1], b'/' | b'\\')
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |at: usize| bytes.get(at).and_then(|byte| (*byte as char).to_digit(16));
        match (bytes[i], hex(i + 1), hex(i + 2)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// The repository `cwd` is in: its branch, and whether anything is
/// uncommitted. Remembered a while per repository (see GIT_FOR).
pub fn git_info(cwd: &str) -> Option<GitInfo> {
    static KNOWN: Mutex<Option<HashMap<PathBuf, (Instant, Option<GitInfo>)>>> = Mutex::new(None);
    let root = crate::git::repo_root(Path::new(cwd))?;
    {
        let known = KNOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((at, info)) = known.as_ref().and_then(|known| known.get(&root)) {
            if at.elapsed() < GIT_FOR {
                return info.clone();
            }
        }
    }
    let info = branch(&root).map(|branch| GitInfo {
        branch,
        // `git status` would run the programs a repository's own config
        // names (a filter): not in one from who knows where.
        dirty: (!crate::git::brings_filters(&root))
            .then(|| crate::git::read(&root, &["status", "--porcelain"]).map(|out| !out.trim().is_empty()))
            .flatten(),
    });
    let mut known = KNOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let known = known.get_or_insert_with(HashMap::new);
    known.retain(|_, (at, _)| at.elapsed() < GIT_FOR);
    known.insert(root, (Instant::now(), info.clone()));
    info
}

/// The branch checked out in `root`, read from its HEAD — no git to run —,
/// or the commit's first letters when none is.
fn branch(root: &Path) -> Option<String> {
    let dot_git = root.join(".git");
    // A worktree's .git is a file naming the folder that has its HEAD.
    let git_dir = if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let named = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
        if named.is_absolute() { named } else { root.join(named) }
    } else {
        dot_git
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            Some(reference.strip_prefix("refs/heads/").unwrap_or(reference).to_string())
        }
        None => Some(head.chars().take(7).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_folder_a_shell_says_its_in() {
        assert_eq!(last_cwd(b"\x1b]7;file:///C:/Users/me%20too\x07PS> ").as_deref(), Some("C:\\Users\\me too"));
        assert_eq!(last_cwd(b"\x1b]7;file://box/home/me\x1b\\$ ").as_deref(), Some("/home/me"));
        // The last one counts.
        assert_eq!(last_cwd(b"\x1b]7;file:///a\x07 \x1b]7;file:///b\x07").as_deref(), Some("/b"));
        assert_eq!(last_cwd(b"no folder here"), None);
        assert_eq!(last_cwd(b"\x1b]7;https://example.com/x\x07"), None);
        // A server's share is no folder to look in.
        assert_eq!(last_cwd(b"\x1b]7;file:////evil/share\x07"), None);
        assert_eq!(last_cwd(b"\x1b]7;file://host/%5C%5Cevil%5Cshare\x07"), None);
        assert_eq!(last_cwd(b"\x1b]7;file:///%5Cevil%5Cshare\x07"), None);
        // Cut before its end: the next output finishes it.
        assert_eq!(last_cwd(b"\x1b]7;file:///hal"), None);
    }

    #[test]
    fn tells_the_branch_from_head() {
        let root = std::env::temp_dir().join(format!("termo-git-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git").join("HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(branch(&root).as_deref(), Some("feature/x"));
        std::fs::write(root.join(".git").join("HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(branch(&root).as_deref(), Some("0123456"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
