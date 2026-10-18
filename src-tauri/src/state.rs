use std::sync::Arc;

use crate::agents::orchestration::executor::Orchestrator;
use crate::agents::runner::Runner;
use crate::git::ops::GitOps;
use crate::git::review::Reviews;
use crate::harness::assist::Asks;
use crate::keep_awake::KeepAwake;
use crate::marketplace::catalog::CatalogCache;
use crate::pty::{Consoles, MemorySampler};

#[derive(Default)]
pub struct AppState {
    pub pty: Consoles,
    pub memory: MemorySampler,
    pub marketplace: CatalogCache,
    pub agents: Arc<Runner>,
    pub orchestrator: Arc<Orchestrator>,
    pub keep_awake: Arc<KeepAwake>,
    /// The git actions under way, by repository (see git/ops.rs).
    pub git: Arc<GitOps>,
    /// The code reviews under way, by repository (see git/review.rs).
    pub git_reviews: Arc<Reviews>,
    /// The Harness panel's questions to an agent under way, by repository
    /// (see harness/assist.rs).
    pub harness_asks: Arc<Asks>,
    /// What a notification Termo was started from was about (see
    /// agents/host/notify.rs), until the main window takes it.
    pub opened_by: std::sync::Mutex<Option<serde_json::Value>>,
}
