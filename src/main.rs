use std::env;
use std::sync::Arc;

use sentry_tracing::EventFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use tokio::signal;
use tokio::time::{sleep, Duration};
use tracing::info;

use tokio_tasker::Tasker;

mod api;
mod consul;
mod error;
mod state;

use api::build_router;
use consul::{KeyManager, MemoryKeyManager, NullKeyManager};
use error::Result;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("signal received, starting graceful shutdown");
}

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
                .unwrap_or_else(|_| "k8s_consul_mutator_rs=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(layer)
        .init();

    let tasker = Tasker::new();

    let key_manager_type = env::var("KEY_MANAGER_TYPE").unwrap_or_else(|_| "MEMORY".to_owned());
    let key_manager = match key_manager_type.as_str() {
        "MEMORY" => Box::<MemoryKeyManager>::default() as Box<dyn KeyManager>,
        _ => Box::<NullKeyManager>::default() as Box<dyn KeyManager>,
    };

    let shared_state = state::AppState(Arc::new(state::InnerState::new(
        version.to_string(),
        key_manager,
    )));

    let local_state = shared_state.clone();

    let loop_tasker = tasker.clone();
    tasker.spawn(async move {
        let mut counter = 1;
        while !loop_tasker.stopper().is_stopped() {
            counter += 1;
            info!("background loop {counter}");
            local_state
                .storage
                .set("loop".to_string(), counter.to_string())
                .await
                .unwrap();
            sleep(Duration::from_secs(5)).await;
        }
        info!("background loop stopped");
        loop_tasker.finish();
    });

    let app = build_router(shared_state.clone());

    let signaller = tasker.signaller();

    axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    if signaller.stop() {
        info!("Stopping background tasks");
    }

    tasker.join().await;

    Ok(())
}
