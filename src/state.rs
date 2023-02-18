use std::ops::Deref;
use std::sync::Arc;

use crate::{checksum::Checksummer, config::Settings, key_manager::KeyManager};
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

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum ConsulWatch {
    Create(String, DateTime<Utc>),
    Destroy(String, DateTime<Utc>),
}

#[derive(Clone)]
pub struct AppState(pub Arc<InnerState>);

pub struct InnerState {
    pub settings: Settings,
    pub key_manager: Box<dyn KeyManager>,
    pub consul_settings: ConsulClientSettings,
    pub tasker: Tasker,
    pub tx: Sender<Work>,
    pub consul_manager_tx: Sender<ConsulWatch>,
    pub checksummer: Box<dyn Checksummer>,
}

impl InnerState {
    pub fn new(
        settings: Settings,
        key_manager: Box<dyn KeyManager>,
        consul_settings: ConsulClientSettings,
        tasker: Tasker,
        tx: Sender<Work>,
        consul_manager_tx: Sender<ConsulWatch>,
        checksummer: Box<dyn Checksummer>,
    ) -> Self {
        Self {
            settings,
            key_manager,
            consul_settings,
            tasker,
            tx,
            consul_manager_tx,
            checksummer,
        }
    }
}

impl Deref for AppState {
    type Target = InnerState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
