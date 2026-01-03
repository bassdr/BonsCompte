use axum::{
    extract::{Path, State},
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    auth::{AdminMember, ProjectMember},
    error::{AppError, AppResult},
    models::{EntityType, ProjectMemberResponse, Role, SetMemberParticipant, UpdateMemberRole},
    services::HistoryService,
    AppState,
};

#[derive(Deserialize)]
struct MemberPath {
    user_id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_members))
        .route("/pending", get(list_pending_members))
        .route(
            "/{user_id}",
            get(get_member)
                .put(update_member_role)
                .delete(remove_member),
        )
        .route("/{user_id}/participant", put(set_member_participant))
        .route("/{user_id}/approve", put(approve_member))
        .route("/{user_id}/reject", put(reject_member))
}

async fn list_members(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ProjectMemberResponse>>> {
    let members: Vec<ProjectMemberResponse> = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.status = 'active'
         ORDER BY pm.joined_at"
    )
    .bind(member.project_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(members))
}

async fn list_pending_members(
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ProjectMemberResponse>>> {
    let member = admin.0;
    let members: Vec<ProjectMemberResponse> = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.status = 'pending'
         ORDER BY pm.joined_at"
    )
    .bind(member.project_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(members))
}

async fn get_member(
    Path(path): Path<MemberPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<ProjectMemberResponse>> {
    let target: Option<ProjectMemberResponse> = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_optional(&pool)
    .await?;

    target
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Member not found".to_string()))
}

async fn update_member_role(
    Path(path): Path<MemberPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<UpdateMemberRole>,
) -> AppResult<Json<ProjectMemberResponse>> {
    let member = admin.0;

    // Can't change own role (prevent admin lockout)
    if path.user_id == member.user_id {
        return Err(AppError::BadRequest(
            "Cannot change your own role".to_string(),
        ));
    }

    // Validate role
    let new_role = input.role.parse::<Role>()
        .map_err(|_| AppError::BadRequest("Invalid role".to_string()))?;

    // Capture before state
    let before: Option<ProjectMemberResponse> = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_optional(&pool)
    .await?;

    let before = before.ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;

    sqlx::query("UPDATE project_members SET role = ? WHERE project_id = ? AND user_id = ?")
        .bind(new_role.as_str())
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&pool)
        .await?;

    let updated: ProjectMemberResponse = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_one(&pool)
    .await?;

    // Log the update to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_update(
        &pool,
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::ProjectMember,
        path.user_id,
        &before,
        &updated,
    )
    .await;

    Ok(Json(updated))
}

async fn remove_member(
    Path(path): Path<MemberPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    let member = admin.0;

    // Can't remove yourself
    if path.user_id == member.user_id {
        return Err(AppError::BadRequest("Cannot remove yourself".to_string()));
    }

    // Capture before state for history
    let before: Option<ProjectMemberResponse> = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_optional(&pool)
    .await?;

    let before = before.ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;

    let result = sqlx::query("DELETE FROM project_members WHERE project_id = ? AND user_id = ?")
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Member not found".to_string()));
    }

    // Also unlink user from any participant they claimed
    sqlx::query("UPDATE participants SET user_id = NULL WHERE project_id = ? AND user_id = ?")
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&pool)
        .await?;

    // Log the deletion to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_delete(
        &pool,
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::ProjectMember,
        path.user_id,
        &before,
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn set_member_participant(
    Path(path): Path<MemberPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<SetMemberParticipant>,
) -> AppResult<Json<ProjectMemberResponse>> {
    let member = admin.0;

    // Verify target is a member
    let target_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM project_members WHERE project_id = ? AND user_id = ?")
            .bind(member.project_id)
            .bind(path.user_id)
            .fetch_optional(&pool)
            .await?;

    if target_exists.is_none() {
        return Err(AppError::NotFound("Member not found".to_string()));
    }

    // If setting a participant, verify it belongs to project
    if let Some(participant_id) = input.participant_id {
        let participant_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM participants WHERE id = ? AND project_id = ?")
                .bind(participant_id)
                .bind(member.project_id)
                .fetch_optional(&pool)
                .await?;

        if participant_exists.is_none() {
            return Err(AppError::BadRequest("Invalid participant".to_string()));
        }

        // Check if participant is already claimed by another user
        let already_claimed: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM participants WHERE id = ? AND user_id IS NOT NULL AND user_id != ?"
        )
        .bind(participant_id)
        .bind(path.user_id)
        .fetch_optional(&pool)
        .await?;

        if already_claimed.is_some() {
            return Err(AppError::BadRequest(
                "Participant already claimed by another user".to_string(),
            ));
        }
    }

    // Start transaction
    let mut tx = pool.begin().await?;

    // Clear previous participant link for this user
    sqlx::query("UPDATE participants SET user_id = NULL WHERE project_id = ? AND user_id = ?")
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&mut *tx)
        .await?;

    // Update project_member
    sqlx::query(
        "UPDATE project_members SET participant_id = ? WHERE project_id = ? AND user_id = ?",
    )
    .bind(input.participant_id)
    .bind(member.project_id)
    .bind(path.user_id)
    .execute(&mut *tx)
    .await?;

    // If setting a participant, also set user_id on participant
    if let Some(participant_id) = input.participant_id {
        sqlx::query("UPDATE participants SET user_id = ? WHERE id = ?")
            .bind(path.user_id)
            .bind(participant_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let updated: ProjectMemberResponse = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(updated))
}

async fn approve_member(
    Path(path): Path<MemberPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<ProjectMemberResponse>> {
    let member = admin.0;

    // Verify target is a pending member
    let target_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM project_members WHERE project_id = ? AND user_id = ?",
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_optional(&pool)
    .await?;

    match target_status.as_deref() {
        None => return Err(AppError::NotFound("Member not found".to_string())),
        Some("active") => return Err(AppError::BadRequest("Member is already active".to_string())),
        Some("pending") => {} // OK to proceed
        Some(s) => {
            return Err(AppError::BadRequest(format!(
                "Cannot approve member with status: {}",
                s
            )))
        }
    }

    sqlx::query(
        "UPDATE project_members SET status = 'active' WHERE project_id = ? AND user_id = ?",
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .execute(&pool)
    .await?;

    let updated: ProjectMemberResponse = sqlx::query_as(
        "SELECT pm.id, pm.project_id, pm.user_id, u.username, u.display_name, pm.role, pm.participant_id, p.name as participant_name, pm.joined_at, pm.status
         FROM project_members pm
         JOIN users u ON pm.user_id = u.id
         LEFT JOIN participants p ON pm.participant_id = p.id
         WHERE pm.project_id = ? AND pm.user_id = ?"
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(updated))
}

async fn reject_member(
    Path(path): Path<MemberPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    let member = admin.0;

    // Verify target is a pending member
    let target_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM project_members WHERE project_id = ? AND user_id = ?",
    )
    .bind(member.project_id)
    .bind(path.user_id)
    .fetch_optional(&pool)
    .await?;

    match target_status.as_deref() {
        None => return Err(AppError::NotFound("Member not found".to_string())),
        Some("active") => {
            return Err(AppError::BadRequest(
                "Cannot reject an active member".to_string(),
            ))
        }
        Some("pending") => {} // OK to proceed
        Some(s) => {
            return Err(AppError::BadRequest(format!(
                "Cannot reject member with status: {}",
                s
            )))
        }
    }

    // Delete the pending member
    sqlx::query("DELETE FROM project_members WHERE project_id = ? AND user_id = ?")
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&pool)
        .await?;

    // Also unlink user from any participant they may have been linked to
    sqlx::query("UPDATE participants SET user_id = NULL WHERE project_id = ? AND user_id = ?")
        .bind(member.project_id)
        .bind(path.user_id)
        .execute(&pool)
        .await?;

    Ok(Json(serde_json::json!({ "rejected": true })))
}
