use futures::prelude::*;
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    api::{Api, ListParams, ResourceExt},
    runtime::watcher,
    Client,
};
use tokio_tasker::Stopper;
use tracing::{error, info};

use crate::state::AppState;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub struct FullSubscription {
    pub namespace: String,
    pub deployment: String,
    pub config_key: String,
    pub consul_key: String,
}

/// This is the main loop that watches for deployment events in Kubernetes.
/// When apply or delete events are received, deployments are watched and
/// unwatched accordingly.
///
/// This does not notify the key manager of consul key subscription changes,
/// but relies on the reconcile step to eventually create subscriptions that
/// don't exist.
pub async fn deployment_watch(app_state: AppState, stopper: Stopper) -> Result<(), anyhow::Error> {
    let client = Client::try_default().await.map_err(anyhow::Error::msg)?;
    let api = Api::<Deployment>::all(client.clone());

    info!("kubernetes deployment watcher started");

    let deployment_watcher = watcher(api, ListParams::default()).try_for_each(|event| async {
        match event {
            kube::runtime::watcher::Event::Deleted(d) => {
                // TODO: Don't unwatch deployments that aren't annotated.
                let namespace = d.namespace();
                let name = d.name_any();
                if namespace.is_some() && !name.is_empty() {
                    if let Err(err) = app_state
                        .key_manager
                        .unwatch_deployment(namespace.clone().unwrap(), name)
                        .await
                    {
                        error!(
                            "kubernetes deployment watcher error: failed to unwatch deployment: {}",
                            err
                        );
                    }
                }
            }
            kube::runtime::watcher::Event::Applied(d) => {
                // TODO: Look for annotation removal and unwatch accordingly.
                let subscriptions = subscriptions_from_deployment(&d).await;
                for sub in subscriptions {
                    if let Err(err) = app_state
                        .key_manager
                        .watch(
                            sub.namespace,
                            sub.deployment,
                            sub.config_key,
                            sub.consul_key.clone(),
                        )
                        .await
                    {
                        error!(
                            "kubernetes deployment watcher error: failed to watch deployment: {}",
                            err
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
    });

    tokio::select! {
        res = deployment_watcher => {
            if let Err(e) = res {
                error!("kubernetes deployment watcher error: {}", e);
            }
        },
        _ = stopper => { },
    };

    info!("kubernetes deployment watcher stopped");

    Ok(())
}

async fn subscriptions_from_deployment(deployment: &Deployment) -> Vec<FullSubscription> {
    let mut results = vec![];

    if deployment
        .annotations()
        .contains_key("k8s-consul-mutator.io/skip")
    {
        return results;
    }

    let found_keys: Vec<String> = deployment
        .annotations()
        .keys()
        .cloned()
        .filter(|x| x.starts_with("k8s-consul-mutator.io/key-"))
        .collect();

    if found_keys.is_empty() {
        return results;
    }

    for found_key in found_keys {
        let key = found_key.replace("k8s-consul-mutator.io/key-", "");

        let found_key_value = deployment.annotations().get(found_key.as_str()).unwrap();
        results.push(FullSubscription {
            namespace: deployment.namespace().unwrap(),
            deployment: deployment.name_any().clone(),
            config_key: key,
            consul_key: found_key_value.clone(),
        });
    }

    results
}
