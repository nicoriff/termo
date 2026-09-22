//! Assembling the catalog once and keeping it around.
//!
//! Building it shells out to the `claude` CLI and reads every marketplace
//! manifest on disk, which is far too slow to do each time the panel opens.
//! So it's cached twice: in memory for the session, and on disk so the very
//! first open after a launch is instant too. Refreshing is always explicit —
//! a button in the panel, or a source being added or removed.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use super::claude_plugins::{self, SourceInfo};
use super::model::{AgentTarget, CatalogItem};

/// How long a cached catalog is served without asking again. Marketplaces
/// change on the order of days; a stale entry costs nothing worse than a
/// plugin missing from the list until the next refresh.
const TTL_SECONDS: u64 = 6 * 60 * 60;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub items: Vec<CatalogItem>,
    pub sources: Vec<SourceInfo>,
    /// Machine-readable reasons the catalog is less than complete, for the
    /// panel to translate — today only "claude-cli-missing".
    pub warnings: Vec<String>,
    /// Unix seconds, so the panel can say how old the list is.
    pub fetched_at: u64,
}

#[derive(Default)]
pub struct CatalogCache(pub(super) Mutex<Option<CatalogSnapshot>>);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn is_fresh(snapshot: &CatalogSnapshot) -> bool {
    now().saturating_sub(snapshot.fetched_at) < TTL_SECONDS
}

fn cache_file(app: &tauri::AppHandle) -> anyhow::Result<std::path::PathBuf> {
    Ok(app.path().app_data_dir()?.join("marketplace").join("catalog.json"))
}

fn read_disk_cache(app: &tauri::AppHandle) -> Option<CatalogSnapshot> {
    let file = cache_file(app).ok()?;
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(&text).ok()
}

/// Written to a sibling temp file first, so a crash mid-write can't leave a
/// half-serialized catalog that fails to parse on the next launch.
fn write_disk_cache(app: &tauri::AppHandle, snapshot: &CatalogSnapshot) {
    let Ok(file) = cache_file(app) else { return };
    let Ok(text) = serde_json::to_string(snapshot) else { return };
    if let Some(dir) = file.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    let partial = file.with_extension("json.tmp");
    if std::fs::write(&partial, text).is_ok() {
        let _ = std::fs::rename(&partial, &file);
    }
}

fn build(claude_override: Option<&str>) -> CatalogSnapshot {
    let manifests = claude_plugins::read_disk_manifests();
    let mut warnings = Vec::new();

    let items = match super::agent_cli::locate(AgentTarget::Claude, claude_override) {
        Some(claude) => match claude_plugins::fetch_catalog(&claude) {
            Ok(cli) => claude_plugins::merge_catalog(&cli, &manifests.entries),
            // The CLI is there but wouldn't answer: fall back to disk rather
            // than showing an empty panel, and say why it may be short.
            Err(_) => {
                warnings.push("claude-cli-failed".to_string());
                claude_plugins::catalog_from_disk(&manifests)
            }
        },
        None => {
            warnings.push("claude-cli-missing".to_string());
            claude_plugins::catalog_from_disk(&manifests)
        }
    };

    CatalogSnapshot {
        items,
        sources: manifests.sources,
        warnings,
        fetched_at: now(),
    }
}

/// The in-memory catalog, when there is one and it's still fresh. Separate
/// from `build_and_store` because managed state can't be held across an
/// await, so the command checks this first and only offloads a real build.
pub fn cached(cache: &CatalogCache) -> Option<CatalogSnapshot> {
    cache
        .0
        .lock()
        .ok()
        .and_then(|held| held.clone())
        .filter(is_fresh)
}

fn store(app: &tauri::AppHandle, snapshot: &CatalogSnapshot) {
    if let Some(state) = app.try_state::<crate::state::AppState>() {
        if let Ok(mut held) = state.marketplace.0.lock() {
            *held = Some(snapshot.clone());
        }
    }
}

/// The catalog from the disk cache when it's fresh enough, otherwise built
/// from scratch. `refresh` skips both caches. Blocking: it shells out to the
/// CLI and reads every marketplace manifest.
pub fn build_and_store(app: &tauri::AppHandle, refresh: bool) -> CatalogSnapshot {
    if !refresh {
        if let Some(snapshot) = read_disk_cache(app).filter(is_fresh) {
            store(app, &snapshot);
            return snapshot;
        }
    }

    let snapshot = build(None);
    store(app, &snapshot);
    write_disk_cache(app, &snapshot);
    snapshot
}

/// Drops the in-memory copy so the next load rebuilds — for after a source
/// is added, removed or updated.
pub fn invalidate(cache: &CatalogCache) {
    if let Ok(mut held) = cache.0.lock() {
        *held = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_at(fetched_at: u64) -> CatalogSnapshot {
        CatalogSnapshot {
            items: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
            fetched_at,
        }
    }

    #[test]
    fn a_catalog_older_than_the_ttl_is_not_served() {
        assert!(is_fresh(&snapshot_at(now())));
        assert!(!is_fresh(&snapshot_at(now().saturating_sub(TTL_SECONDS + 1))));
    }

    #[test]
    fn a_snapshot_round_trips_through_the_disk_cache_format() {
        let text = serde_json::to_string(&snapshot_at(1_700_000_000)).unwrap();
        let back: CatalogSnapshot = serde_json::from_str(&text).unwrap();
        assert_eq!(back.fetched_at, 1_700_000_000);
    }
}
