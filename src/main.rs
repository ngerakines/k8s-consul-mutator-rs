use axum_server::{tls_rustls::RustlsConfig, Handle};
use consulrs::client::ConsulClientSettingsBuilder;
use std::net::SocketAddr;
use std::{borrow::BorrowMut, sync::Arc};
use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tokio_tasker::Tasker;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod checksum;
mod config;
mod consul;
mod error;
mod k8s;
mod key_manager;
mod state;
mod updater;

use api::build_router;
use error::Result;

use crate::{
    checksum::get_checksummer,
    config::SettingsBuilder,
    consul::watch_dispatcher,
    k8s::deployment_watch,
    key_manager::get_key_manager,
    state::{ConsulWatch, DeploymentUpdate},
    updater::update_loop,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "k8s_consul_mutator_rs=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    #[cfg(debug_assertions)]
    warn!("Debug assertions enabled");

    let settings_builder = SettingsBuilder::default();
    let settings = settings_builder.build().unwrap();

    if !settings.is_secure_enabled() && !settings.is_secure_enabled() {
        panic!("one or both of PORT and SECURE_PORT must be set to a non-zero value");
    }

    let key_manager = get_key_manager(&settings.key_manager_type);

    let checksummer = get_checksummer(&settings.checksum_type);

    let consul_config_builder = ConsulClientSettingsBuilder::default();

    let tasker = Tasker::new();

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
        let (updater_tx, mut updater_rx) = mpsc::channel::<DeploymentUpdate>(100);
        let (watch_dispatcher_tx, mut watch_dispatcher_rx) = mpsc::channel::<ConsulWatch>(100);

        let state_tasker = tasker.clone();
        let shared_state = state::AppState(Arc::new(state::InnerState::new(
            settings_builder.build().unwrap(),
            key_manager,
            consul_config_builder.build().unwrap(),
            state_tasker.clone(),
            updater_tx.clone(),
            watch_dispatcher_tx.clone(),
            checksummer,
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

        if settings.is_insecure_enabled() {
            let mut insecure_notify = shutdown_tx.subscribe();
            let insecure_app = app.clone();
            tasker.spawn(async move {
                info!("insecure server starting");
                axum::Server::bind(&format!("0.0.0.0:{}", settings.port).parse().unwrap())
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

        if settings.is_secure_enabled() {
            info!("secure server starting");

            let tls_config =
                RustlsConfig::from_pem_file(settings.certificate, settings.certificate_key)
                    .await
                    .unwrap();

            let addr = SocketAddr::from(([0, 0, 0, 0], settings.secure_port));

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
