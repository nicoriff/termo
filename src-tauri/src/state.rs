use crate::pty::PtyManager;

#[derive(Default)]
pub struct AppState {
    pub pty: PtyManager,
}
