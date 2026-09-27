use tauri::State;

use crate::agents::events::RunEvent;
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
    store::save_in(&root(&app)?, spec).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

#[tauri::command]
pub fn agents_delete(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<Vec<AgentSpec>, String> {
    state.agents.stop(&id);
    store::delete_in(&root(&app)?, &id).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
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
    state.agents.start(&app, &spec, trigger)
}

#[tauri::command]
pub fn agent_stop(state: State<'_, AppState>, agent_id: String) {
    state.agents.stop(&agent_id);
}

#[tauri::command]
pub fn agents_running(state: State<'_, AppState>) -> Vec<RunRecord> {
    state.agents.running()
}

/// Newest first.
#[tauri::command]
pub fn agent_runs(app: tauri::AppHandle, agent_id: String) -> Result<Vec<RunRecord>, String> {
    Ok(store::list_runs_in(&root(&app)?, &agent_id))
}

#[tauri::command]
pub fn agent_run_log(app: tauri::AppHandle, agent_id: String, run_id: String) -> Result<Vec<RunEvent>, String> {
    Ok(store::read_log_in(&root(&app)?, &agent_id, &run_id))
}
