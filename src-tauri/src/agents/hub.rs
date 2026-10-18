//! What the agents need from whoever runs them: where their files are, and
//! someone to tell what's happening (a run's steps, a question, an
//! orchestration's progress). The app is one (its windows hear it); the
//! agents' own background process is the other (see host/), which passes it
//! on to the app when one is connected.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

/// Hears what the agents say: an event's name, as the windows listen for
/// it, and its payload.
pub trait Sink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

#[derive(Clone)]
pub struct Hub {
    root: PathBuf,
    sink: Arc<dyn Sink>,
}

impl Hub {
    pub fn new(root: PathBuf, sink: Arc<dyn Sink>) -> Self {
        Self { root, sink }
    }

    /// The app's: its windows hear every event.
    pub fn for_app(app: &tauri::AppHandle) -> Result<Self, String> {
        let root = super::store::root(app).map_err(|err| err.to_string())?;
        Ok(Self::new(root, Arc::new(AppSink(app.clone()))))
    }

    /// The agents' folder (see store::root).
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) {
        if let Ok(value) = serde_json::to_value(payload) {
            self.sink.emit(event, value);
        }
    }
}

struct AppSink(tauri::AppHandle);

impl Sink for AppSink {
    fn emit(&self, event: &str, payload: Value) {
        let _ = tauri::Emitter::emit(&self.0, event, payload);
    }
}

/// Hears nothing: for tests, and for work nobody watches.
#[cfg(test)]
pub struct Silent;

#[cfg(test)]
impl Sink for Silent {
    fn emit(&self, _event: &str, _payload: Value) {}
}
