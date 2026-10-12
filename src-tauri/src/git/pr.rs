//! The pull request (GitHub) or merge request (GitLab) of the branch in
//! hand: reading it, changing it and opening one.
//!
//! Asked through each service's own CLI — `gh` and `glab` — with the session
//! the user already has there: Termo keeps no token of its own. Kept for a
//! minute, since it's asked again every time the panel looks.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{tr, Msg};
use crate::pty::bin_search::find_binary;

use super::forge::ForgeKind;
use super::no_window;

const FRESH_FOR: Duration = Duration::from_secs(60);
/// A failure is asked again sooner: the CLI may have just been signed in to.
const FAILURE_FRESH_FOR: Duration = Duration::from_secs(20);
const CLI_TIMEOUT: Duration = Duration::from_secs(30);

const GH_FIELDS: &str = "number,title,body,state,isDraft,url,baseRefName,headRefName,author,labels,assignees,reviewRequests,reviewDecision,mergeable,statusCheckRollup,additions,deletions,changedFiles,comments,updatedAt";

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Cli {
    Gh,
    Glab,
}

impl Cli {
    pub fn for_forge(kind: ForgeKind) -> Option<Cli> {
        match kind {
            ForgeKind::Github => Some(Cli::Gh),
            ForgeKind::Gitlab => Some(Cli::Glab),
            _ => None,
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Cli::Gh => "gh",
            Cli::Glab => "glab",
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PrStatus {
    Open,
    Draft,
    Merged,
    Closed,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrLabel {
    pub name: String,
    /// Six hex digits, when the service gives one.
    pub color: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrChecks {
    pub passed: u32,
    pub failed: u32,
    pub pending: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrInfo {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: PrStatus,
    pub url: String,
    pub base: String,
    pub head: String,
    pub author: String,
    pub labels: Vec<PrLabel>,
    pub reviewers: Vec<String>,
    pub assignees: Vec<String>,
    pub review_decision: Option<ReviewDecision>,
    /// None while the service hasn't worked it out.
    pub mergeable: Option<bool>,
    /// None when nothing checks it.
    pub checks: Option<PrChecks>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub changed_files: Option<u64>,
    pub comments: u64,
    /// As the service writes it (ISO 8601).
    pub updated_at: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PrState {
    Ok { cli: Cli, pr: Box<PrInfo> },
    /// The branch has none.
    None { cli: Cli },
    /// The service's CLI isn't installed.
    NoCli { cli: Cli },
    NotSignedIn { cli: Cli },
    /// A host whose requests no CLI here knows.
    Unsupported,
    Failed { message: String },
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn names(value: &Value, key: &str, field: &str) -> Vec<String> {
    let list = value.get(key).and_then(Value::as_array);
    list.into_iter().flatten().filter_map(|item| item.get(field).and_then(Value::as_str)).map(str::to_string).collect()
}

fn gh_checks(rollup: Option<&Value>) -> Option<PrChecks> {
    let list = rollup?.as_array().filter(|list| !list.is_empty())?;
    let mut checks = PrChecks::default();
    for check in list {
        let upper = |key: &str| check.get(key).and_then(Value::as_str).unwrap_or_default().to_ascii_uppercase();
        // A check run says how it ended once it's done; a commit status has
        // only its state.
        let outcome = if check.get("status").is_some() {
            if upper("status") == "COMPLETED" { upper("conclusion") } else { "PENDING".into() }
        } else {
            upper("state")
        };
        match outcome.as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => checks.passed += 1,
            "PENDING" | "EXPECTED" | "QUEUED" | "IN_PROGRESS" | "" => checks.pending += 1,
            _ => checks.failed += 1,
        }
    }
    Some(checks)
}

/// `gh pr view --json …`.
pub fn parse_gh(body: &Value) -> Option<PrInfo> {
    let number = body.get("number")?.as_u64()?;
    let draft = body.get("isDraft").and_then(Value::as_bool).unwrap_or(false);
    let state = match text(body, "state").to_ascii_uppercase().as_str() {
        "MERGED" => PrStatus::Merged,
        "CLOSED" => PrStatus::Closed,
        _ if draft => PrStatus::Draft,
        _ => PrStatus::Open,
    };
    let labels = body.get("labels").and_then(Value::as_array);
    // A person asked to review has a login; a team, its slug.
    let requests = body.get("reviewRequests").and_then(Value::as_array);
    let reviewers = requests
        .into_iter()
        .flatten()
        .filter_map(|who| ["login", "slug", "name"].iter().find_map(|key| who.get(*key).and_then(Value::as_str)))
        .map(str::to_string)
        .collect();
    Some(PrInfo {
        number,
        title: text(body, "title"),
        body: text(body, "body"),
        state,
        url: text(body, "url"),
        base: text(body, "baseRefName"),
        head: text(body, "headRefName"),
        author: body.pointer("/author/login").and_then(Value::as_str).unwrap_or_default().to_string(),
        labels: labels
            .into_iter()
            .flatten()
            .filter_map(|label| {
                let name = label.get("name")?.as_str()?.to_string();
                let color = label.get("color").and_then(Value::as_str).filter(|color| is_hex_color(color)).map(str::to_string);
                Some(PrLabel { name, color })
            })
            .collect(),
        reviewers,
        assignees: names(body, "assignees", "login"),
        review_decision: match text(body, "reviewDecision").as_str() {
            "APPROVED" => Some(ReviewDecision::Approved),
            "CHANGES_REQUESTED" => Some(ReviewDecision::ChangesRequested),
            "REVIEW_REQUIRED" => Some(ReviewDecision::ReviewRequired),
            _ => None,
        },
        mergeable: match text(body, "mergeable").as_str() {
            "MERGEABLE" => Some(true),
            "CONFLICTING" => Some(false),
            _ => None,
        },
        checks: gh_checks(body.get("statusCheckRollup")),
        additions: body.get("additions").and_then(Value::as_u64),
        deletions: body.get("deletions").and_then(Value::as_u64),
        changed_files: body.get("changedFiles").and_then(Value::as_u64),
        comments: body.get("comments").and_then(Value::as_array).map_or(0, |list| list.len() as u64),
        updated_at: body.get("updatedAt").and_then(Value::as_str).map(str::to_string),
    })
}

fn is_hex_color(color: &str) -> bool {
    color.len() == 6 && color.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// `glab mr view --output json`: GitLab's own merge request object.
pub fn parse_glab(body: &Value) -> Option<PrInfo> {
    let number = body.get("iid")?.as_u64()?;
    let flag = |key: &str| body.get(key).and_then(Value::as_bool);
    let draft = flag("draft").or_else(|| flag("work_in_progress")).unwrap_or(false);
    let state = match text(body, "state").as_str() {
        "merged" => PrStatus::Merged,
        "closed" | "locked" => PrStatus::Closed,
        _ if draft => PrStatus::Draft,
        _ => PrStatus::Open,
    };
    let labels = body.get("labels").and_then(Value::as_array);
    // One pipeline stands for every job in it.
    let checks = body.pointer("/head_pipeline/status").and_then(Value::as_str).map(|status| match status {
        "success" | "skipped" | "manual" => PrChecks { passed: 1, ..PrChecks::default() },
        "failed" | "canceled" => PrChecks { failed: 1, ..PrChecks::default() },
        _ => PrChecks { pending: 1, ..PrChecks::default() },
    });
    Some(PrInfo {
        number,
        title: text(body, "title"),
        body: text(body, "description"),
        state,
        url: text(body, "web_url"),
        base: text(body, "target_branch"),
        head: text(body, "source_branch"),
        author: body.pointer("/author/username").and_then(Value::as_str).unwrap_or_default().to_string(),
        labels: labels
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|name| PrLabel { name: name.to_string(), color: None })
            .collect(),
        reviewers: names(body, "reviewers", "username"),
        assignees: names(body, "assignees", "username"),
        review_decision: None,
        mergeable: flag("has_conflicts").map(|conflicts| !conflicts),
        checks,
        additions: None,
        deletions: None,
        // "12", or "1000+" past what GitLab counts.
        changed_files: body.get("changes_count").and_then(Value::as_str).and_then(|count| count.trim_end_matches('+').parse().ok()),
        comments: body.get("user_notes_count").and_then(Value::as_u64).unwrap_or(0),
        updated_at: body.get("updated_at").and_then(Value::as_str).map(str::to_string),
    })
}

#[derive(Debug)]
enum CliError {
    Missing,
    /// What it printed on its way out.
    Failed(String),
}

/// Runs a service's CLI in the repository, giving up after a while: both ask
/// the network. Its output on success.
fn run_cli(cli: Cli, root: &Path, args: &[String], stdin: Option<&str>) -> Result<String, CliError> {
    let binary = find_binary(cli.binary()).ok_or(CliError::Missing)?;
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(root)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Nothing to answer a question with, and no colors to strip.
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GLAB_CHECK_UPDATE", "false")
        .env("NO_PROMPT", "1")
        .env("NO_COLOR", "1");
    no_window(&mut command);
    let mut child = command.spawn().map_err(|err| CliError::Failed(err.to_string()))?;
    if let (Some(text), Some(mut input)) = (stdin, child.stdin.take()) {
        let _ = input.write_all(text.as_bytes());
    }
    let drain = |stream: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(mut stream) = stream {
                let _ = stream.read_to_end(&mut out);
            }
            String::from_utf8_lossy(&out).into_owned()
        })
    };
    let stdout = drain(child.stdout.take().map(|out| Box::new(out) as Box<dyn Read + Send>));
    let stderr = drain(child.stderr.take().map(|err| Box::new(err) as Box<dyn Read + Send>));
    let deadline = Instant::now() + CLI_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                crate::agents::runner::kill_tree(&mut child);
                let _ = child.wait();
                break None;
            }
        }
    };
    let (out, err) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    match status {
        Some(status) if status.success() => Ok(out),
        Some(_) => Err(CliError::Failed(if err.trim().is_empty() { out } else { err })),
        None => Err(CliError::Failed("timeout".into())),
    }
}

/// What a CLI's failure means: the branch has no request, nobody is signed
/// in, or something else it says in its own words.
pub fn classify_failure(cli: Cli, said: &str) -> PrState {
    let lower = said.to_ascii_lowercase();
    let none = ["no pull requests found", "no open merge request", "no merge request"];
    let signed_out = ["auth login", "not logged in", "http 401", "401 unauthorized", "authentication required", "no token found"];
    if none.iter().any(|sign| lower.contains(sign)) {
        PrState::None { cli }
    } else if signed_out.iter().any(|sign| lower.contains(sign)) {
        PrState::NotSignedIn { cli }
    } else {
        // The first thing it says is the reason; the rest is usage.
        let message = said.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("?").to_string();
        PrState::Failed { message }
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

fn fetch(cli: Cli, root: &Path) -> PrState {
    let args = match cli {
        Cli::Gh => strings(&["pr", "view", "--json", GH_FIELDS]),
        Cli::Glab => strings(&["mr", "view", "--output", "json"]),
    };
    match run_cli(cli, root, &args, None) {
        Ok(out) => {
            let parsed = serde_json::from_str::<Value>(&out).ok().and_then(|body| match cli {
                Cli::Gh => parse_gh(&body),
                Cli::Glab => parse_glab(&body),
            });
            match parsed {
                Some(pr) => PrState::Ok { cli, pr: Box::new(pr) },
                None => PrState::Failed { message: tr(Msg::PrUnreadable(cli.binary())) },
            }
        }
        Err(CliError::Missing) => PrState::NoCli { cli },
        Err(CliError::Failed(said)) => classify_failure(cli, &said),
    }
}

static CACHE: Mutex<Option<HashMap<String, (Instant, PrState)>>> = Mutex::new(None);

fn cache_key(root: &Path, branch: &str) -> String {
    format!("{}\0{branch}", root.to_string_lossy())
}

pub fn forget(root: &Path, branch: &str) {
    if let Some(cache) = CACHE.lock().unwrap().as_mut() {
        cache.remove(&cache_key(root, branch));
    }
}

/// The request of `branch`, on the service `kind` says. Blocking.
pub fn view(root: &Path, branch: &str, kind: Option<ForgeKind>, refresh: bool) -> PrState {
    let Some(cli) = kind.and_then(Cli::for_forge) else {
        return PrState::Unsupported;
    };
    let key = cache_key(root, branch);
    if !refresh {
        if let Some((at, state)) = CACHE.lock().unwrap().as_ref().and_then(|cache| cache.get(&key)) {
            let fresh_for = if matches!(state, PrState::Ok { .. } | PrState::None { .. }) { FRESH_FOR } else { FAILURE_FRESH_FOR };
            if at.elapsed() < fresh_for {
                return state.clone();
            }
        }
    }
    let state = fetch(cli, root);
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, (Instant::now(), state.clone()));
    state
}

/// What to change of a request; what's left out stays as it is.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PrPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub base: Option<String>,
    pub draft: Option<bool>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
    pub add_reviewers: Vec<String>,
    pub remove_reviewers: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct NewPr {
    pub title: String,
    pub body: String,
    pub base: Option<String>,
    pub draft: bool,
}

/// One run of a CLI: its arguments, and what goes in through stdin.
#[derive(Debug, PartialEq)]
pub struct Call {
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

/// Names for a flag that takes a list: trimmed, no empty ones, and none that
/// could split into two.
fn list(names: &[String]) -> Option<String> {
    let clean: Vec<&str> = names.iter().map(|name| name.trim()).filter(|name| !name.is_empty() && !name.contains(',')).collect();
    (!clean.is_empty()).then(|| clean.join(","))
}

/// The runs that apply `patch`. Every value goes as `--flag=value`, so one
/// starting with a dash is still a value.
pub fn update_calls(cli: Cli, patch: &PrPatch) -> Vec<Call> {
    let mut calls = Vec::new();
    match cli {
        Cli::Gh => {
            let mut args = strings(&["pr", "edit"]);
            let mut stdin = None;
            if let Some(title) = &patch.title {
                args.push(format!("--title={title}"));
            }
            if let Some(body) = &patch.body {
                args.push("--body-file=-".into());
                stdin = Some(body.clone());
            }
            if let Some(base) = &patch.base {
                args.push(format!("--base={base}"));
            }
            for (flag, names) in [
                ("--add-label", &patch.add_labels),
                ("--remove-label", &patch.remove_labels),
                ("--add-reviewer", &patch.add_reviewers),
                ("--remove-reviewer", &patch.remove_reviewers),
            ] {
                if let Some(names) = list(names) {
                    args.push(format!("{flag}={names}"));
                }
            }
            if args.len() > 2 {
                calls.push(Call { args, stdin });
            }
            match patch.draft {
                Some(true) => calls.push(Call { args: strings(&["pr", "ready", "--undo"]), stdin: None }),
                Some(false) => calls.push(Call { args: strings(&["pr", "ready"]), stdin: None }),
                None => {}
            }
        }
        Cli::Glab => {
            let mut args = strings(&["mr", "update"]);
            if let Some(title) = &patch.title {
                args.push(format!("--title={title}"));
            }
            if let Some(body) = &patch.body {
                args.push(format!("--description={body}"));
            }
            if let Some(base) = &patch.base {
                args.push(format!("--target-branch={base}"));
            }
            match patch.draft {
                Some(true) => args.push("--draft".into()),
                Some(false) => args.push("--ready".into()),
                None => {}
            }
            if let Some(names) = list(&patch.add_labels) {
                args.push(format!("--label={names}"));
            }
            if let Some(names) = list(&patch.remove_labels) {
                args.push(format!("--unlabel={names}"));
            }
            // One flag for both: `+name` adds, `-name` takes away.
            let signed = |sign: char, names: &[String]| {
                names.iter().map(|name| name.trim()).filter(|name| !name.is_empty() && !name.contains(',')).map(move |name| format!("{sign}{name}")).collect::<Vec<_>>()
            };
            let reviewers = [signed('+', &patch.add_reviewers), signed('-', &patch.remove_reviewers)].concat();
            if !reviewers.is_empty() {
                args.push(format!("--reviewer={}", reviewers.join(",")));
            }
            if args.len() > 2 {
                calls.push(Call { args, stdin: None });
            }
        }
    }
    calls
}

pub fn create_call(cli: Cli, new: &NewPr) -> Call {
    let title = new.title.trim();
    match cli {
        Cli::Gh => {
            let mut args = strings(&["pr", "create"]);
            args.push(format!("--title={title}"));
            args.push("--body-file=-".into());
            if let Some(base) = new.base.as_deref().filter(|base| !base.trim().is_empty()) {
                args.push(format!("--base={}", base.trim()));
            }
            if new.draft {
                args.push("--draft".into());
            }
            Call { args, stdin: Some(new.body.clone()) }
        }
        Cli::Glab => {
            let mut args = strings(&["mr", "create", "--yes"]);
            args.push(format!("--title={title}"));
            args.push(format!("--description={}", new.body));
            if let Some(base) = new.base.as_deref().filter(|base| !base.trim().is_empty()) {
                args.push(format!("--target-branch={}", base.trim()));
            }
            if new.draft {
                args.push("--draft".into());
            }
            Call { args, stdin: None }
        }
    }
}

fn run_calls(cli: Cli, root: &Path, calls: &[Call]) -> Result<(), String> {
    for call in calls {
        run_cli(cli, root, &call.args, call.stdin.as_deref()).map_err(|err| match err {
            CliError::Missing => tr(Msg::PrCliMissing(cli.binary())),
            CliError::Failed(said) => match classify_failure(cli, &said) {
                PrState::NotSignedIn { .. } => tr(Msg::PrNotSignedIn(cli.binary())),
                PrState::Failed { message } => tr(Msg::PrFailed(&message)),
                _ => tr(Msg::PrFailed(said.trim())),
            },
        })?;
    }
    Ok(())
}

/// Changes the branch's request and reads it again. Blocking.
pub fn update(root: &Path, branch: &str, kind: Option<ForgeKind>, patch: &PrPatch) -> Result<PrState, String> {
    let cli = kind.and_then(Cli::for_forge).ok_or_else(|| tr(Msg::PrUnsupported))?;
    run_calls(cli, root, &update_calls(cli, patch))?;
    Ok(view(root, branch, kind, true))
}

/// Opens a request for the branch and reads it. Blocking.
pub fn create(root: &Path, branch: &str, kind: Option<ForgeKind>, new: &NewPr) -> Result<PrState, String> {
    let cli = kind.and_then(Cli::for_forge).ok_or_else(|| tr(Msg::PrUnsupported))?;
    if new.title.trim().is_empty() {
        return Err(tr(Msg::PrNeedsTitle));
    }
    run_calls(cli, root, &[create_call(cli, new)])?;
    Ok(view(root, branch, kind, true))
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub found: bool,
    /// What it says of itself, as it says it.
    pub version: Option<String>,
    /// None for git, which has nowhere to sign in to.
    pub signed_in: Option<bool>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub git: Tool,
    pub gh: Tool,
    pub glab: Tool,
}

fn probe(binary: &str, version_args: &[&str], auth_args: Option<&[&str]>) -> Tool {
    let Some(path) = find_binary(binary) else {
        return Tool::default();
    };
    let quiet = |args: &[&str]| {
        let mut command = Command::new(&path);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).env("NO_COLOR", "1");
        no_window(&mut command);
        command.output().ok()
    };
    let version = quiet(version_args)
        .and_then(|output| String::from_utf8_lossy(&output.stdout).lines().next().map(|line| line.trim().to_string()))
        .filter(|line| !line.is_empty());
    let signed_in = auth_args.map(|args| quiet(args).is_some_and(|output| output.status.success()));
    Tool { found: true, version, signed_in }
}

/// What's installed of what all this leans on, for Preferences. Blocking.
pub fn tools() -> Tools {
    Tools {
        git: probe("git", &["--version"], None),
        gh: probe("gh", &["--version"], Some(&["auth", "token"])),
        glab: probe("glab", &["--version"], Some(&["auth", "status"])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_a_github_pull_request() {
        let body = json!({
            "number": 7, "title": "Git panel", "body": "Adds **git**.", "state": "OPEN", "isDraft": true,
            "url": "https://github.com/nicoriff/termo/pull/7", "baseRefName": "main", "headRefName": "git-panel",
            "author": { "login": "nicoriff" },
            "labels": [{ "name": "feature", "color": "a2eeef" }, { "name": "odd", "color": "red;x" }],
            "assignees": [{ "login": "nicoriff" }],
            "reviewRequests": [{ "__typename": "User", "login": "ana" }, { "__typename": "Team", "name": "Core", "slug": "core" }],
            "reviewDecision": "REVIEW_REQUIRED", "mergeable": "CONFLICTING",
            "statusCheckRollup": [
                { "__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS" },
                { "__typename": "CheckRun", "status": "IN_PROGRESS", "conclusion": "" },
                { "__typename": "CheckRun", "status": "COMPLETED", "conclusion": "FAILURE" },
                { "__typename": "StatusContext", "state": "SUCCESS" }
            ],
            "additions": 120, "deletions": 8, "changedFiles": 5, "comments": [{}, {}], "updatedAt": "2026-10-12T10:00:00Z"
        });
        let pr = parse_gh(&body).unwrap();
        assert_eq!(pr.number, 7);
        assert_eq!(pr.state, PrStatus::Draft);
        assert_eq!(pr.author, "nicoriff");
        assert_eq!(pr.labels[0].color.as_deref(), Some("a2eeef"));
        assert_eq!(pr.labels[1].color, None);
        assert_eq!(pr.reviewers, ["ana", "core"]);
        assert_eq!(pr.review_decision, Some(ReviewDecision::ReviewRequired));
        assert_eq!(pr.mergeable, Some(false));
        assert_eq!(pr.checks, Some(PrChecks { passed: 2, failed: 1, pending: 1 }));
        assert_eq!((pr.additions, pr.deletions, pr.changed_files, pr.comments), (Some(120), Some(8), Some(5), 2));
    }

    #[test]
    fn a_merged_pull_request_isnt_a_draft_and_no_checks_means_none() {
        let body = json!({ "number": 1, "state": "MERGED", "isDraft": true, "statusCheckRollup": [] });
        let pr = parse_gh(&body).unwrap();
        assert_eq!(pr.state, PrStatus::Merged);
        assert!(pr.checks.is_none());
        assert!(parse_gh(&json!({ "title": "no number" })).is_none());
    }

    #[test]
    fn reads_a_gitlab_merge_request() {
        let body = json!({
            "iid": 12, "title": "Draft: Git panel", "description": "Adds git.", "state": "opened", "draft": true,
            "web_url": "https://gitlab.com/g/app/-/merge_requests/12", "target_branch": "main", "source_branch": "git-panel",
            "author": { "username": "nico" }, "labels": ["feature"], "assignees": [], "reviewers": [{ "username": "ana" }],
            "has_conflicts": false, "user_notes_count": 3, "changes_count": "1000+", "updated_at": "2026-10-12T10:00:00Z",
            "head_pipeline": { "status": "running" }
        });
        let pr = parse_glab(&body).unwrap();
        assert_eq!((pr.number, pr.state), (12, PrStatus::Draft));
        assert_eq!(pr.body, "Adds git.");
        assert_eq!(pr.reviewers, ["ana"]);
        assert_eq!(pr.mergeable, Some(true));
        assert_eq!(pr.checks, Some(PrChecks { passed: 0, failed: 0, pending: 1 }));
        assert_eq!((pr.changed_files, pr.comments), (Some(1000), 3));
    }

    #[test]
    fn a_failure_is_no_request_no_session_or_its_own_words() {
        let none = classify_failure(Cli::Gh, "no pull requests found for branch \"topic\"");
        assert_eq!(none, PrState::None { cli: Cli::Gh });
        let none = classify_failure(Cli::Glab, "no open merge request available for \"topic\"");
        assert_eq!(none, PrState::None { cli: Cli::Glab });
        let out = classify_failure(Cli::Gh, "To get started with GitHub CLI, please run:  gh auth login");
        assert_eq!(out, PrState::NotSignedIn { cli: Cli::Gh });
        let other = classify_failure(Cli::Gh, "\nGraphQL: Could not resolve to a Repository\nusage: …");
        assert_eq!(other, PrState::Failed { message: "GraphQL: Could not resolve to a Repository".into() });
    }

    #[test]
    fn only_github_and_gitlab_have_a_cli() {
        assert_eq!(Cli::for_forge(ForgeKind::Github), Some(Cli::Gh));
        assert_eq!(Cli::for_forge(ForgeKind::Gitlab), Some(Cli::Glab));
        assert_eq!(Cli::for_forge(ForgeKind::Bitbucket), None);
        assert_eq!(view(Path::new("x"), "main", Some(ForgeKind::Bitbucket), false), PrState::Unsupported);
        assert_eq!(view(Path::new("x"), "main", None, false), PrState::Unsupported);
    }

    #[test]
    fn a_github_edit_never_lets_a_value_pass_for_a_flag() {
        let patch = PrPatch {
            title: Some("--web".into()),
            body: Some("--repo evil/x".into()),
            base: Some("main".into()),
            draft: Some(false),
            add_labels: vec![" bug ".into(), "a,b".into(), String::new()],
            remove_reviewers: vec!["ana".into()],
            ..PrPatch::default()
        };
        let calls = update_calls(Cli::Gh, &patch);
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].args,
            ["pr", "edit", "--title=--web", "--body-file=-", "--base=main", "--add-label=bug", "--remove-reviewer=ana"]
        );
        assert_eq!(calls[0].stdin.as_deref(), Some("--repo evil/x"));
        assert_eq!(calls[1].args, ["pr", "ready"]);
    }

    #[test]
    fn nothing_to_change_runs_nothing() {
        assert!(update_calls(Cli::Gh, &PrPatch::default()).is_empty());
        assert!(update_calls(Cli::Glab, &PrPatch::default()).is_empty());
        let draft_only = PrPatch { draft: Some(true), ..PrPatch::default() };
        assert_eq!(update_calls(Cli::Gh, &draft_only)[0].args, ["pr", "ready", "--undo"]);
        assert_eq!(update_calls(Cli::Glab, &draft_only)[0].args, ["mr", "update", "--draft"]);
    }

    #[test]
    fn a_gitlab_edit_signs_its_reviewers() {
        let patch = PrPatch {
            title: Some("T".into()),
            add_reviewers: vec!["ana".into()],
            remove_reviewers: vec!["leo".into()],
            remove_labels: vec!["wip".into()],
            ..PrPatch::default()
        };
        let calls = update_calls(Cli::Glab, &patch);
        assert_eq!(calls[0].args, ["mr", "update", "--title=T", "--unlabel=wip", "--reviewer=+ana,-leo"]);
    }

    #[test]
    fn opening_one_passes_the_body_where_no_flag_can_be_read() {
        let new = NewPr { title: " Git panel ".into(), body: "--draft".into(), base: Some("main".into()), draft: true };
        let gh = create_call(Cli::Gh, &new);
        assert_eq!(gh.args, ["pr", "create", "--title=Git panel", "--body-file=-", "--base=main", "--draft"]);
        assert_eq!(gh.stdin.as_deref(), Some("--draft"));
        let glab = create_call(Cli::Glab, &new);
        assert_eq!(glab.args, ["mr", "create", "--yes", "--title=Git panel", "--description=--draft", "--target-branch=main", "--draft"]);
        assert!(create(Path::new("x"), "topic", Some(ForgeKind::Github), &NewPr::default()).is_err());
    }
}
