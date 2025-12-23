use axum::{extract::State, Json};
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    models::{AuthResponse, CreateUser, LoginRequest, User, UserResponse, UserState},
};

use super::{jwt::create_token, password::{hash_password, verify_password}};

pub async fn register(
    State(pool): State<SqlitePool>,
    State(jwt_secret): State<String>,
    Json(input): Json<CreateUser>,
) -> AppResult<Json<AuthResponse>> {
    // Validate input
    if input.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }
    if input.password.len() < 6 {
        return Err(AppError::BadRequest("Password must be at least 6 characters".to_string()));
    }

    // Check if user exists
    let existing: Option<User> = sqlx::query_as(
        "SELECT * FROM users WHERE username = ?"
    )
    .bind(&input.username)
    .fetch_optional(&pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::UserExists);
    }

    // Hash password and create user
    let password_hash = hash_password(&input.password)?;

    let result = sqlx::query(
        "INSERT INTO users (username, display_name, password_hash, user_state, token_version) VALUES (?, ?, ?, 'active', 1)"
    )
    .bind(&input.username)
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
    // Find user
    let user: Option<User> = sqlx::query_as(
        "SELECT * FROM users WHERE username = ?"
    )
    .bind(&input.username)
    .fetch_optional(&pool)
    .await?;

    let user = user.ok_or(AppError::InvalidCredentials)?;

    // Check user state before allowing login
    match user.state() {
        UserState::Active => {
            // User is active, proceed with login
        }
        UserState::PendingApproval => {
            return Err(AppError::AccountPendingApproval);
        }
        UserState::Revoked => {
            return Err(AppError::AccountRevoked);
        }
    }

    // Verify password
    let valid = verify_password(&input.password, &user.password_hash)?;
    if !valid {
        return Err(AppError::InvalidCredentials);
    }

    // Generate token with token_version
    let token = create_token(user.id, &user.username, user.token_version, &jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}
