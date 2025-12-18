use axum::{
    extract::{Path, State},
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    auth::{password::{hash_password, verify_password}, AuthUser},
    error::{AppError, AppResult},
    models::{User, UserResponse},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users))
        .route("/me/password", put(change_password))
        .route("/me", delete(delete_account))
        .route("/{id}", get(get_user))
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Deserialize)]
struct DeleteAccountRequest {
    password: String,
}

#[derive(Serialize)]
struct ProjectOutcome {
    project_id: i64,
    project_name: String,
    outcome: String, // "transferred", "deleted"
    transferred_to: Option<String>, // username if transferred
}

#[derive(Serialize)]
struct DeleteAccountResponse {
    message: String,
    affected_projects: Vec<ProjectOutcome>,
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

async fn change_password(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Validate new password
    if req.new_password.len() < 6 {
        return Err(AppError::Validation("New password must be at least 6 characters".to_string()));
    }

    // Get current user with password hash
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(auth.user_id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Verify current password
    if !verify_password(&req.current_password, &user.password_hash)? {
        return Err(AppError::InvalidCredentials);
    }

    // Hash new password
    let new_hash = hash_password(&req.new_password)?;

    // Update password
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    Ok(Json(serde_json::json!({ "message": "Password changed successfully" })))
}

async fn delete_account(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(req): Json<DeleteAccountRequest>,
) -> AppResult<Json<DeleteAccountResponse>> {
    // Get current user with password hash
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(auth.user_id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Verify password
    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::InvalidCredentials);
    }

    let mut affected_projects = Vec::new();

    // Get all projects where user is creator
    #[derive(sqlx::FromRow)]
    struct OwnedProject {
        id: i64,
        name: String,
    }

    let owned_projects: Vec<OwnedProject> = sqlx::query_as(
        "SELECT id, name FROM projects WHERE created_by = ?"
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    for project in owned_projects {
        // Check for other admins
        #[derive(sqlx::FromRow)]
        struct OtherAdmin {
            user_id: i64,
            username: String,
        }

        let other_admin: Option<OtherAdmin> = sqlx::query_as(
            "SELECT pm.user_id, u.username
             FROM project_members pm
             JOIN users u ON pm.user_id = u.id
             WHERE pm.project_id = ? AND pm.user_id != ? AND pm.role = 'admin' AND pm.status = 'active'
             LIMIT 1"
        )
        .bind(project.id)
        .bind(auth.user_id)
        .fetch_optional(&pool)
        .await?;

        if let Some(admin) = other_admin {
            // Transfer ownership to another admin
            sqlx::query("UPDATE projects SET created_by = ? WHERE id = ?")
                .bind(admin.user_id)
                .bind(project.id)
                .execute(&pool)
                .await?;

            affected_projects.push(ProjectOutcome {
                project_id: project.id,
                project_name: project.name,
                outcome: "transferred".to_string(),
                transferred_to: Some(admin.username),
            });
        } else {
            // Check for other non-admin members
            let other_members_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id != ? AND status = 'active'"
            )
            .bind(project.id)
            .bind(auth.user_id)
            .fetch_one(&pool)
            .await?;

            if other_members_count > 0 {
                return Err(AppError::Validation(format!(
                    "Cannot delete account: project '{}' has other members but no other admins. Please promote another member to admin first.",
                    project.name
                )));
            }

            // No other members - delete the project (will cascade to all related records)
            sqlx::query("DELETE FROM projects WHERE id = ?")
                .bind(project.id)
                .execute(&pool)
                .await?;

            affected_projects.push(ProjectOutcome {
                project_id: project.id,
                project_name: project.name,
                outcome: "deleted".to_string(),
                transferred_to: None,
            });
        }
    }

    // Nullify used_by in any participant invites that reference this user
    sqlx::query("UPDATE participant_invites SET used_by = NULL WHERE used_by = ?")
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    // Unlink user from all participants (in projects they didn't own)
    sqlx::query("UPDATE participants SET user_id = NULL WHERE user_id = ?")
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    // Delete user (this will CASCADE delete project_members entries)
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    Ok(Json(DeleteAccountResponse {
        message: "Account deleted successfully".to_string(),
        affected_projects,
    }))
}
