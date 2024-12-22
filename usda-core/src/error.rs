use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    InvalidInput(String),
    NotFound(String),
    DatabaseError(String),
    InsufficientBalance,
    InvalidSignature,
    InvalidNonce,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::InsufficientBalance => (
                StatusCode::BAD_REQUEST,
                "Insufficient balance for transaction".into(),
            ),
            AppError::InvalidSignature => (
                StatusCode::BAD_REQUEST,
                "Invalid transaction signature".into(),
            ),
            AppError::InvalidNonce => (
                StatusCode::BAD_REQUEST,
                "Invalid transaction nonce".into(),
            ),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".into()),
            _ => AppError::DatabaseError(err.to_string()),
        }
    }
}
