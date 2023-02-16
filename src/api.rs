use anyhow::anyhow;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    DynamicObject, ResourceExt,
};
use serde::Deserialize;
use serde_json::json;
use std::error::Error;
use tower_http::trace::TraceLayer;

use crate::state::{AppState, Work};
use crate::{
    consul::check_key,
    error::{ConMutError, Result},
};

#[derive(Deserialize)]
struct WatchRequest {
    pub namespace: String,
    pub deployment: String,
    pub config_key: String,
    pub consul_key: String,
}

async fn handle_index(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({"version": state.version}))
}

async fn handle_watch(
    State(state): State<AppState>,
    Json(payload): Json<WatchRequest>,
) -> impl IntoResponse {
    let is_new_watch_res = state
        .key_manager
        .watch(
            payload.namespace.clone(),
            payload.deployment.clone(),
            payload.config_key.clone(),
            payload.consul_key.clone(),
        )
        .await
        .map_err(|err| anyhow!(err.to_string()));
    if is_new_watch_res.is_err() {
        return Json(json!({"error": is_new_watch_res.err().unwrap().to_string()}));
    }
    let is_new_watch = is_new_watch_res.unwrap();

    if is_new_watch {
        let tasker2 = state.tasker.clone();
        let task_shared_state = state.clone();
        let consul_config = state.consul_settings.clone();
        state.tasker.spawn(async move {
            check_key(
                consul_config,
                payload.consul_key.clone(),
                "10s".to_string(),
                tasker2.stopper(),
                task_shared_state,
            )
            .await;
            tasker2.finish();
        });
    }

    Json(json!({ "created": is_new_watch }))
}

async fn handle_work(
    State(state): State<AppState>,
    Json(payload): Json<WatchRequest>,
) -> impl IntoResponse {
    let now = Utc::now();
    if let Err(err) = state
        .tx
        .send(Work {
            namespace: payload.namespace.clone(),
            deployment: payload.deployment.clone(),
            occurred: now,
        })
        .await
    {
        return Json(json!({ "ok": false, "message": err.to_string() }));
    }

    Json(json!({ "ok": true }))
}

async fn handle_mutate(
    State(state): State<AppState>,
    Json(payload): Json<AdmissionReview<DynamicObject>>,
) -> impl IntoResponse {
    let req: AdmissionRequest<_> = match payload.try_into() {
        Ok(req) => req,
        Err(err) => {
            let ok = &AdmissionResponse::invalid(err.to_string()).into_review();
            return Ok((StatusCode::OK, Json(ok.clone()))) as Result<_, ConMutError>;
        }
    };

    let mut res = AdmissionResponse::from(&req);

    if let Some(obj) = req.object {
        res = match mutate(&state, res.clone(), &obj).await {
            Ok(res) => res,
            Err(err) => res.deny(err.to_string()),
        };
    };

    let real_res = &res.into_review();
    Ok((StatusCode::OK, Json(real_res.clone()))) as Result<_, ConMutError>
}

async fn mutate(
    state: &AppState,
    res: AdmissionResponse,
    obj: &DynamicObject,
) -> Result<AdmissionResponse, Box<dyn Error>> {
    if obj.annotations().contains_key("k8s-consul-mutator.io/skip") {
        return Ok(res);
    }
    let found_keys: Vec<String> = obj
        .annotations()
        .keys()
        .cloned()
        .filter(|x| x.starts_with("k8s-consul-mutator.io/key-"))
        .collect();

    if found_keys.is_empty() {
        return Ok(res);
    }

    let mut patches = Vec::new();

    for found_key in found_keys {
        let key = found_key.replace("k8s-consul-mutator.io/key-", "");

        let found_key_value = obj.annotations().get(found_key.as_str()).unwrap();

        let is_new_watch = state
            .key_manager
            .watch(
                obj.namespace().unwrap(),
                obj.name_any().clone(),
                key.clone(),
                found_key_value.clone(),
            )
            .await?;
        if is_new_watch {
            let tasker2 = state.tasker.clone();
            let task_shared_state = state.clone();
            let consul_config = state.consul_settings.clone();
            let consul_key = found_key_value.clone();
            state.tasker.spawn(async move {
                check_key(
                    consul_config,
                    consul_key,
                    "10s".to_string(),
                    tasker2.stopper(),
                    task_shared_state,
                )
                .await;
                tasker2.finish();
            });
        }

        if obj.metadata.annotations.is_none() {
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: "/metadata/annotations".into(),
                value: serde_json::Value::Object(serde_json::Map::new()),
            }));
        }

        let checksum = state.key_manager.get(found_key_value.clone()).await?;
        if checksum.is_some() {
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                // https://jsonpatch.com/#json-pointer
                path: format!("/metadata/annotations/k8s-consul-mutator.io~1checksum-{key}"),
                value: serde_json::Value::String(checksum.unwrap()),
            }));
        }
    }
    Ok(res.with_patch(json_patch::Patch(patches))?)
}

pub fn build_router(shared_state: AppState) -> Router {
    Router::new()
        .route("/", get(handle_index))
        .route("/mutate", post(handle_mutate))
        .route("/api/v1/watch", post(handle_watch))
        .route("/api/v1/work", post(handle_work))
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state)
}

#[cfg(test)]
mod tests {
    use crate::state;

    use super::*;

    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use std::sync::Arc;

    // #[tokio::test]
    // async fn test_handle_index() {
    //     let router = build_router(state::AppState(Arc::new(Default::default())));
    //     let client = TestClient::new(router);
    //     let res = client.get("/").send().await;
    //     assert_eq!(res.status(), StatusCode::OK);
    //     assert_eq!(res.text().await, r#"{"version":"default"}"#);
    // }
}
