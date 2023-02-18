use chrono::{DateTime, Duration, Utc};
use consulrs::{
    api::{features::Blocking, kv::requests::ReadKeyRequest, Features},
    client::{ConsulClient, ConsulClientSettings},
    kv,
};
use std::error::Error;
use std::{collections::HashSet, convert::TryInto};

use crate::state::{AppState, ConsulWatch, Work};
use tokio::{
    sync::mpsc::Receiver,
    time::{sleep, Instant},
};
use tokio_tasker::Stopper;
use tracing::{debug, error, info, trace, warn};

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

    let idle_duration = chrono::Duration::seconds(app_state.settings.check_key_idle as i64);
    let error_wait_duration =
        chrono::Duration::seconds(app_state.settings.check_key_error_wait as i64);

    while !stopper.is_stopped() {
        let now = Utc::now();

        if app_state
            .key_manager
            .consul_key_subscriber_count(consul_key.clone())
            .await
            .unwrap()
            == 0
        {
            if stop_countdown.is_none() {
                warn!("consul key watcher preparing to stop:{consul_key}");
                stop_countdown = Some(Utc::now() + idle_duration);
            } else if Utc::now() > stop_countdown.unwrap() {
                warn!("consul key watcher stopping because of inactivity: {consul_key}");

                if let Err(err) = app_state
                    .consul_manager_tx
                    .send(ConsulWatch::Destroy(consul_key.clone(), now))
                    .await
                {
                    error!("consul key watcher error: {consul_key}: {err}");
                }

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
            sleep(error_wait_duration.to_std().unwrap()).await;
            continue;
        }

        let mut wait_success = wait_res.unwrap();

        if wait_success.response.is_empty() {
            warn!("watch {consul_key} error: no keys returned from consul for key");
            sleep(error_wait_duration.to_std().unwrap()).await;
            continue;
        }

        let kv = wait_success.response.pop().unwrap();
        if kv.modify_index == key_index {
            trace!("consul key watcher error: {consul_key}: modify index is the same as last time {key_index}");
            continue;
        }

        key_index = kv.modify_index;

        if kv.value.is_none() {
            warn!("consul key watcher error: {consul_key}: value option is none");
            sleep(error_wait_duration.to_std().unwrap()).await;
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
            .set(consul_key.clone(), format!("md5-{:x}", digest))
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

        for subscriber in subscribers {
            info!(
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

pub async fn watch_dispatcher(
    stopper: Stopper,
    app_state: AppState,
    rx: &mut Receiver<ConsulWatch>,
) {
    info!("consul dispatcher started");

    let one_second = Duration::seconds(1).to_std().unwrap();

    let sleeper = sleep(one_second);
    tokio::pin!(sleeper);

    let mut work: HashSet<ConsulWatch> = HashSet::new();
    let mut running_watchers: HashSet<String> = HashSet::new();

    let mut last_reconcile: Option<DateTime<Utc>> = None;

    let debounce_duration =
        chrono::Duration::seconds(app_state.settings.watch_dispatcher_debounce as i64);
    let first_reconcile_duration =
        chrono::Duration::seconds(app_state.settings.watch_dispatcher_first_reconcile as i64);
    let reconcile_duration =
        chrono::Duration::seconds(app_state.settings.watch_dispatcher_reconcile as i64);

    let check_key_timeout = app_state.settings.check_key_timeout.clone();

    while !stopper.is_stopped() {
        tokio::select! {
            biased;
            r = rx.recv() => {
                let val = r.unwrap();
                match val {
                    ConsulWatch::Create(key, occurred) => {
                        // If there are any delete notifications for this key, don't insert it.
                        work.retain(|k| match k {
                            ConsulWatch::Create(k, _) => k != &key,
                            ConsulWatch::Destroy(k, _) => k != &key,
                        });
                        work.insert(ConsulWatch::Create(key, occurred));
                    },
                    ConsulWatch::Destroy(key, occurred) => {
                        // If there are any create notifications for this key, remove them. The reconcile will handle creating consul key watchers that may fall through the cracks.
                        work.retain(|k| match k {
                            ConsulWatch::Create(k, _) => k != &key,
                            ConsulWatch::Destroy(k, _) => k != &key,
                        });
                        work.insert(ConsulWatch::Destroy(key, occurred));
                    }
                }
            }
            () = &mut sleeper => {
                sleeper.as_mut().reset(Instant::now() + one_second);
                trace!("consul dispatcher timed out, resetting sleep");
            }
        }

        if stopper.is_stopped() {
            break;
        }
        let now = Utc::now();

        if last_reconcile.is_none() {
            last_reconcile = Some(Utc::now() + first_reconcile_duration);
            debug!("consul dispatcher reconciling in 30 seconds");
        } else if now > last_reconcile.unwrap() && work.is_empty() {
            last_reconcile = Some(Utc::now() + reconcile_duration);

            let consul_keys = app_state.key_manager.consul_keys().await;
            if let Err(err) = consul_keys {
                warn!("consul dispatcher error: {err}");
                continue;
            }
            for consul_key in consul_keys.unwrap() {
                if !running_watchers.contains(&consul_key) {
                    let tasker2 = app_state.tasker.clone();
                    let task_shared_state = app_state.clone();
                    let consul_config = app_state.consul_settings.clone();
                    let inner_consul_key = consul_key.clone();
                    let inner_check_key_timeout = check_key_timeout.clone();

                    app_state.tasker.spawn(async move {
                        check_key(
                            consul_config,
                            inner_consul_key,
                            inner_check_key_timeout,
                            tasker2.stopper(),
                            task_shared_state,
                        )
                        .await;
                        tasker2.finish();
                    });
                    running_watchers.insert(consul_key);
                }
            }
            continue;
        }

        let debounce_gap = now - debounce_duration;

        let mut drained: Vec<ConsulWatch> = vec![];
        for v in work.iter() {
            match v {
                ConsulWatch::Create(consul_key, occurred) => {
                    if occurred < &debounce_gap {
                        if !running_watchers.contains(consul_key) {
                            let tasker2 = app_state.tasker.clone();
                            let task_shared_state = app_state.clone();
                            let consul_config = app_state.consul_settings.clone();
                            let inner_consul_key = consul_key.clone();
                            let inner_check_key_timeout = check_key_timeout.clone();
                            app_state.tasker.spawn(async move {
                                check_key(
                                    consul_config,
                                    inner_consul_key,
                                    inner_check_key_timeout,
                                    tasker2.stopper(),
                                    task_shared_state,
                                )
                                .await;
                                tasker2.finish();
                            });
                        }

                        running_watchers.insert(consul_key.clone());
                        drained.push(v.clone());
                    }
                }
                ConsulWatch::Destroy(consul_key, _) => {
                    drained.push(v.clone());
                    running_watchers.remove(consul_key);
                }
            }
        }

        for element in drained {
            debug!("consul dispatcher processing {:?}", element);
            work.remove(&element);
        }
    }
    info!("consul dispatcher stopped");
}
