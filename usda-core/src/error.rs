use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::error::Error as SqlxError;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    DatabaseError(sqlx::Error),
    NotFound(String),
    InvalidInput(String),
    InvalidAmount(String),
    InvalidNonce(String),
    InvalidSignature(String),
    InsufficientBalance(String),
    WebSocketError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DatabaseError(e) => write!(f, "Database error: {}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::InvalidAmount(msg) => write!(f, "Invalid amount: {}", msg),
            AppError::InvalidNonce(msg) => write!(f, "Invalid nonce: {}", msg),
            AppError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            AppError::InsufficientBalance(msg) => write!(f, "Insufficient balance: {}", msg),
            AppError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::DatabaseError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InvalidAmount(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InvalidNonce(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InvalidSignature(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::InsufficientBalance(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::WebSocketError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, message).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_response() {
        let db_error = sqlx::Error::RowNotFound;
        let error = AppError::DatabaseError(db_error);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_not_found_error_response() {
        let error = AppError::NotFound("Resource not found".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_invalid_input_error_response() {
        let error = AppError::InvalidInput("Invalid input".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_invalid_amount_error_response() {
        let error = AppError::InvalidAmount("Invalid amount".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_invalid_nonce_error_response() {
        let error = AppError::InvalidNonce("Invalid nonce".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_invalid_signature_error_response() {
        let error = AppError::InvalidSignature("Invalid signature".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_insufficient_balance_error_response() {
        let error = AppError::InsufficientBalance("Insufficient balance".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_web_socket_error_response() {
        let error = AppError::WebSocketError("WebSocket error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
