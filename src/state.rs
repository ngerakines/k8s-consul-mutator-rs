use std::ops::Deref;
use std::sync::Arc;

use crate::key_manager::KeyManager;
use chrono::{DateTime, Utc};
use consulrs::client::ConsulClientSettings;
use tokio::sync::mpsc::Sender;
use tokio_tasker::Tasker;

/// A subscription is a namespaced resource for a key.
#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub struct Work {
    pub namespace: String,
    pub deployment: String,
    pub occurred: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<InnerState>);

pub struct InnerState {
    pub version: String,
    pub key_manager: Box<dyn KeyManager>,
    pub consul_settings: ConsulClientSettings,
    pub tasker: Tasker,
    pub tx: Sender<Work>,
}

impl InnerState {
    pub fn new(
        version: String,
        key_manager: Box<dyn KeyManager>,
        consul_settings: ConsulClientSettings,
        tasker: Tasker,
        tx: Sender<Work>,
    ) -> Self {
        Self {
            version,
            key_manager,
            consul_settings,
            tasker,
            tx,
        }
    }
}

impl Deref for AppState {
    type Target = InnerState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
