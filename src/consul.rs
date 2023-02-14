use chrono::Utc;
use consulrs::{
    api::{features::Blocking, kv::requests::ReadKeyRequest, Features},
    client::{ConsulClient, ConsulClientSettings},
    kv,
};
use std::convert::TryInto;
use std::error::Error;

use crate::state::{AppState, Work};
use tokio::time::{sleep, Duration};
use tokio_tasker::Stopper;
use tracing::{debug, error, info, warn};

pub async fn check_key(
    consul_config: ConsulClientSettings,
    consul_key: String,
    timeout: String,
    stopper: Stopper,
    app_state: AppState,
) {
    debug!("watch {consul_key} starting");
    let consul_client_maybe = ConsulClient::new(consul_config);
    if let Err(ref error) = consul_client_maybe {
        error!("watch {consul_key} error: {error}");
        return;
    }
    let consul_client = consul_client_maybe.unwrap();

    let mut key_index = 0;

    while !stopper.is_stopped() {
        debug!("{consul_key} {key_index} loop");
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
                warn!("watch {consul_key} error: {:?} - {:?}", err, source);
            } else {
                warn!("watch {consul_key} error: {:?}", err);
            }
            sleep(Duration::from_secs(10)).await;
            continue;
        }

        let mut wait_success = wait_res.unwrap();

        if wait_success.response.is_empty() {
            warn!("watch {consul_key} error: no keys returned from consul for key");
            sleep(Duration::from_secs(10)).await;
            continue;
        }

        let kv = wait_success.response.pop().unwrap();
        if kv.modify_index == key_index {
            debug!("watch {consul_key} error: modify index is the same as last time {key_index}");
            continue;
        }

        key_index = kv.modify_index;

        if kv.value.is_none() {
            warn!("watch {consul_key} error: value option is none");
            sleep(Duration::from_secs(10)).await;
            continue;
        }

        if stopper.is_stopped() {
            break;
        }

        let key_content: Vec<u8> = kv.value.unwrap().try_into().unwrap_or_else(|_| Vec::new());
        let digest = md5::compute(key_content);

        info!("watch {consul_key} checksum: {:x}", digest);
        if let Err(err) = app_state
            .key_manager
            .set(consul_key.clone(), format!("{:x}", digest))
            .await
        {
            warn!("watch {consul_key} error: {err}");
            continue;
        }

        let subscribers_res = app_state
            .key_manager
            .subscriptions_for_consul_key(consul_key.clone())
            .await;
        if let Err(err) = subscribers_res {
            warn!("watch {consul_key} error: {err}");
            continue;
        }
        let subscribers = subscribers_res.unwrap();

        let now = Utc::now();

        for subscriber in subscribers {
            info!("watch {consul_key} notifying {:?}", subscriber);

            if let Err(err) = app_state
                .tx
                .send(Work {
                    namespace: subscriber.namespace.clone(),
                    deployment: subscriber.deployment.clone(),
                    occurred: now,
                })
                .await
            {
                warn!("watch {consul_key} error: {err}");
            }
        }

        info!("watch {consul_key} looping");
    }
    info!("watch {consul_key} stopping");
}
