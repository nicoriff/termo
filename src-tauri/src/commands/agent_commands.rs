use tauri::State;

use crate::agents::draft::{self, Draft};
use crate::agents::events::RunEvent;
use crate::agents::git_hooks;
use crate::agents::github::{self, GithubPreview};
use crate::agents::webhook;
use crate::agents::integrations::{self, AgentMcpRequest, AgentMcpServer, CustomMcpRequest};
use crate::agents::scheduler;
use crate::agents::portable::{self, ImportReport};
use crate::agents::secrets;
use crate::agents::spec::AgentSpec;
use crate::agents::store::{self, RunRecord, RunTrigger};
use crate::i18n::{tr, Msg};
use crate::state::AppState;

fn root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    store::root(app).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

#[tauri::command]
pub fn agents_list(app: tauri::AppHandle) -> Result<Vec<AgentSpec>, String> {
    Ok(store::list_in(&root(&app)?))
}

/// Returns the full list after saving, so the caller refreshes in one call.
#[tauri::command]
pub fn agents_save(app: tauri::AppHandle, spec: AgentSpec) -> Result<Vec<AgentSpec>, String> {
    let root = root(&app)?;
    // The hook first: if it can't go in, nothing is saved half-done.
    git_hooks::sync(store::get_in(&root, &spec.id).as_ref(), Some(&spec))?;
    store::save_in(&root, spec).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

#[tauri::command]
pub fn agents_delete(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<Vec<AgentSpec>, String> {
    state.agents.stop(&id);
    let root = root(&app)?;
    if let Some(spec) = store::get_in(&root, &id) {
        secrets::delete_all(&id, &spec.secrets);
        let _ = git_hooks::sync(Some(&spec), None);
    }
    store::delete_in(&root, &id).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

/// Runs the spec as given rather than the saved one, so the wizard can try
/// out an agent before it's saved.
#[tauri::command]
pub fn agent_run(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    spec: AgentSpec,
    trigger: RunTrigger,
) -> Result<RunRecord, String> {
    state.agents.start(&app, &spec, trigger, None)
}

#[tauri::command]
pub fn agent_stop(state: State<'_, AppState>, agent_id: String) {
    state.agents.stop(&agent_id);
}

#[tauri::command]
pub fn agents_running(state: State<'_, AppState>) -> Vec<RunRecord> {
    state.agents.running()
}

/// When each scheduled agent runs next on its own (Unix milliseconds).
#[tauri::command]
pub fn agents_next_runs(app: tauri::AppHandle) -> Result<std::collections::HashMap<String, i64>, String> {
    let root = root(&app)?;
    Ok(store::list_in(&root)
        .iter()
        .filter_map(|spec| scheduler::agent_next_run(&root, spec).map(|at| (spec.id.clone(), at)))
        .collect())
}

/// Newest first.
#[tauri::command]
pub fn agent_runs(app: tauri::AppHandle, agent_id: String) -> Result<Vec<RunRecord>, String> {
    Ok(store::list_runs_in(&root(&app)?, &agent_id))
}

/// "Build it with AI": a draft of the whole agent from a description.
#[tauri::command]
pub async fn agent_draft(app: tauri::AppHandle, description: String, work_dir: String) -> Result<Draft, String> {
    let scratch = root(&app)?.join(".draft");
    tauri::async_runtime::spawn_blocking(move || draft::draft(&scratch, &description, &work_dir))
        .await
        .map_err(|err| tr(Msg::AgentDraftFailed(&err.to_string())))?
}

/// An MCP server from the registry, ready to add to the agent, with its
/// secrets already in the keychain.
#[tauri::command]
pub fn agent_mcp_server(request: AgentMcpRequest) -> Result<AgentMcpServer, String> {
    integrations::build(request)
}

/// Drops secrets the agent no longer uses (an integration removed, a new
/// agent cancelled).
#[tauri::command]
pub fn agent_forget_secrets(agent_id: String, names: Vec<String>) {
    secrets::delete_all(&agent_id, &names);
}

/// What deploying to GitHub Actions would write, without writing it.
#[tauri::command]
pub fn agent_github_preview(spec: AgentSpec) -> Result<GithubPreview, String> {
    github::preview(&spec)
}

/// Writes the agent's workflow into its repository; returns the file's path.
#[tauri::command]
pub fn agent_github_write(spec: AgentSpec) -> Result<String, String> {
    github::write(&spec)
}

/// A connector of the user's own, ready to add to the agent.
#[tauri::command]
pub fn agent_custom_mcp(request: CustomMcpRequest) -> Result<AgentMcpServer, String> {
    integrations::build_custom(request)
}

/// Last month's downloads of an MCP server's package (npm or PyPI), when
/// its registry says.
#[tauri::command]
pub async fn mcp_downloads(registry_type: String, identifier: String) -> Option<u64> {
    tauri::async_runtime::spawn_blocking(move || crate::marketplace::downloads::last_month(&registry_type, &identifier))
        .await
        .ok()
        .flatten()
}

/// Writes the agents with these ids to a file.
#[tauri::command]
pub fn agents_export(app: tauri::AppHandle, ids: Vec<String>, path: String) -> Result<(), String> {
    let root = root(&app)?;
    let specs: Vec<AgentSpec> = ids.iter().filter_map(|id| store::get_in(&root, id)).collect();
    std::fs::write(&path, portable::export(&specs, store::now_seconds()))
        .map_err(|err| tr(Msg::AgentFileFailed(&path, &err.to_string())))
}

/// Saves the agents in these files as new ones; returns the full list and
/// what the import couldn't bring along.
#[tauri::command]
pub fn agents_import(app: tauri::AppHandle, paths: Vec<String>) -> Result<(Vec<AgentSpec>, ImportReport), String> {
    let root = root(&app)?;
    let mut report = ImportReport { imported: vec![], moved_to_own_folder: vec![], needs_secrets: vec![], review: vec![] };
    for path in &paths {
        let text = std::fs::read_to_string(path).map_err(|err| tr(Msg::AgentFileFailed(path, &err.to_string())))?;
        let existing: Vec<String> = store::list_in(&root).into_iter().map(|spec| spec.name).collect();
        let (agents, part) = portable::import(&text, &existing, || uuid::Uuid::new_v4().to_string())?;
        for spec in agents {
            store::save_in(&root, spec).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))?;
        }
        report.imported.extend(part.imported);
        report.moved_to_own_folder.extend(part.moved_to_own_folder);
        report.needs_secrets.extend(part.needs_secrets);
        report.review.extend(part.review);
    }
    Ok((store::list_in(&root), report))
}

/// The local webhook's port, and whether it could take it.
#[tauri::command]
pub fn agents_webhook() -> serde_json::Value {
    serde_json::json!({ "port": webhook::PORT, "listening": webhook::listening() })
}

/// Which of these secrets have no value on this machine.
#[tauri::command]
pub fn agent_missing_secrets(agent_id: String, names: Vec<String>) -> Vec<String> {
    portable::missing_secrets(&agent_id, &names)
}

/// Stores one secret's value in the keychain.
#[tauri::command]
pub fn agent_set_secret(agent_id: String, name: String, value: String) -> Result<(), String> {
    if !crate::agents::spec::is_valid_id(&agent_id) {
        return Err(tr(Msg::AgentSaveFailed("invalid agent id")));
    }
    secrets::set(&agent_id, &name, &value).map_err(|err| tr(Msg::AgentSecretFailed(&err)))
}

#[tauri::command]
pub fn agent_run_log(app: tauri::AppHandle, agent_id: String, run_id: String) -> Result<Vec<RunEvent>, String> {
    Ok(store::read_log_in(&root(&app)?, &agent_id, &run_id))
}
