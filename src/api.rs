use axum::{
    extract::{Json, State},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use consulrs::client::ConsulClientSettingsBuilder;
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    DynamicObject, ResourceExt,
};
use serde_json::json;
use std::error::Error;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::{
    consul::check_key,
    error::{ConMutError, Result},
};

async fn handle_index(State(state): State<AppState>) -> impl IntoResponse {
    let consul_config = ConsulClientSettingsBuilder::default()
        .address("http://127.0.0.1:8500")
        .build()
        .unwrap();

    let tasker2 = state.tasker.clone();
    let task_shared_state = state.clone();
    state.tasker.spawn(async move {
        check_key(
            consul_config,
            "apps/foo/settings".to_string(),
            "10s".to_string(),
            tasker2.stopper(),
            task_shared_state,
        )
        .await;
        tasker2.finish();
    });

    Json(json!({"version": state.version}))
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

        let checksum_key = format!("k8s-consul-mutator.io/checksum-{key}");
        if obj.annotations().contains_key(checksum_key.as_str()) {
            continue;
        }

        if obj.metadata.annotations.is_none() {
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: "/metadata/annotations".into(),
                value: serde_json::Value::Object(serde_json::Map::new()),
            }));
        }

        state.key_manager.watch(key.clone()).await?;

        let checksum = state.key_manager.get(key.clone()).await?;
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
        .layer(sentry_tower::NewSentryLayer::<Request<_>>::new_from_top())
        .layer(sentry_tower::SentryHttpLayer::with_transaction())
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

    #[tokio::test]
    async fn test_handle_index() {
        let router = build_router(state::AppState(Arc::new(Default::default())));
        let client = TestClient::new(router);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, r#"{"version":"default"}"#);
    }
}
