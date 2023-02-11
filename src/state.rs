use std::ops::Deref;
use std::sync::Arc;

use crate::key_manager::{KeyManager, NullKeyManager};
use tokio_tasker::Tasker;

#[derive(Clone)]
pub struct AppState(pub Arc<InnerState>);

pub struct InnerState {
    pub version: String,
    pub key_manager: Box<dyn KeyManager>,
    pub tasker: Tasker,
}

impl Default for InnerState {
    fn default() -> Self {
        InnerState {
            version: "default".to_string(),
            key_manager: Box::new(NullKeyManager::default()) as Box<dyn KeyManager>,
            tasker: Tasker::new(),
        }
    }
}

impl InnerState {
    pub fn new(version: String, key_manager: Box<dyn KeyManager>, tasker: Tasker) -> Self {
        Self {
            version,
            key_manager,
            tasker,
        }
    }
}

impl Deref for AppState {
    type Target = InnerState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
