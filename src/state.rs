use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState(pub Arc<InnerState>);

pub struct InnerState {
    pub version: String,
}

impl InnerState {
    pub fn new(version: String) -> Self {
        Self { version }
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

    #[test]
    fn create_app_staet() {
        let app_state = InnerState::new("1.0.0".to_string());
        assert_eq!(app_state.version, "1.0.0");
    }
}
