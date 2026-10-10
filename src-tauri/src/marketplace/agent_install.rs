//! Installing an agent's CLI for the user, from Preferences or the setup:
//! `npm install -g <package>`, the one way all five ship on every platform.
//!
//! Into npm's own global folder unless another is chosen. A CLI installed
//! elsewhere is on nobody's PATH, so that folder is remembered (AppConfig's
//! `agent_install_dirs`): Termo looks there too (bin_search's extra folders),
//! and the consoles it opens get it on theirs (pty/session.rs). Outside
//! Termo it's the user's to add.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::Serialize;
use tauri::Emitter;

use crate::i18n::{tr, Msg};
use crate::pty::bin_search;

use super::agent_cli;
use super::model::AgentTarget;

/// The event each step and each line of npm's output goes out on.
const EVENT: &str = "agent-cli-install";

/// Each agent's package on npm, as its own install guide names it.
pub fn package(target: AgentTarget) -> &'static str {
    match target {
        AgentTarget::Claude => "@anthropic-ai/claude-code",
        AgentTarget::Codex => "@openai/codex",
        AgentTarget::Gemini => "@google/gemini-cli",
        AgentTarget::Copilot => "@github/copilot",
        AgentTarget::Grok => "@xai-official/grok",
    }
}

/// What the install dialog shows before anything runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    /// npm itself, or None when Node.js isn't installed.
    pub npm: Option<String>,
    /// npm's global folder: where a package goes unless told otherwise.
    pub default_dir: Option<String>,
    pub package: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// The CLI's executable, as Termo now finds it.
    pub path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    tool: &'a str,
    /// "checking", "installing" or "verifying".
    stage: &'a str,
    /// A line npm printed, when this is one.
    line: Option<&'a str>,
}

fn npm() -> Option<PathBuf> {
    bin_search::find_binary("npm")
}

pub fn info(target: AgentTarget) -> InstallInfo {
    let npm = npm();
    let default_dir = npm
        .as_ref()
        .and_then(|npm| agent_cli::run(npm, &["config", "get", "prefix"]).ok())
        .map(|output| output.trim().to_string())
        .filter(|dir| !dir.is_empty());
    InstallInfo { npm: npm.map(|path| path.display().to_string()), default_dir, package: package(target).into() }
}

/// Where a package installed globally under `dir` leaves its commands: the
/// folder itself on Windows, its `bin` elsewhere.
pub fn bin_dirs(dir: &Path) -> [PathBuf; 2] {
    [dir.to_path_buf(), dir.join("bin")]
}

/// The folders the user's own installs went to, for bin_search and for the
/// consoles' PATH. Called at startup and whenever the config is saved.
pub fn apply(dirs: &BTreeMap<String, String>) {
    let mut extra: Vec<PathBuf> = Vec::new();
    for dir in dirs.values().map(|dir| dir.trim()).filter(|dir| !dir.is_empty()) {
        for bin in bin_dirs(Path::new(dir)) {
            if !extra.contains(&bin) {
                extra.push(bin);
            }
        }
    }
    bin_search::set_extra_dirs(extra);
}

/// The install under way, so it can be cancelled. One at a time.
fn running() -> &'static Mutex<Option<Child>> {
    static RUNNING: Mutex<Option<Child>> = Mutex::new(None);
    &RUNNING
}

/// Whether the one under way was cancelled, to say so instead of "failed".
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn cancel() {
    CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(child) = running().lock().unwrap().as_mut() {
        crate::agents::runner::kill_tree(child);
    }
}

/// The folder as a path npm can be handed: absolute, and there (created if
/// need be).
fn checked_dir(dir: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(dir.trim());
    if !path.is_absolute() || std::fs::create_dir_all(&path).is_err() || !path.is_dir() {
        return Err(tr(Msg::AgentInstallBadFolder(dir.trim())));
    }
    Ok(path)
}

/// Installs `target`'s CLI, into `dir` when one is given, reporting each step
/// and each line npm prints. Blocking: run it off the main thread.
pub fn install(app: &tauri::AppHandle, target: AgentTarget, dir: Option<&str>) -> Result<Installed, String> {
    let tool = target.binary_stem();
    let report = |stage: &str, line: Option<&str>| {
        let _ = app.emit(EVENT, Progress { tool, stage, line });
    };
    report("checking", None);
    let npm = npm().ok_or_else(|| tr(Msg::NpmNotFound))?;
    let dir = dir.map(checked_dir).transpose()?;

    let mut slot = running().lock().unwrap();
    if slot.is_some() {
        return Err(tr(Msg::AgentInstallBusy));
    }
    CANCELLED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut cmd = Command::new(&npm);
    cmd.args(["install", "-g", "--no-fund", "--no-audit", "--loglevel", "http"]);
    if let Some(dir) = &dir {
        cmd.arg("--prefix").arg(dir);
    }
    cmd.arg(package(target))
        // Nothing to answer: a question gets EOF instead of hanging.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own group, so cancelling ends what it started too.
        cmd.process_group(0);
    }
    report("installing", None);
    let mut child = cmd.spawn().map_err(|err| tr(Msg::AgentInstallFailed(&err.to_string())))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *slot = Some(child);
    drop(slot);

    // npm's progress goes to stderr; what it installed, to stdout. The last
    // lines of either explain a failure.
    let tail = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
    let pump = |stream: Option<Box<dyn std::io::Read + Send>>| {
        let (app, tail) = (app.clone(), tail.clone());
        std::thread::spawn(move || {
            let Some(stream) = stream else { return };
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let line = line.trim_end();
                if line.is_empty() {
                    continue;
                }
                let _ = app.emit(EVENT, Progress { tool, stage: "installing", line: Some(line) });
                let mut tail = tail.lock().unwrap();
                tail.push(line.to_string());
                if tail.len() > 12 {
                    tail.remove(0);
                }
            }
        })
    };
    let readers = [
        pump(stdout.map(|out| Box::new(out) as Box<dyn std::io::Read + Send>)),
        pump(stderr.map(|err| Box::new(err) as Box<dyn std::io::Read + Send>)),
    ];
    for reader in readers {
        let _ = reader.join();
    }
    // The pipes closed: it's over, or about to be.
    let status = running().lock().unwrap().take().and_then(|mut child| child.wait().ok());
    if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(tr(Msg::AgentInstallCancelled));
    }
    if !status.is_some_and(|status| status.success()) {
        // What npm said last, without the request log around it.
        let said: Vec<String> =
            tail.lock().unwrap().iter().filter(|line| !line.starts_with("npm http")).cloned().collect();
        let detail = if said.is_empty() { "npm".to_string() } else { said.join("\n") };
        return Err(tr(Msg::AgentInstallFailed(&detail)));
    }

    report("verifying", None);
    // Found from now on, before the config that remembers the folder is saved.
    if let Some(dir) = &dir {
        let mut extra = bin_search::extra_dirs();
        for bin in bin_dirs(dir) {
            if !extra.contains(&bin) {
                extra.push(bin);
            }
        }
        bin_search::set_extra_dirs(extra);
    }
    let path = agent_cli::locate(target, None).ok_or_else(|| tr(Msg::AgentInstalledNotFound(tool)))?;
    Ok(Installed { path: path.display().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_has_a_scoped_package() {
        for target in [AgentTarget::Claude, AgentTarget::Codex, AgentTarget::Gemini, AgentTarget::Copilot, AgentTarget::Grok] {
            let name = package(target);
            assert!(name.starts_with('@') && agent_cli::is_valid_component_name(name), "{name}");
        }
    }

    #[test]
    fn a_folder_to_install_into_must_be_absolute() {
        assert!(checked_dir("relative/folder").is_err());
        assert!(checked_dir("").is_err());
        let dir = std::env::temp_dir().join(format!("termo-install-{}", uuid::Uuid::new_v4())).join("nested");
        let checked = checked_dir(&dir.display().to_string()).unwrap();
        assert!(checked.is_dir());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn the_users_folders_are_searched_once_each() {
        let dirs: BTreeMap<String, String> =
            [("grok".to_string(), "/opt/agents".to_string()), ("codex".to_string(), "/opt/agents".to_string()), ("claude".to_string(), " ".to_string())].into();
        apply(&dirs);
        assert_eq!(bin_search::extra_dirs(), [PathBuf::from("/opt/agents"), PathBuf::from("/opt/agents/bin")]);
        apply(&BTreeMap::new());
        assert!(bin_search::extra_dirs().is_empty());
    }
}
