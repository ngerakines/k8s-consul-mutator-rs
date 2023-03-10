use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    api::{Api, Patch, PatchParams},
    Client,
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::{
    sync::mpsc::Receiver,
    time::{self, Instant},
};
use tokio_tasker::Stopper;
use tracing::{debug, error, info, trace};

use crate::state::{AppState, DeploymentUpdate};

/// This is the main loop that publishes checksum changes to deployment
/// resources in Kubernetes. It receives updates from the deployment watcher
/// and then debounces them before applying them.
pub async fn deployment_update_loop(
    app_state: AppState,
    stopper: Stopper,
    rx: &mut Receiver<DeploymentUpdate>,
) {
    let client = Client::try_default()
        .await
        .map_err(anyhow::Error::msg)
        .unwrap();

    let sleep = time::sleep(Duration::from_secs(1));
    tokio::pin!(sleep);

    let mut work: HashSet<DeploymentUpdate> = HashSet::new();

    info!("update worker starting");

    let debounce_duration = chrono::Duration::seconds(app_state.settings.update_debounce as i64);

    while !stopper.is_stopped() {
        tokio::select! {
            biased;
            r = rx.recv() => {
                let val = r.unwrap();
                debug!("update worker got value: {:?}", val);
                work.retain(|k| k.namespace != val.namespace && k.deployment != val.deployment);
                work.insert(val);
            }
            () = &mut sleep => {
                sleep.as_mut().reset(Instant::now() + Duration::from_secs(1));
                trace!("update worker timed out, resetting sleep");
            }
        }

        if stopper.is_stopped() {
            break;
        }

        let now = Utc::now();

        let mut drained: Vec<DeploymentUpdate> = vec![];
        for v in work.iter() {
            if v.occurred < now - debounce_duration {
                let deployment_client: Api<Deployment> =
                    Api::namespaced(client.clone(), &v.namespace.clone());

                let deployment_res = deployment_client.get_opt(&v.deployment).await;
                if let Ok(deployment_opt) = deployment_res {
                    if deployment_opt.is_some() {
                        let annotations_res = app_state
                            .key_manager
                            .deployment_annotations(v.namespace.clone(), v.deployment.clone())
                            .await;

                        if let Ok(annotations) = annotations_res {
                            let mut deployment_annotations: HashMap<String, String> =
                                HashMap::new();
                            let mut deployment_spec_annotations: HashMap<String, String> =
                                HashMap::new();

                            if app_state.settings.set_deployment_annotations {
                                for (k, v) in annotations.iter() {
                                    deployment_annotations.insert(
                                        format!("k8s-consul-mutator.io/checksum-{k}"),
                                        v.clone(),
                                    );
                                }
                            }
                            if app_state.settings.set_deployment_timestamp {
                                deployment_annotations.insert(
                                    "k8s-consul-mutator.io/last-updated".to_string(),
                                    now.to_rfc3339(),
                                );
                            }

                            if app_state.settings.set_deployment_spec_annotations {
                                for (k, v) in annotations.iter() {
                                    deployment_spec_annotations.insert(
                                        format!("k8s-consul-mutator.io/checksum-{k}"),
                                        v.clone(),
                                    );
                                }
                            }
                            if app_state.settings.set_deployment_spec_timestamp {
                                deployment_spec_annotations.insert(
                                    "k8s-consul-mutator.io/last-updated".to_string(),
                                    now.to_rfc3339(),
                                );
                            }

                            let body = json!({
                                "apiVersion": "apps/v1",
                                "kind": "Deployment",
                                "metadata": {
                                    "name": v.deployment.clone(),
                                    "annotations": deployment_annotations,
                                },
                                "spec": {
                                    "template": {
                                        "metadata": {
                                            "annotations": deployment_spec_annotations,
                                        }
                                    }
                                }
                            });

                            let patch_res = deployment_client
                                .patch(
                                    &v.deployment,
                                    &PatchParams::apply("k8s-consul-mutator"),
                                    &Patch::Merge(&body),
                                )
                                .await;
                            if let Err(err) = patch_res {
                                error!("update worker error: {err}");
                            }
                        } else if let Err(err) = annotations_res {
                            error!("update worker error: {err}");
                        }
                    } else if deployment_opt.is_none() {
                        error!(
                            "update worker error: deployment not found {}/{}",
                            v.namespace, v.deployment
                        );
                    }
                } else if let Err(err) = deployment_res {
                    error!("update worker error: {err}");
                }
                drained.push(v.clone());
            }
        }

        if stopper.is_stopped() {
            break;
        }

        for element in drained {
            debug!("update worker processing {:?}", element);
            work.remove(&element);
        }
    }
    info!("update worker stopped");
}
