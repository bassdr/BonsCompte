use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,           // user id
    pub username: String,
    pub exp: i64,           // expiration timestamp
    pub iat: i64,           // issued at
    pub token_version: i64, // for invalidation on password reset/revoke
}

pub fn create_token(user_id: i64, username: &str, token_version: i64, secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let exp = now + Duration::hours(24);

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        token_version,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Validate a JWT token, distinguishing between expired and invalid tokens
pub fn validate_token(token: &str, secret: &str) -> AppResult<Claims> {
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(token_data) => Ok(token_data.claims),
        Err(e) => {
            match e.kind() {
                ErrorKind::ExpiredSignature => {
                    // Expected behavior - log at info level, not error
                    tracing::info!("JWT expired for token validation");
                    Err(AppError::TokenExpired)
                }
                _ => {
                    // Unexpected token issues - log at warn level
                    tracing::warn!("JWT validation failed: {:?}", e.kind());
                    Err(AppError::InvalidToken)
                }
            }
        }
    }
}
