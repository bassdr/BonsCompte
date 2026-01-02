use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    auth::{AdminMember, ProjectMember},
    error::{AppError, AppResult},
    models::{CreateParticipant, EntityType, Participant, UpdateParticipant},
    services::HistoryService,
    AppState,
};

#[derive(Deserialize)]
struct ParticipantPath {
    participant_id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_participants).post(create_participant))
        .route(
            "/{participant_id}",
            get(get_participant)
                .put(update_participant)
                .delete(delete_participant),
        )
        .route("/{participant_id}/claim", post(claim_participant))
        .route(
            "/{participant_id}/invite",
            post(create_invite).get(get_invite).delete(revoke_invite),
        )
}

async fn list_participants(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<Participant>>> {
    let participants: Vec<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE project_id = ? ORDER BY name")
            .bind(member.project_id)
            .fetch_all(&pool)
            .await?;

    Ok(Json(participants))
}

async fn get_participant(
    Path(path): Path<ParticipantPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Participant>> {
    let participant: Option<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    participant
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Participant not found".to_string()))
}

async fn create_participant(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreateParticipant>,
) -> AppResult<Json<Participant>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::Forbidden("Editor access required".to_string()));
    }

    let default_weight = input.default_weight.unwrap_or(1.0);
    let account_type = input.account_type.as_deref().unwrap_or("user");

    // Validate account_type
    if account_type != "user" && account_type != "pool" {
        return Err(AppError::BadRequest(
            "account_type must be 'user' or 'pool'".to_string(),
        ));
    }

    let result = sqlx::query(
        "INSERT INTO participants (project_id, name, default_weight, account_type) VALUES (?, ?, ?, ?)"
    )
    .bind(member.project_id)
    .bind(&input.name)
    .bind(default_weight)
    .bind(account_type)
    .execute(&pool)
    .await?;

    let participant: Participant = sqlx::query_as("SELECT * FROM participants WHERE id = ?")
        .bind(result.last_insert_rowid())
        .fetch_one(&pool)
        .await?;

    // Log the creation to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_create(
        &pool,
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::Participant,
        participant.id,
        &participant,
    )
    .await;

    Ok(Json(participant))
}

async fn update_participant(
    Path(path): Path<ParticipantPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<UpdateParticipant>,
) -> AppResult<Json<Participant>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::Forbidden("Editor access required".to_string()));
    }

    // Verify participant belongs to project
    let existing: Option<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    let existing =
        existing.ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    // Validate account_type if provided
    if let Some(ref account_type) = input.account_type {
        if account_type != "user" && account_type != "pool" {
            return Err(AppError::BadRequest(
                "account_type must be 'user' or 'pool'".to_string(),
            ));
        }

        // If changing to pool, check that user is not linked
        if account_type == "pool" && existing.account_type != "pool" {
            // Prevent linked users from becoming pools
            if existing.user_id.is_some() {
                return Err(AppError::BadRequest(
                    "Linked users cannot become pool accounts".to_string(),
                ));
            }
        }
    }

    // Build dynamic update
    let mut updates = Vec::new();
    let mut has_name = false;
    let mut has_weight = false;
    let mut has_account_type = false;
    let mut name_val = String::new();
    let mut weight_val = 0.0f64;
    let mut account_type_val = String::new();

    if let Some(name) = &input.name {
        updates.push("name = ?");
        has_name = true;
        name_val = name.clone();
    }
    if let Some(weight) = input.default_weight {
        updates.push("default_weight = ?");
        has_weight = true;
        weight_val = weight;
    }
    if let Some(account_type) = &input.account_type {
        updates.push("account_type = ?");
        has_account_type = true;
        account_type_val = account_type.clone();
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let sql = format!(
        "UPDATE participants SET {} WHERE id = ?",
        updates.join(", ")
    );
    let mut query = sqlx::query(&sql);
    if has_name {
        query = query.bind(&name_val);
    }
    if has_weight {
        query = query.bind(weight_val);
    }
    if has_account_type {
        query = query.bind(&account_type_val);
    }
    query = query.bind(path.participant_id);
    query.execute(&pool).await?;

    let participant: Participant = sqlx::query_as("SELECT * FROM participants WHERE id = ?")
        .bind(path.participant_id)
        .fetch_one(&pool)
        .await?;

    // Log the update to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_update(
        &pool,
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::Participant,
        path.participant_id,
        &existing,
        &participant,
    )
    .await;

    Ok(Json(participant))
}

async fn delete_participant(
    Path(path): Path<ParticipantPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    let member = admin.0;

    // Capture before state for history
    let existing: Option<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    let existing =
        existing.ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    let result = sqlx::query("DELETE FROM participants WHERE id = ? AND project_id = ?")
        .bind(path.participant_id)
        .bind(member.project_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Participant not found".to_string()));
    }

    // Log the deletion to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_delete(
        &pool,
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::Participant,
        path.participant_id,
        &existing,
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn claim_participant(
    Path(path): Path<ParticipantPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Participant>> {
    // Verify participant belongs to project and has no user_id
    let participant: Option<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    let participant =
        participant.ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    if participant.user_id.is_some() {
        return Err(AppError::BadRequest(
            "Participant already claimed".to_string(),
        ));
    }

    // Check if user already has a participant in this project
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM participants WHERE project_id = ? AND user_id = ?")
            .bind(member.project_id)
            .bind(member.user_id)
            .fetch_optional(&pool)
            .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "You already have a participant in this project".to_string(),
        ));
    }

    // Start transaction
    let mut tx = pool.begin().await?;

    // Update participant with user_id
    sqlx::query("UPDATE participants SET user_id = ? WHERE id = ?")
        .bind(member.user_id)
        .bind(path.participant_id)
        .execute(&mut *tx)
        .await?;

    // Update project_member to link to this participant
    sqlx::query(
        "UPDATE project_members SET participant_id = ? WHERE project_id = ? AND user_id = ?",
    )
    .bind(path.participant_id)
    .bind(member.project_id)
    .bind(member.user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let updated: Participant = sqlx::query_as("SELECT * FROM participants WHERE id = ?")
        .bind(path.participant_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(updated))
}

fn generate_invite_token() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[derive(Serialize, sqlx::FromRow)]
struct ParticipantInviteResponse {
    id: i64,
    participant_id: i64,
    invite_token: String,
    created_at: String,
    expires_at: Option<String>,
    used_by: Option<i64>,
    used_at: Option<String>,
}

async fn create_invite(
    Path(path): Path<ParticipantPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<ParticipantInviteResponse>> {
    let member = admin.0;

    // Verify participant belongs to project
    let participant: Option<Participant> =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    let participant =
        participant.ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    // Check if participant already has a user
    if participant.user_id.is_some() {
        return Err(AppError::BadRequest(
            "Participant already linked to a user".to_string(),
        ));
    }

    // Delete any existing invite for this participant
    sqlx::query("DELETE FROM participant_invites WHERE participant_id = ?")
        .bind(path.participant_id)
        .execute(&pool)
        .await?;

    // Generate new invite
    let token = generate_invite_token();

    sqlx::query(
        "INSERT INTO participant_invites (project_id, participant_id, invite_token) VALUES (?, ?, ?)"
    )
    .bind(member.project_id)
    .bind(path.participant_id)
    .bind(&token)
    .execute(&pool)
    .await?;

    let invite: ParticipantInviteResponse = sqlx::query_as(
        "SELECT id, participant_id, invite_token, created_at, expires_at, used_by, used_at
         FROM participant_invites WHERE participant_id = ?",
    )
    .bind(path.participant_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(invite))
}

async fn get_invite(
    Path(path): Path<ParticipantPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<ParticipantInviteResponse>> {
    // Verify participant belongs to project
    let _participant: Participant =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    let invite: Option<ParticipantInviteResponse> = sqlx::query_as(
        "SELECT id, participant_id, invite_token, created_at, expires_at, used_by, used_at
         FROM participant_invites WHERE participant_id = ?",
    )
    .bind(path.participant_id)
    .fetch_optional(&pool)
    .await?;

    invite
        .map(Json)
        .ok_or_else(|| AppError::NotFound("No invite found for this participant".to_string()))
}

async fn revoke_invite(
    Path(path): Path<ParticipantPath>,
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    let member = admin.0;

    // Verify participant belongs to project
    let _participant: Participant =
        sqlx::query_as("SELECT * FROM participants WHERE id = ? AND project_id = ?")
            .bind(path.participant_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Participant not found".to_string()))?;

    let result = sqlx::query("DELETE FROM participant_invites WHERE participant_id = ?")
        .bind(path.participant_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("No invite found".to_string()));
    }

    Ok(Json(serde_json::json!({ "revoked": true })))
}
