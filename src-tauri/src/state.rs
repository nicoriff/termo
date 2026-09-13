use crate::pty::{MemorySampler, PtyManager};

#[derive(Default)]
pub struct AppState {
    pub pty: PtyManager,
    pub memory: MemorySampler,
}
