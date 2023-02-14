use std::{borrow::BorrowMut, env, sync::Arc};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use consulrs::client::ConsulClientSettingsBuilder;
use std::net::SocketAddr;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use tokio::signal;
use tracing::{debug, info};

mod api;
mod consul;
mod error;
mod key_manager;
mod state;
mod updater;

use api::build_router;
use error::Result;
use key_manager::{KeyManager, MemoryKeyManager, NullKeyManager};

use tokio_tasker::Tasker;

use crate::{state::Work, updater::update_loop};

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

    let mut consul_config_builder = ConsulClientSettingsBuilder::default();
    if let Ok(consul_address) = env::var("CONSUL_ADDRESS") {
        consul_config_builder.address(consul_address);
    }
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
        let (updater_tx, mut updater_rx) = mpsc::channel::<Work>(5);

        let state_tasker = tasker.clone();
        let shared_state = state::AppState(Arc::new(state::InnerState::new(
            version.to_string(),
            key_manager,
            consul_config_builder.build().unwrap(),
            state_tasker.clone(),
            updater_tx.clone(),
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
                .await;
            });
        }

        let app = build_router(shared_state.clone());

        info!("getting insecure_server placeholder");
        let insecure_server = match start_insecure_server {
            true => {
                debug!("starting insecure server");
                let mut insecure_notify = shutdown_tx.subscribe();
                axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
                    .serve(app.clone().into_make_service())
                    .with_graceful_shutdown(async move {
                        debug!("waiting for insecure port to shutdown");
                        insecure_notify.recv().await.unwrap();
                        debug!("insecure port shutdown");
                    })
            }
            false => std::future::pending().await,
        };

        info!("getting secure_server placeholder");
        let secure_server = match start_secure_server {
            true => {
                let tls_config =
                    RustlsConfig::from_pem_file(cerificate.unwrap(), cerificate_key.unwrap())
                        .await
                        .unwrap();

                let addr = SocketAddr::from(([0, 0, 0, 0], secure_port.parse::<u16>().unwrap()));

                let handle = Handle::new();

                let mut secure_notify = shutdown_tx.subscribe();
                let shutdown_handler = handle.clone();
                tokio::spawn(async move {
                    debug!("waiting for secure port to shutdown");
                    secure_notify.recv().await.unwrap();
                    handle.shutdown();
                    debug!("secure port shutdown");
                });

                axum_server::bind_rustls(addr, tls_config)
                    .handle(shutdown_handler)
                    .serve(app.clone().into_make_service())
            }
            false => std::future::pending().await,
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
                        _ = insecure_server => {},
            _ = secure_server => {},
        }

        info!("signal received, starting graceful shutdown");

        shutdown_tx.send(true).unwrap();
        state_tasker.finish();
    }

    let signaller = tasker.signaller();
    if signaller.stop() {
        debug!("Stopping background tasks");
    }

    tasker.join().await;

    debug!("Shutdown complete");

    Ok(())
}
