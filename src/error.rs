use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub use anyhow::{Error, Result};

#[derive(Debug)]
pub struct ConMutError {
    err: anyhow::Error,
}

impl IntoResponse for ConMutError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", self.err)).into_response()
    }
}

impl From<anyhow::Error> for ConMutError {
    fn from(err: anyhow::Error) -> ConMutError {
        ConMutError { err }
    }
}
