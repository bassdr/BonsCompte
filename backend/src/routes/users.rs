use axum::{
    extract::{Path, State},
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    auth::{
        password::{hash_password, verify_password},
        AuthUser,
    },
    error::{AppError, AppResult},
    models::{User, UserPreferences, UserResponse, UserState},
    services::approval_service,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users))
        .route("/me/password", put(change_password))
        .route("/me/profile", put(update_profile))
        .route(
            "/me/preferences",
            get(get_preferences).put(update_preferences),
        )
        .route("/me", delete(delete_account))
        .route("/{id}", get(get_user))
        .route("/{id}/approve", put(approve_user))
        .route("/{id}/revoke", put(revoke_user))
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct DeleteAccountRequest {
    password: String,
}

#[derive(Deserialize)]
struct UpdatePreferencesRequest {
    date_format: Option<String>,
    decimal_separator: Option<String>,
    currency_symbol: Option<String>,
    currency_symbol_position: Option<String>,
}

#[derive(Serialize)]
struct ProjectOutcome {
    project_id: i64,
    project_name: String,
    outcome: String,                // "transferred", "deleted"
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
        return Err(AppError::Validation(
            "New password must be at least 6 characters".to_string(),
        ));
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

    // Update password AND increment token_version to invalidate existing sessions
    // This is critical for security: old tokens become invalid after password change
    sqlx::query(
        "UPDATE users SET password_hash = ?, token_version = token_version + 1 WHERE id = ?",
    )
    .bind(&new_hash)
    .bind(auth.user_id)
    .execute(&pool)
    .await?;

    // Trigger approval workflow
    // Creates approval records in all user's projects and sets memberships to pending
    approval_service::create_approval_for_all_projects(&pool, auth.user_id, "password_change")
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Password changed successfully. Your account requires approval to continue.",
        "requires_approval": true
    })))
}

async fn update_profile(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserResponse>> {
    // Trim and normalize display_name (empty string becomes NULL)
    let display_name = req
        .display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Update display_name
    sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
        .bind(&display_name)
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    // Fetch and return updated user
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(auth.user_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(UserResponse::from(user)))
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

    let owned_projects: Vec<OwnedProject> =
        sqlx::query_as("SELECT id, name FROM projects WHERE created_by = ?")
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

async fn get_preferences(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<UserPreferences>> {
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(auth.user_id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(UserPreferences::from_user(&user)))
}

fn validate_preferences(req: &UpdatePreferencesRequest) -> Result<(), AppError> {
    if let Some(ref df) = req.date_format {
        if !["mdy", "dmy", "ymd", "iso"].contains(&df.as_str()) {
            return Err(AppError::Validation(
                "Invalid date format. Supported: mdy, dmy, ymd, iso".to_string(),
            ));
        }
    }
    if let Some(ref sep) = req.decimal_separator {
        if ![".", ","].contains(&sep.as_str()) {
            return Err(AppError::Validation(
                "Invalid decimal separator. Supported: . ,".to_string(),
            ));
        }
    }
    if let Some(ref pos) = req.currency_symbol_position {
        if !["before", "after"].contains(&pos.as_str()) {
            return Err(AppError::Validation(
                "Invalid currency position. Supported: before, after".to_string(),
            ));
        }
    }
    // currency_symbol is freeform, no validation needed
    Ok(())
}

async fn update_preferences(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(req): Json<UpdatePreferencesRequest>,
) -> AppResult<Json<UserPreferences>> {
    // Validate input
    validate_preferences(&req)?;

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut bindings: Vec<Option<String>> = Vec::new();

    if let Some(ref v) = req.date_format {
        updates.push("date_format = ?");
        bindings.push(Some(v.clone()));
    }
    if let Some(ref v) = req.decimal_separator {
        updates.push("decimal_separator = ?");
        bindings.push(Some(v.clone()));
    }
    if let Some(ref v) = req.currency_symbol {
        updates.push("currency_symbol = ?");
        bindings.push(Some(v.clone()));
    }
    if let Some(ref v) = req.currency_symbol_position {
        updates.push("currency_symbol_position = ?");
        bindings.push(Some(v.clone()));
    }

    if !updates.is_empty() {
        let query = format!("UPDATE users SET {} WHERE id = ?", updates.join(", "));
        let mut q = sqlx::query(&query);
        for binding in bindings {
            q = q.bind(binding);
        }
        q = q.bind(auth.user_id);
        q.execute(&pool).await?;
    }

    // Fetch and return updated preferences
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(auth.user_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(UserPreferences::from_user(&user)))
}

/// Check if user is a system admin (user ID 1 for now)
fn is_system_admin(user_id: i64) -> bool {
    user_id == 1
}

async fn approve_user(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Path(user_id): Path<i64>,
) -> AppResult<Json<UserResponse>> {
    // Check if requester is admin
    if !is_system_admin(auth.user_id) {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    // Update user state to active
    sqlx::query("UPDATE users SET user_state = ? WHERE id = ?")
        .bind(UserState::Active.as_str())
        .bind(user_id)
        .execute(&pool)
        .await?;

    // Fetch updated user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(UserResponse::from(user)))
}

async fn revoke_user(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Path(user_id): Path<i64>,
) -> AppResult<Json<UserResponse>> {
    // Check if requester is admin
    if !is_system_admin(auth.user_id) {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    // Prevent revoking yourself
    if auth.user_id == user_id {
        return Err(AppError::Forbidden(
            "Cannot revoke your own account".to_string(),
        ));
    }

    // Update user state to revoked and increment token_version to invalidate tokens
    sqlx::query("UPDATE users SET user_state = ?, token_version = token_version + 1 WHERE id = ?")
        .bind(UserState::Revoked.as_str())
        .bind(user_id)
        .execute(&pool)
        .await?;

    // Fetch updated user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&pool)
        .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(UserResponse::from(user)))
}
