//! Copilot's plan usage, for the status bar: how much of the month's chat
//! and premium requests is used, and when it renews.
//!
//! Copilot CLI's status line doesn't carry it (see pty/copilot_status.rs);
//! GitHub serves it at `copilot_internal/user`, the endpoint Copilot itself
//! and the editors read. It isn't a documented API, so anything unexpected
//! simply means no reading. It's asked with the token the GitHub CLI (`gh`)
//! already holds — Termo never reads Copilot's own credentials — and kept for
//! a few minutes, since a plan's monthly count barely moves.

use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

const ENDPOINT: &str = "https://api.github.com/copilot_internal/user";
const FRESH_FOR: Duration = Duration::from_secs(5 * 60);
/// A failure is asked again sooner: `gh` may have just been signed in to.
const FAILURE_FRESH_FOR: Duration = Duration::from_secs(60);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CopilotQuota {
    /// `premium_interactions` or `chat`.
    pub id: String,
    pub percent_used: f64,
    pub remaining: f64,
    pub entitlement: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CopilotPlanUsage {
    #[serde(rename_all = "camelCase")]
    Ok {
        plan: Option<String>,
        /// Only the limited ones, premium first: an unlimited quota has no
        /// gauge to show.
        quotas: Vec<CopilotQuota>,
        /// `YYYY-MM-DD`.
        resets_on: Option<String>,
    },
    /// No `gh`, or not signed in: nothing to ask GitHub with.
    NoGh,
    Failed,
}

static CACHE: Mutex<Option<(Instant, CopilotPlanUsage)>> = Mutex::new(None);

fn parse(body: &Value) -> CopilotPlanUsage {
    let quotas = ["premium_interactions", "chat"]
        .iter()
        .filter_map(|id| {
            let quota = body.pointer(&format!("/quota_snapshots/{id}"))?;
            let number = |key: &str| quota.get(key).and_then(Value::as_f64);
            let entitlement = number("entitlement")?;
            if quota.get("unlimited").and_then(Value::as_bool) == Some(true) || entitlement <= 0.0 {
                return None;
            }
            let remaining = number("quota_remaining").or_else(|| number("remaining"))?;
            let percent_used = match number("percent_remaining") {
                Some(left) => 100.0 - left,
                None => (entitlement - remaining) / entitlement * 100.0,
            };
            Some(CopilotQuota {
                id: id.to_string(),
                percent_used: percent_used.clamp(0.0, 100.0),
                remaining,
                entitlement,
            })
        })
        .collect();
    let text = |key: &str| body.get(key).and_then(Value::as_str).map(str::to_owned);
    CopilotPlanUsage::Ok {
        plan: text("copilot_plan"),
        quotas,
        resets_on: text("quota_reset_date"),
    }
}

fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// The token `gh` is signed in with (it also honors GH_TOKEN).
fn gh_token() -> Option<String> {
    let gh = crate::pty::bin_search::find_binary("gh")?;
    let mut command = Command::new(gh);
    command.args(["auth", "token"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    no_window(&mut command);
    let output = command.output().ok()?;
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    // A token is one plain word: anything else isn't going in a header.
    (output.status.success() && !token.is_empty() && token.chars().all(|c| c.is_ascii_graphic())).then_some(token)
}

fn fetch() -> CopilotPlanUsage {
    let Some(token) = gh_token() else {
        return CopilotPlanUsage::NoGh;
    };
    let response = ureq::get(ENDPOINT)
        .set("Authorization", &format!("token {token}"))
        .set("Accept", "application/json")
        .set("User-Agent", "Termo")
        .timeout(Duration::from_secs(10))
        .call();
    let body = response.ok().and_then(|response| response.into_string().ok());
    match body.and_then(|text| serde_json::from_str::<Value>(&text).ok()) {
        Some(body) => parse(&body),
        None => CopilotPlanUsage::Failed,
    }
}

#[tauri::command]
pub async fn copilot_plan_usage() -> CopilotPlanUsage {
    if !crate::ai_mode::enabled() {
        return CopilotPlanUsage::Failed;
    }
    tauri::async_runtime::spawn_blocking(|| {
        if let Some((at, usage)) = CACHE.lock().unwrap().as_ref() {
            let fresh_for = if matches!(usage, CopilotPlanUsage::Ok { .. }) { FRESH_FOR } else { FAILURE_FRESH_FOR };
            if at.elapsed() < fresh_for {
                return usage.clone();
            }
        }
        let usage = fetch();
        *CACHE.lock().unwrap() = Some((Instant::now(), usage.clone()));
        usage
    })
    .await
    .unwrap_or(CopilotPlanUsage::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(entitlement: f64, remaining: f64, percent_remaining: f64, unlimited: bool) -> Value {
        serde_json::json!({
            "entitlement": entitlement,
            "quota_remaining": remaining,
            "percent_remaining": percent_remaining,
            "unlimited": unlimited,
        })
    }

    /// A free plan: a limited chat quota, no premium requests at all.
    #[test]
    fn a_free_plan_shows_its_chat_quota() {
        let body = serde_json::json!({
            "copilot_plan": "individual",
            "quota_reset_date": "2026-11-01",
            "quota_snapshots": {
                "chat": snapshot(200.0, 198.6, 99.3, false),
                "completions": snapshot(2000.0, 2000.0, 100.0, false),
                "premium_interactions": snapshot(0.0, 0.0, 0.0, false),
            }
        });
        let CopilotPlanUsage::Ok { plan, quotas, resets_on } = parse(&body) else { panic!() };
        assert_eq!(plan.as_deref(), Some("individual"));
        assert_eq!(resets_on.as_deref(), Some("2026-11-01"));
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].id, "chat");
        assert!((quotas[0].percent_used - 0.7).abs() < 1e-9);
    }

    /// A paid plan: unlimited chat, a monthly premium allowance.
    #[test]
    fn a_paid_plan_shows_its_premium_requests() {
        let body = serde_json::json!({
            "quota_snapshots": {
                "chat": snapshot(0.0, 0.0, 100.0, true),
                "premium_interactions": snapshot(300.0, 225.0, 75.0, false),
            }
        });
        let CopilotPlanUsage::Ok { quotas, .. } = parse(&body) else { panic!() };
        assert_eq!(quotas, vec![CopilotQuota { id: "premium_interactions".into(), percent_used: 25.0, remaining: 225.0, entitlement: 300.0 }]);
    }

    #[test]
    fn an_unexpected_answer_has_no_quotas() {
        let CopilotPlanUsage::Ok { quotas, .. } = parse(&serde_json::json!({ "message": "Not Found" })) else { panic!() };
        assert!(quotas.is_empty());
    }
}
