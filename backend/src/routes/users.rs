use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{User, UserResponse},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users))
        .route("/{id}", get(get_user))
}

async fn list_users(
    _auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<UserResponse>>> {
    let users: Vec<User> = sqlx::query_as("SELECT * FROM users ORDER BY username")
        .fetch_all(&pool)
        .await?;

    Ok(Json(users.into_iter().map(UserResponse::from).collect()))
}

async fn get_user(
    _auth: AuthUser,
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> AppResult<Json<UserResponse>> {
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(UserResponse::from(user)))
}
