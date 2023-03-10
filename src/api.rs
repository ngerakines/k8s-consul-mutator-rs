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
use serde_json::json;
use std::error::Error;
use tower_http::trace::TraceLayer;
use tracing::warn;

use crate::error::{ConMutError, Result};
use crate::state::{AppState, ConsulWatch};

async fn handle_index(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({"version": state.settings.version}))
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

        if let Err(err) = state
            .key_manager
            .watch(
                obj.namespace().unwrap(),
                obj.name_any().clone(),
                key.clone(),
                found_key_value.clone(),
            )
            .await
            .map_err(|err| anyhow!(err.to_string()))
        {
            warn!("Error watching key: {err}");
        }

        let now = Utc::now();

        if let Err(err) = state
            .consul_manager_tx
            .send(ConsulWatch::Create(found_key_value.clone(), now))
            .await
        {
            warn!("Error watching key: {err}");
        }

        let checksum = state.key_manager.get(found_key_value.clone()).await?;
        if checksum.is_some() {
            let checksum_value = checksum.unwrap();
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: format!("/metadata/annotations/k8s-consul-mutator.io~1checksum-{key}"),
                value: serde_json::Value::String(checksum_value.clone()),
            }));
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: format!(
                    "/spec/template/metadata/annotations/k8s-consul-mutator.io~1checksum-{key}"
                ),
                value: serde_json::Value::String(checksum_value.clone()),
            }));
        }
    }
    Ok(res.with_patch(json_patch::Patch(patches))?)
}

pub fn build_router(shared_state: AppState) -> Router {
    Router::new()
        .route("/", get(handle_index))
        .route("/mutate", post(handle_mutate))
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state)
}
