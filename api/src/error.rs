use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Internal(anyhow::Error),
    Auth(String),
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Internal(err) => {
                tracing::error!("Internal server error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::Auth(msg) => {
                (StatusCode::UNAUTHORIZED, msg.clone())
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone())
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(inner: sqlx::Error) -> Self {
        AppError::Internal(anyhow::anyhow!(inner))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(inner: anyhow::Error) -> Self {
        AppError::Internal(inner)
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(inner: argon2::password_hash::Error) -> Self {
        AppError::Internal(anyhow::anyhow!(inner))
    }
}
