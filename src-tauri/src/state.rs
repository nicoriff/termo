use std::sync::Arc;

use crate::agents::runner::Runner;
use crate::marketplace::catalog::CatalogCache;
use crate::pty::{MemorySampler, PtyManager};

#[derive(Default)]
pub struct AppState {
    pub pty: PtyManager,
    pub memory: MemorySampler,
    pub marketplace: CatalogCache,
    pub agents: Arc<Runner>,
}
