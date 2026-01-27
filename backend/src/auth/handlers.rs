use axum::{extract::State, Json};
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult, AuthFailureReason, ErrorCode},
    models::{AuthResponse, CreateUser, LoginRequest, User, UserResponse, UserState},
};

use super::{
    jwt::create_token,
    password::{hash_password, verify_password},
};

pub async fn register(
    State(pool): State<SqlitePool>,
    State(jwt_secret): State<String>,
    Json(input): Json<CreateUser>,
) -> AppResult<Json<AuthResponse>> {
    let username = input.username.trim().to_string();

    tracing::info!(
        event = "auth.register.attempt",
        username = %username,
        "Registration attempt"
    );

    // Validate input
    if username.is_empty() {
        tracing::warn!(
            event = "auth.register.failure",
            reason = AuthFailureReason::InvalidInput.as_code(),
            "Registration failed: empty username"
        );
        return Err(AppError::bad_request(ErrorCode::UsernameRequired));
    }
    if input.password.len() < 6 {
        tracing::warn!(
            event = "auth.register.failure",
            reason = AuthFailureReason::PasswordTooWeak.as_code(),
            username = %username,
            "Registration failed: password too short"
        );
        return Err(AppError::bad_request(ErrorCode::PasswordTooWeak));
    }

    // Check if user exists
    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&pool)
        .await?;

    if existing.is_some() {
        tracing::warn!(
            event = "auth.register.failure",
            reason = AuthFailureReason::UsernameExists.as_code(),
            username = %username,
            "Registration failed: username exists"
        );
        return Err(AppError::UserExists);
    }

    // Hash password and create user
    let password_hash = hash_password(&input.password)?;

    let result = sqlx::query(
        "INSERT INTO users (username, display_name, password_hash, user_state, token_version) VALUES (?, ?, ?, 'active', 1)"
    )
    .bind(&username)
    .bind(&input.display_name)
    .bind(&password_hash)
    .execute(&pool)
    .await?;

    let user_id = result.last_insert_rowid();

    // Fetch created user
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;

    // Generate token with token_version
    let token = create_token(user.id, &user.username, user.token_version, &jwt_secret)?;

    tracing::info!(
        event = "auth.register.success",
        user_id = user_id,
        username = %username,
        "Registration successful"
    );

    Ok(Json(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}

pub async fn login(
    State(pool): State<SqlitePool>,
    State(jwt_secret): State<String>,
    Json(input): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let username = input.username.trim();

    tracing::info!(
        event = "auth.login.attempt",
        username = %username,
        "Login attempt"
    );

    // Find user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(&pool)
        .await?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(
                event = "auth.login.failure",
                reason = AuthFailureReason::InvalidCredentials.as_code(),
                username = %username,
                "Login failed: user not found"
            );
            return Err(AppError::InvalidCredentials);
        }
    };

    // Check user state before allowing login
    // Pending users CAN log in (they'll be restricted by middleware)
    // Revoked users are blocked entirely
    match user.state() {
        UserState::Active | UserState::PendingApproval => {
            // User can log in, middleware will handle access restrictions
        }
        UserState::Revoked => {
            tracing::warn!(
                event = "auth.login.failure",
                reason = AuthFailureReason::AccountRevoked.as_code(),
                user_id = user.id,
                username = %username,
                "Login failed: account revoked"
            );
            return Err(AppError::AccountRevoked);
        }
    }

    // Verify password
    let valid = verify_password(&input.password, &user.password_hash)?;
    if !valid {
        tracing::warn!(
            event = "auth.login.failure",
            reason = AuthFailureReason::InvalidCredentials.as_code(),
            user_id = user.id,
            username = %username,
            "Login failed: invalid password"
        );
        return Err(AppError::InvalidCredentials);
    }

    // Cancel any pending recovery requests - user proved they know their password
    sqlx::query(
        "UPDATE recovery_intents SET status = 'used', resolved_at = datetime('now') WHERE user_id = ? AND status = 'pending'"
    )
    .bind(user.id)
    .execute(&pool)
    .await
    .ok(); // Ignore errors, this is a cleanup operation

    // Generate token with token_version
    let token = create_token(user.id, &user.username, user.token_version, &jwt_secret)?;

    tracing::info!(
        event = "auth.login.success",
        user_id = user.id,
        username = %username,
        "Login successful"
    );

    Ok(Json(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}
