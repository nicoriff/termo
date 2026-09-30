use tauri::State;

use crate::marketplace::catalog::{self, CatalogSnapshot};
use crate::marketplace::install::{self, InstallOutcome, InstallRequest, McpInstallRequest};
use crate::marketplace::mcp_registry::{self, McpPage};
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

/// One page of the MCP registry. Network-bound and unbounded (unlike the
/// Claude catalog), so it isn't cached here — the frontend re-queries as the
/// search box changes, same as any other server-side search.
#[tauri::command]
pub async fn search_mcp_registry(query: String, cursor: Option<String>) -> Result<McpPage, String> {
    tauri::async_runtime::spawn_blocking(move || mcp_registry::search(&query, cursor.as_deref()))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn install_mcp_item(request: McpInstallRequest) -> Result<InstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || install::install_mcp(&request))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn uninstall_mcp_item(request: McpInstallRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install::uninstall_mcp(&request))
        .await
        .map_err(|err| err.to_string())?
}
