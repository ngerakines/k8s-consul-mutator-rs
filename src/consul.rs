use chrono::{DateTime, Duration, Utc};
use consulrs::{
    api::{features::Blocking, kv::requests::ReadKeyRequest, Features},
    client::{ConsulClient, ConsulClientSettings},
    kv,
};
use std::convert::TryInto;
use std::error::Error;

use crate::state::{AppState, Work};
use tokio::time::sleep;
use tokio_tasker::Stopper;
use tracing::{debug, error, info, warn};

pub async fn check_key(
    consul_config: ConsulClientSettings,
    consul_key: String,
    timeout: String,
    stopper: Stopper,
    app_state: AppState,
) {
    info!("consul key watcher started: {consul_key}");

    let consul_client_maybe = ConsulClient::new(consul_config);
    if let Err(ref err) = consul_client_maybe {
        warn!("consul key watcher error: {consul_key}: {err}");
        return;
    }
    let consul_client = consul_client_maybe.unwrap();

    let mut key_index = 0;
    let mut stop_countdown: Option<DateTime<Utc>> = None;

    while !stopper.is_stopped() {
        if app_state
            .key_manager
            .consul_key_subscriber_count(consul_key.clone())
            .await
            .unwrap()
            == 0
        {
            if stop_countdown.is_none() {
                warn!("consul key watcher preparing to stop:{consul_key}");
                stop_countdown = Some(Utc::now() + Duration::seconds(60));
            } else if Utc::now() > stop_countdown.unwrap() {
                warn!("consul key watcher stopping because of inactivity:{consul_key}");
                break;
            }
        } else {
            stop_countdown = None;
        }

        let wait_res = kv::read(
            &consul_client,
            &consul_key,
            Some(
                ReadKeyRequest::builder().features(
                    Features::builder()
                        .blocking(Blocking {
                            index: key_index,
                            wait: Some(timeout.clone()),
                        })
                        .build()
                        .unwrap(),
                ),
            ),
        )
        .await;

        if stopper.is_stopped() {
            break;
        }

        if let Err(err) = wait_res {
            if let Some(source) = err.source() {
                error!(
                    "consul key watcher error: {consul_key}: {:?} - {:?}",
                    err, source
                );
            } else {
                error!("consul key watcher error: {consul_key}: {:?}", err);
            }
            sleep(Duration::seconds(10).to_std().unwrap()).await;
            continue;
        }

        let mut wait_success = wait_res.unwrap();

        if wait_success.response.is_empty() {
            warn!("watch {consul_key} error: no keys returned from consul for key");
            sleep(Duration::seconds(10).to_std().unwrap()).await;
            continue;
        }

        let kv = wait_success.response.pop().unwrap();
        if kv.modify_index == key_index {
            debug!("consul key watcher error: {consul_key}: modify index is the same as last time {key_index}");
            continue;
        }

        key_index = kv.modify_index;

        if kv.value.is_none() {
            warn!("consul key watcher error: {consul_key}: value option is none");
            sleep(Duration::seconds(10).to_std().unwrap()).await;
            continue;
        }

        if stopper.is_stopped() {
            break;
        }

        let key_content: Vec<u8> = kv.value.unwrap().try_into().unwrap_or_else(|_| Vec::new());
        let digest = md5::compute(key_content);

        debug!("consul key watcher checksum: {consul_key} {:x}", digest);
        if let Err(err) = app_state
            .key_manager
            .set(consul_key.clone(), format!("{:x}", digest))
            .await
        {
            warn!("consul key watcher error: {consul_key}: {err}");
            continue;
        }

        let subscribers_res = app_state
            .key_manager
            .subscriptions_for_consul_key(consul_key.clone())
            .await;
        if let Err(err) = subscribers_res {
            warn!("consul key watcher error: {consul_key}: {err}");
            continue;
        }
        let subscribers = subscribers_res.unwrap();

        let now = Utc::now();

        for subscriber in subscribers {
            warn!(
                "consul key watcher notifying: {consul_key} {:?}",
                subscriber
            );

            if let Err(err) = app_state
                .tx
                .send(Work {
                    namespace: subscriber.namespace.clone(),
                    deployment: subscriber.deployment.clone(),
                    occurred: now,
                })
                .await
            {
                warn!("consul key watcher error: {consul_key}: {err}");
            }
        }
    }
    info!("consul key watcher stopped: {consul_key}");
}
