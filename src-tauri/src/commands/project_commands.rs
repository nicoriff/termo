use serde_json::json;

use crate::agents::host::client::dispatch;
use crate::agents::portable::{self, ImportReport};
use crate::agents::project_ai::{self, GeneratedProject, Improvement};
use crate::agents::projects::{self, Project, Stats, VersionInfo};
use crate::agents::spec::{is_valid_id, AgentSpec, Engine};
use crate::agents::store::{self, RunRecord};
use crate::commands::agent_commands::{root, save_agent};
use crate::i18n::{tr, Msg};

fn project(root: &std::path::Path, id: &str) -> Result<Project, String> {
    projects::get_in(root, id).ok_or_else(|| tr(Msg::ProjectNotFound))
}

fn saved(result: anyhow::Result<Vec<Project>>) -> Result<Vec<Project>, String> {
    result.map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

/// Off the main thread: a window reads it as it starts, and there it would
/// hold up every other call.
#[tauri::command]
pub async fn projects_list(app: tauri::AppHandle) -> Result<Vec<Project>, String> {
    Ok(projects::list_in(&root(&app)?))
}

/// Returns the full list after saving.
#[tauri::command]
pub fn project_save(app: tauri::AppHandle, project: Project) -> Result<Vec<Project>, String> {
    let root = root(&app)?;
    let id = project.id.clone();
    let list = saved(projects::save_in(&root, project))?;
    projects::snapshot(&root, &id);
    Ok(list)
}

/// Deletes a project, and its agents too when `delete_agents`; otherwise
/// they're left outside any project. Returns both lists.
#[tauri::command]
pub async fn project_delete(app: tauri::AppHandle, id: String, delete_agents: bool) -> Result<(Vec<Project>, Vec<AgentSpec>), String> {
    dispatch(app, "project_delete", json!({ "id": id, "delete_agents": delete_agents })).await
}

/// Moves an agent into a project (or out of any, with an empty id).
#[tauri::command]
pub fn agent_move(app: tauri::AppHandle, agent_id: String, project_id: String) -> Result<Vec<AgentSpec>, String> {
    let root = root(&app)?;
    let mut spec = store::get_in(&root, &agent_id).ok_or_else(|| tr(Msg::AgentSaveFailed("agent not found")))?;
    if !project_id.is_empty() {
        project(&root, &project_id)?;
    }
    if let Some(mut old) = projects::get_in(&root, &spec.project_id).filter(|old| old.entry_agent_id == agent_id) {
        old.entry_agent_id = String::new();
        let _ = projects::save_in(&root, old);
    }
    spec.project_id = project_id;
    save_agent(&root, spec)
}

/// Pauses or turns on every agent of a project.
#[tauri::command]
pub fn project_set_enabled(app: tauri::AppHandle, id: String, enabled: bool) -> Result<Vec<AgentSpec>, String> {
    let root = root(&app)?;
    for mut spec in projects::agents_of(&root, &id).into_iter().filter(|spec| spec.enabled != enabled) {
        spec.enabled = enabled;
        save_agent(&root, spec)?;
    }
    Ok(store::list_in(&root))
}

/// Starts the project's entry agent; a rehearsal runs the whole
/// conversation it sets off as "only look".
#[tauri::command]
pub async fn project_run(app: tauri::AppHandle, id: String, rehearsal: bool) -> Result<RunRecord, String> {
    dispatch(app, "project_run", json!({ "id": id, "rehearsal": rehearsal })).await
}

/// Stops every run of the project's agents.
#[tauri::command]
pub async fn project_stop(app: tauri::AppHandle, id: String) -> Result<(), String> {
    dispatch(app, "project_stop", json!({ "id": id })).await
}

#[tauri::command]
pub fn project_stats(app: tauri::AppHandle, id: String) -> Result<Stats, String> {
    let root = root(&app)?;
    Ok(projects::stats(&root, &project(&root, &id)?, store::now_millis()))
}

/// The latest runs of the project's agents, newest first.
#[tauri::command]
pub fn project_runs(app: tauri::AppHandle, id: String, limit: usize) -> Result<Vec<RunRecord>, String> {
    Ok(projects::runs_of(&root(&app)?, &id).into_iter().take(limit).collect())
}

#[tauri::command]
pub fn project_export(app: tauri::AppHandle, id: String, path: String) -> Result<(), String> {
    let root = root(&app)?;
    let project = project(&root, &id)?;
    let text = portable::export_project(&project, &projects::agents_of(&root, &id), store::now_seconds());
    std::fs::write(&path, text).map_err(|err| tr(Msg::AgentFileFailed(&path, &err.to_string())))
}

/// Saves the project in the file as a new one, with new agents.
#[tauri::command]
pub fn project_import(
    app: tauri::AppHandle,
    path: String,
) -> Result<(Vec<Project>, Vec<AgentSpec>, ImportReport), String> {
    let root = root(&app)?;
    let text = std::fs::read_to_string(&path).map_err(|err| tr(Msg::AgentFileFailed(&path, &err.to_string())))?;
    let agent_names: Vec<String> = store::list_in(&root).into_iter().map(|spec| spec.name).collect();
    let project_names: Vec<String> = projects::list_in(&root).into_iter().map(|project| project.name).collect();
    let (project, agents, report) =
        portable::import_project(&text, &agent_names, &project_names, || uuid::Uuid::new_v4().to_string())?;
    let id = project.id.clone();
    saved(projects::save_in(&root, project))?;
    for spec in agents {
        save_agent(&root, spec)?;
    }
    projects::snapshot(&root, &id);
    Ok((projects::list_in(&root), store::list_in(&root), report))
}

/// A team drafted with AI from a description, not saved yet.
#[tauri::command]
pub async fn project_draft(
    app: tauri::AppHandle,
    description: String,
    work_dir: String,
    engine: Option<Engine>,
) -> Result<GeneratedProject, String> {
    let scratch = root(&app)?.join(".draft");
    let engine = engine.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || project_ai::generate(&scratch, engine, &description, &work_dir))
        .await
        .map_err(|err| tr(Msg::ProjectDraftFailed(&err.to_string())))?
}

/// Saves a project and its agents as reviewed (a generated one, or one
/// from a template).
#[tauri::command]
pub fn project_create(
    app: tauri::AppHandle,
    project: Project,
    agents: Vec<AgentSpec>,
) -> Result<(Vec<Project>, Vec<AgentSpec>), String> {
    let root = root(&app)?;
    if !is_valid_id(&project.id) {
        return Err(tr(Msg::AgentSaveFailed("invalid project id")));
    }
    let id = project.id.clone();
    saved(projects::save_in(&root, project))?;
    for mut spec in agents {
        spec.project_id = id.clone();
        save_agent(&root, spec)?;
    }
    Ok((projects::list_in(&root), store::list_in(&root)))
}

/// Changes an engine suggests from how the project's runs went.
#[tauri::command]
pub async fn project_improve(app: tauri::AppHandle, id: String, engine: Option<Engine>) -> Result<Improvement, String> {
    let root = root(&app)?;
    let project = project(&root, &id)?;
    let agents = projects::agents_of(&root, &id);
    let runs = projects::runs_of(&root, &id);
    let scratch = root.join(".draft");
    let engine = engine.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || project_ai::improve(&scratch, engine, &project, &agents, &runs))
        .await
        .map_err(|err| tr(Msg::ProjectImproveFailed(&err.to_string())))?
}

/// Newest first.
#[tauri::command]
pub fn project_versions(app: tauri::AppHandle, id: String) -> Result<Vec<VersionInfo>, String> {
    Ok(projects::versions(&root(&app)?, &id))
}

/// Puts the project back as it was: itself and the agents it had then
/// (recreated if deleted since). Agents added since are left outside it,
/// not deleted. The restore is itself a new version.
#[tauri::command]
pub fn project_restore(app: tauri::AppHandle, id: String, at: i64) -> Result<(Vec<Project>, Vec<AgentSpec>), String> {
    let root = root(&app)?;
    let snapshot = projects::load_version(&root, &id, at).ok_or_else(|| tr(Msg::ProjectVersionMissing))?;
    let kept: Vec<&str> = snapshot.agents.iter().map(|spec| spec.id.as_str()).collect();
    for mut spec in projects::agents_of(&root, &id).into_iter().filter(|spec| !kept.contains(&spec.id.as_str())) {
        spec.project_id = String::new();
        save_agent(&root, spec)?;
    }
    saved(projects::save_in(&root, snapshot.project.clone()))?;
    for spec in snapshot.agents {
        save_agent(&root, spec)?;
    }
    projects::snapshot(&root, &id);
    Ok((projects::list_in(&root), store::list_in(&root)))
}
