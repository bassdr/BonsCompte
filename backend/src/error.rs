use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Enumerated auth failure reasons for structured logging and frontend messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailureReason {
    PasswordTooWeak,
    UsernameExists,
    InvalidCredentials,
    InvalidInput,
    InternalError,
    AccountPending,
    AccountRevoked,
}

impl AuthFailureReason {
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::PasswordTooWeak => "PASSWORD_TOO_WEAK",
            Self::UsernameExists => "USERNAME_EXISTS",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::InvalidInput => "INVALID_INPUT",
            Self::InternalError => "INTERNAL_ERROR",
            Self::AccountPending => "ACCOUNT_PENDING",
            Self::AccountRevoked => "ACCOUNT_REVOKED",
        }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Account pending approval")]
    AccountPendingApproval,

    #[error("Account revoked")]
    AccountRevoked,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token invalidated")]
    TokenInvalidated,

    #[error("Invalid token")]
    InvalidToken,

    #[error("User already exists")]
    UserExists,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                self.to_string(),
            ),
            AppError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                AuthFailureReason::InvalidCredentials.as_code(),
                self.to_string(),
            ),
            AppError::AccountPendingApproval => (
                StatusCode::FORBIDDEN,
                AuthFailureReason::AccountPending.as_code(),
                self.to_string(),
            ),
            AppError::AccountRevoked => (
                StatusCode::FORBIDDEN,
                AuthFailureReason::AccountRevoked.as_code(),
                self.to_string(),
            ),
            AppError::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                "Session expired, please log in again".to_string(),
            ),
            AppError::TokenInvalidated => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_INVALIDATED",
                "Session invalidated, please log in again".to_string(),
            ),
            AppError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "Invalid token".to_string(),
            ),
            AppError::UserExists => (
                StatusCode::CONFLICT,
                AuthFailureReason::UsernameExists.as_code(),
                self.to_string(),
            ),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Validation(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                msg.clone(),
            ),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Database error".to_string(),
                )
            }
            AppError::Jwt(e) => {
                // Don't log JWT errors as ERROR - they're expected behavior
                tracing::debug!("JWT validation failed: {:?}", e);
                (
                    StatusCode::UNAUTHORIZED,
                    "INVALID_TOKEN",
                    "Invalid token".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Internal error".to_string(),
                )
            }
        };

        let body = Json(json!({ "error": message, "code": code }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
