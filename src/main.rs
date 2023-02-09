use std::env;
use std::sync::Arc;

use sentry_tracing::EventFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod error;
mod state;

use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let environment: String = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());

    let version = option_env!("GIT_HASH").unwrap_or(env!("CARGO_PKG_VERSION", "develop"));

    let _guard = sentry::init(sentry::ClientOptions {
        debug: environment != "production",
        environment: Some(environment.clone().into()),
        release: Some(std::borrow::Cow::Borrowed(version)),
        attach_stacktrace: true,
        ..sentry::ClientOptions::default()
    });

    let layer = sentry_tracing::layer().event_filter(|md| match md.level() {
        &tracing::Level::ERROR => EventFilter::Event,
        _ => EventFilter::Ignore,
    });

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "k8s-consul-mutator-rs=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(layer)
        .init();

    let shared_state = state::AppState(Arc::new(state::InnerState::new(version.to_string())));

    api::run(&format!("0.0.0.0:{port}"), shared_state).await
}
