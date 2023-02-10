use std::ops::Deref;
use std::sync::Arc;

use crate::consul::KeyManager;

#[derive(Clone)]
pub struct AppState(pub Arc<InnerState>);

pub struct InnerState {
    pub version: String,
    pub storage: Box<dyn KeyManager>,
}

impl InnerState {
    pub fn new(version: String, storage: Box<dyn KeyManager>) -> Self {
        Self { version, storage }
    }
}

impl Deref for AppState {
    type Target = InnerState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consul::NullKeyManager;

    #[test]
    fn create_app_staet() {
        let key_manager = Box::new(NullKeyManager::default()) as Box<dyn KeyManager>;
        let app_state = InnerState::new("1.0.0".to_string(), key_manager);
        assert_eq!(app_state.version, "1.0.0");
    }
}
