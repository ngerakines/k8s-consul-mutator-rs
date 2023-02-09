use axum::{
    extract::{Json, State},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    DynamicObject, ResourceExt,
};
use serde_json::json;
use std::error::Error;
use tower_http::trace::TraceLayer;

use crate::error::{ConMutError, Result};
use crate::state::AppState;

async fn handle_index(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "version": state.version,
    }))
}

async fn handle_mutate(
    State(_state): State<AppState>,
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
        res = match mutate(res.clone(), &obj) {
            Ok(res) => res,
            Err(err) => res.deny(err.to_string()),
        };
    };

    let real_res = &res.into_review();
    Ok((StatusCode::OK, Json(real_res.clone()))) as Result<_, ConMutError>
}

fn mutate(
    res: AdmissionResponse,
    obj: &DynamicObject,
) -> Result<AdmissionResponse, Box<dyn Error>> {
    if obj.annotations().contains_key("k8s-consul-mutator.io/skip") {
        return Ok(res);
    }
    let found_keys: Vec<String> = obj
        .labels()
        .keys()
        .cloned()
        .filter(|x| x.starts_with("k8s-consul-mutator.io/key-"))
        .collect();

    for found_key in found_keys {
        println!("Got: {}", found_key);
    }

    Ok(res)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
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

pub async fn run(listen: &str, shared_state: AppState) -> Result<()> {
    let app = build_router(shared_state);

    axum::Server::bind(&listen.parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
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
        let shared_state = state::AppState(Arc::new(state::InnerState::new("1".to_string())));
        let router = build_router(shared_state);
        let client = TestClient::new(router);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, r#"{"version":"1"}"#);
    }
}
