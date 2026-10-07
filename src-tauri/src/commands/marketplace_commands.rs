use tauri::State;

use crate::marketplace::catalog::{self, CatalogSnapshot};
use crate::marketplace::install::{self, InstallOutcome, InstallRequest, McpInstallRequest};
use crate::marketplace::installed::Installed;
use crate::marketplace::mcp_registry::McpPage;
use crate::marketplace::model::CatalogItem;
use crate::marketplace::sources::{self, SourceId, SourceStatus};
use crate::marketplace::{agent_cli, model::AgentTarget, model::MarketplaceStatus};
use crate::state::AppState;

/// Which agent CLIs this machine has, so the panel can say why installing
/// isn't available instead of failing at the moment the user clicks.
#[tauri::command]
pub fn marketplace_status() -> MarketplaceStatus {
    MarketplaceStatus {
        claude_cli: agent_cli::locate(AgentTarget::Claude, None)
            .map(|path| path.to_string_lossy().into_owned()),
        codex_cli: agent_cli::locate(AgentTarget::Codex, None)
            .map(|path| path.to_string_lossy().into_owned()),
        copilot_cli: agent_cli::locate(AgentTarget::Copilot, None)
            .map(|path| path.to_string_lossy().into_owned()),
        gemini_cli: agent_cli::locate(AgentTarget::Gemini, None)
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

/// Building the catalog shells out to `claude` and reads every marketplace
/// manifest, so it runs on the blocking pool rather than holding up the
/// webview's IPC — same as the Oh My Posh downloads.
#[tauri::command]
pub async fn list_catalog(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<CatalogSnapshot, String> {
    // The cache lives in managed state, which isn't Send across the await,
    // so the fast path is checked here and only a real build is offloaded.
    if !refresh {
        if let Some(snapshot) = catalog::cached(&state.marketplace) {
            return Ok(snapshot);
        }
    }
    tauri::async_runtime::spawn_blocking(move || catalog::build_and_store(&app, refresh))
        .await
        .map_err(|err| err.to_string())
}

/// Installs a plugin through `claude plugin install`. Can come back asking
/// for a marketplace-declared command to be approved (see marketplace/
/// install.rs); the UI shows it and calls again with `acceptCommand`.
#[tauri::command]
pub async fn install_item(request: InstallRequest) -> Result<InstallOutcome, String> {
    let outcome = tauri::async_runtime::spawn_blocking(move || install::install_plugin(&request))
        .await
        .map_err(|err| err.to_string())??;
    Ok(outcome)
}

#[tauri::command]
pub async fn uninstall_item(request: InstallRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install::uninstall_plugin(&request))
        .await
        .map_err(|err| err.to_string())?
}

/// Drops the cached catalog so the next load rebuilds it — what the frontend
/// calls after an install, so the "installed" badges are right again.
#[tauri::command]
pub fn invalidate_catalog(state: State<'_, AppState>) {
    catalog::invalidate(&state.marketplace);
}

/// Where Termo notes which catalog entry each server name it installed
/// belongs to (see marketplace/installed.rs).
fn owners_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path().app_data_dir().map(|dir| dir.join("marketplace-installed.json")).map_err(|err| err.to_string())
}

/// The sources MCP servers are searched in, and whether each is ready.
#[tauri::command]
pub async fn marketplace_sources() -> Result<Vec<SourceStatus>, String> {
    tauri::async_runtime::spawn_blocking(sources::statuses).await.map_err(|err| err.to_string())
}

/// Saves a source's API key in the system keychain, once it's seen to work.
#[tauri::command]
pub async fn marketplace_set_key(source: SourceId, key: String) -> Result<SourceStatus, String> {
    tauri::async_runtime::spawn_blocking(move || sources::set_key(source, &key))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn marketplace_clear_key(source: SourceId) -> Result<SourceStatus, String> {
    tauri::async_runtime::spawn_blocking(move || sources::clear_key(source)).await.map_err(|err| err.to_string())
}

/// Whether a source's saved key still works.
#[tauri::command]
pub async fn marketplace_test_key(source: SourceId) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || sources::test(source)).await.map_err(|err| err.to_string())?
}

/// One page of MCP servers from `source`. Network-bound and unbounded
/// (unlike the Claude catalog), so it isn't cached here — the frontend
/// re-queries as the search box changes, same as any other server-side
/// search. Each result says which agents already have it.
#[tauri::command]
pub async fn search_mcp(app: tauri::AppHandle, source: SourceId, query: String, cursor: Option<String>) -> Result<McpPage, String> {
    let owners = owners_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || sources::search(source, &query, cursor.as_deref(), &Installed::read(&owners)))
        .await
        .map_err(|err| err.to_string())?
}

/// A server's details from a source whose results don't carry them.
#[tauri::command]
pub async fn mcp_detail(app: tauri::AppHandle, source: SourceId, name: String) -> Result<CatalogItem, String> {
    let owners = owners_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || sources::detail(source, &name, &Installed::read(&owners)))
        .await
        .map_err(|err| err.to_string())?
}

/// One page of the official MCP registry: what `search_mcp` does for that
/// source, kept for the callers that only ever ask it.
#[tauri::command]
pub async fn search_mcp_registry(app: tauri::AppHandle, query: String, cursor: Option<String>) -> Result<McpPage, String> {
    search_mcp(app, SourceId::Official, query, cursor).await
}

#[tauri::command]
pub async fn install_mcp_item(app: tauri::AppHandle, request: McpInstallRequest) -> Result<InstallOutcome, String> {
    let owners = owners_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || install::install_mcp(&request, &owners))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn uninstall_mcp_item(app: tauri::AppHandle, request: McpInstallRequest) -> Result<(), String> {
    let owners = owners_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || install::uninstall_mcp(&request, &owners))
        .await
        .map_err(|err| err.to_string())?
}
