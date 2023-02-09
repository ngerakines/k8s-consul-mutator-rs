use axum::{
    extract::{Json, State},
    http::Request,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::error::Result;
use crate::state::AppState;

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "version": state.version,
    }))
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
}

pub async fn run(listen: &str, shared_state: AppState) -> Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .layer(sentry_tower::NewSentryLayer::<Request<_>>::new_from_top())
        .layer(sentry_tower::SentryHttpLayer::with_transaction())
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state);

    axum::Server::bind(&listen.parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
