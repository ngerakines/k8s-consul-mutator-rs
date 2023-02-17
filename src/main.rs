use std::{borrow::BorrowMut, env, sync::Arc};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use consulrs::client::ConsulClientSettingsBuilder;
use std::net::SocketAddr;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use tokio::signal;
use tracing::{error, info};

mod api;
mod config;
mod consul;
mod error;
mod k8s;
mod key_manager;
mod state;
mod updater;

use api::build_router;
use error::Result;
use key_manager::{KeyManager, MemoryKeyManager, NullKeyManager};

use tokio_tasker::Tasker;

use crate::{
    config::SettingsBuilder,
    consul::watch_dispatcher,
    k8s::deployment_watch,
    state::{ConsulWatch, Work},
    updater::update_loop,
};

#[tokio::main]
async fn main() -> Result<()> {
    let version = option_env!("GIT_HASH").unwrap_or(env!("CARGO_PKG_VERSION", "develop"));

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let secure_port = env::var("SECURE_PORT").unwrap_or_else(|_| "8443".to_string());

    let cerificate = env::var("CERTIFICATE").ok();
    let cerificate_key = env::var("CERTIFICATE_KEY").ok();
    let start_secure_server =
        secure_port != "0" && cerificate.is_some() && cerificate_key.is_some();
    let start_insecure_server = port != "0";

    if !start_secure_server && !start_insecure_server {
        panic!("one or both of PORT and SECURE_PORT must be set to a non-zero value");
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "k8s_consul_mutator_rs=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let tasker = Tasker::new();

    let key_manager_type = env::var("KEY_MANAGER_TYPE").unwrap_or_else(|_| "MEMORY".to_owned());
    let key_manager = match key_manager_type.as_str() {
        "MEMORY" => Box::<MemoryKeyManager>::default() as Box<dyn KeyManager>,
        _ => Box::<NullKeyManager>::default() as Box<dyn KeyManager>,
    };

    let settings_builder = SettingsBuilder::default();

    let consul_config_builder = ConsulClientSettingsBuilder::default();

    info!(
        "consul config {:#?}",
        consul_config_builder.build().unwrap()
    );

    let (shutdown_tx, _) = broadcast::channel(1);

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

    {
        let (updater_tx, mut updater_rx) = mpsc::channel::<Work>(100);
        let (watch_dispatcher_tx, mut watch_dispatcher_rx) = mpsc::channel::<ConsulWatch>(100);

        let state_tasker = tasker.clone();
        let shared_state = state::AppState(Arc::new(state::InnerState::new(
            settings_builder.build().unwrap(),
            version.to_string(),
            key_manager,
            consul_config_builder.build().unwrap(),
            state_tasker.clone(),
            updater_tx.clone(),
            watch_dispatcher_tx.clone(),
        )));

        {
            let update_loop_stopper = tasker.stopper();
            let update_loop_shared_state = shared_state.clone();

            tasker.spawn(async move {
                let update_loop_rx = updater_rx.borrow_mut();
                update_loop(
                    update_loop_shared_state,
                    update_loop_stopper,
                    update_loop_rx,
                )
                .await
            });
        }

        {
            let watch_dispatcher_stopper = tasker.stopper();
            let watch_dispatcher_shared_state = shared_state.clone();

            tasker.spawn(async move {
                let inner_rx = watch_dispatcher_rx.borrow_mut();
                watch_dispatcher(
                    watch_dispatcher_stopper,
                    watch_dispatcher_shared_state,
                    inner_rx,
                )
                .await
            });
        }

        {
            let deployment_watcher_stopper = tasker.stopper();
            let deployment_watcher_state = shared_state.clone();

            tasker.spawn(async move {
                let hmmm = deployment_watcher_state.clone();
                let watch = deployment_watch(hmmm, deployment_watcher_stopper).await;
                if let Err(err) = watch {
                    error!("deployment watch failed: {}", err);
                }
            });
        }

        let app = build_router(shared_state.clone());

        if start_insecure_server {
            let mut insecure_notify = shutdown_tx.subscribe();
            let insecure_app = app.clone();
            tasker.spawn(async move {
                info!("insecure server starting");
                axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
                    .serve(insecure_app.into_make_service())
                    .with_graceful_shutdown(async move {
                        info!("insecure server waiting on shutdown");
                        insecure_notify.recv().await.unwrap();
                        info!("insecure server shutdown");
                    })
                    .await
                    .unwrap();
            });
        }

        if start_secure_server {
            info!("secure server starting");

            let tls_config =
                RustlsConfig::from_pem_file(cerificate.unwrap(), cerificate_key.unwrap())
                    .await
                    .unwrap();

            let addr = SocketAddr::from(([0, 0, 0, 0], secure_port.parse::<u16>().unwrap()));

            let handle = Handle::new();

            let mut secure_notify = shutdown_tx.subscribe();
            let shutdown_handler = handle.clone();
            tokio::spawn(async move {
                info!("secure server waiting on shutdown");
                secure_notify.recv().await.unwrap();
                handle.shutdown();
                info!("secure server shutdown");
            });

            let secure_app = app.clone();
            tasker.spawn(async move {
                axum_server::bind_rustls(addr, tls_config)
                    .handle(shutdown_handler)
                    .serve(secure_app.into_make_service())
                    .await
                    .unwrap();
            });
        }

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        info!("signal received, starting graceful shutdown");

        shutdown_tx.send(true).unwrap();
        state_tasker.finish();
    }

    let signaller = tasker.signaller();
    if signaller.stop() {
        info!("stopping tasks");
    }

    tasker.join().await;

    info!("shutdown complete");

    Ok(())
}
