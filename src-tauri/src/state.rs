use std::sync::Arc;

use crate::agents::orchestration::executor::Orchestrator;
use crate::agents::runner::Runner;
use crate::git::ops::GitOps;
use crate::git::review::Reviews;
use crate::keep_awake::KeepAwake;
use crate::marketplace::catalog::CatalogCache;
use crate::pty::{MemorySampler, PtyManager};

#[derive(Default)]
pub struct AppState {
    pub pty: PtyManager,
    pub memory: MemorySampler,
    pub marketplace: CatalogCache,
    pub agents: Arc<Runner>,
    pub orchestrator: Arc<Orchestrator>,
    pub keep_awake: Arc<KeepAwake>,
    /// The git actions under way, by repository (see git/ops.rs).
    pub git: Arc<GitOps>,
    /// The code reviews under way, by repository (see git/review.rs).
    pub git_reviews: Arc<Reviews>,
}
