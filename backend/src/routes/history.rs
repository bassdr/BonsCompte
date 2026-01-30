use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;

use crate::{
    auth::ProjectMember,
    error::{AppError, AppResult, ErrorCode},
    models::{ChainVerification, HistoryEntryResponse, HistoryQuery, UndoRequest},
    services::{history::LogEventParams, HistoryService},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_project_history))
        .route("/verify", get(verify_chain))
        .route("/{entity_type}/{entity_id}", get(get_entity_history))
        .route("/{history_id}/undo", post(undo_action))
}

/// GET /projects/{id}/history
/// Get paginated history entries for the project
async fn get_project_history(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Query(query): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryEntryResponse>>> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    let entries = HistoryService::get_project_history(
        &pool,
        member.project_id,
        limit,
        offset,
        query.entity_type.as_deref(),
    )
    .await?;

    let responses = HistoryService::resolve_actor_names(&pool, entries).await?;

    Ok(Json(responses))
}

/// GET /projects/{id}/history/{entity_type}/{entity_id}
/// Get history entries for a specific entity
#[derive(serde::Deserialize)]
struct EntityPath {
    entity_type: String,
    entity_id: i64,
}

async fn get_entity_history(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Path(path): Path<EntityPath>,
) -> AppResult<Json<Vec<HistoryEntryResponse>>> {
    let entries = HistoryService::get_entity_history(
        &pool,
        member.project_id,
        &path.entity_type,
        path.entity_id,
    )
    .await?;

    let responses = HistoryService::resolve_actor_names(&pool, entries).await?;

    Ok(Json(responses))
}

/// POST /projects/{id}/history/{history_id}/undo
/// Undo a history entry (admin only)
#[derive(serde::Deserialize)]
struct UndoPath {
    history_id: i64,
}

async fn undo_action(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Path(path): Path<UndoPath>,
    Json(request): Json<UndoRequest>,
) -> AppResult<Json<HistoryEntryResponse>> {
    // Only admins can undo
    if !member.is_admin() {
        return Err(AppError::Forbidden(
            "Only admins can undo actions".to_string(),
        ));
    }

    // Get the original entry
    let entry = HistoryService::get_entry(&pool, path.history_id)
        .await?
        .ok_or_else(|| AppError::not_found(ErrorCode::NotFound))?;

    // Verify it belongs to this project
    if entry.project_id != Some(member.project_id) {
        return Err(AppError::not_found(ErrorCode::NotFound));
    }

    // Check if already undone
    if HistoryService::is_entry_undone(&pool, path.history_id).await? {
        return Err(AppError::BadRequest(
            "This action has already been undone".to_string(),
        ));
    }

    // Cannot undo an UNDO action
    if entry.action == "UNDO" {
        return Err(AppError::BadRequest(
            "Cannot undo an UNDO action".to_string(),
        ));
    }

    // Perform the undo based on entity type and action
    let undo_history_id = perform_undo(&pool, &member, &entry, request.reason.as_deref()).await?;

    // Get the new UNDO entry
    let undo_entry = HistoryService::get_entry(&pool, undo_history_id)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to retrieve undo entry".to_string()))?;

    let responses = HistoryService::resolve_actor_names(&pool, vec![undo_entry]).await?;
    let response = responses.into_iter().next().unwrap();

    Ok(Json(response))
}

/// Perform the actual undo operation based on entity type and action
async fn perform_undo(
    pool: &SqlitePool,
    member: &ProjectMember,
    entry: &crate::models::HistoryEntry,
    reason: Option<&str>,
) -> AppResult<i64> {
    let correlation_id = HistoryService::new_correlation_id();
    let entity_id = entry
        .entity_id
        .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

    match entry.entity_type.as_str() {
        "payment" => undo_payment(pool, member, entry, entity_id, &correlation_id, reason).await,
        "participant" => {
            undo_participant(pool, member, entry, entity_id, &correlation_id, reason).await
        }
        "project_member" => {
            undo_project_member(pool, member, entry, entity_id, &correlation_id, reason).await
        }
        "project" => undo_project(pool, member, entry, entity_id, &correlation_id, reason).await,
        _ => Err(AppError::bad_request(ErrorCode::InvalidInput)),
    }
}

/// Undo a payment action
async fn undo_payment(
    pool: &SqlitePool,
    member: &ProjectMember,
    entry: &crate::models::HistoryEntry,
    entity_id: i64,
    correlation_id: &str,
    reason: Option<&str>,
) -> AppResult<i64> {
    match entry.action.as_str() {
        "CREATE" => {
            // Undo create = delete the payment
            sqlx::query("DELETE FROM contributions WHERE payment_id = ?")
                .bind(entity_id)
                .execute(pool)
                .await?;

            sqlx::query("DELETE FROM payments WHERE id = ? AND project_id = ?")
                .bind(entity_id)
                .bind(member.project_id)
                .execute(pool)
                .await?;

            // Log the undo
            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "payment",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: None,
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        "UPDATE" => {
            // Undo update = restore the before state
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            // Update payment with before values
            sqlx::query(
                r#"
                UPDATE payments SET
                    payer_id = ?,
                    amount = ?,
                    description = ?,
                    payment_date = ?,
                    receipt_image = ?,
                    is_recurring = ?,
                    recurrence_type = ?,
                    recurrence_interval = ?,
                    recurrence_end_date = ?,
                    receiver_account_id = ?
                WHERE id = ? AND project_id = ?
                "#,
            )
            .bind(before.get("payer_id").and_then(|v| v.as_i64()))
            .bind(before.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(
                before
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )
            .bind(before.get("payment_date").and_then(|v| v.as_str()))
            .bind(before.get("receipt_image").and_then(|v| v.as_str()))
            .bind(
                before
                    .get("is_recurring")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )
            .bind(before.get("recurrence_type").and_then(|v| v.as_str()))
            .bind(
                before
                    .get("recurrence_interval")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
            )
            .bind(before.get("recurrence_end_date").and_then(|v| v.as_str()))
            .bind(before.get("receiver_account_id").and_then(|v| v.as_i64()))
            .bind(entity_id)
            .bind(member.project_id)
            .execute(pool)
            .await?;

            // Log the undo
            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "payment",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        "DELETE" => {
            // Undo delete = recreate the payment from before state
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            // Get the payment data from the nested "payment" field (PaymentWithContributions structure)
            let payment_data = before.get("payment").unwrap_or(&before);

            // Recreate payment (using original ID if possible via INSERT OR REPLACE)
            sqlx::query(
                r#"
                INSERT INTO payments (
                    id, project_id, payer_id, amount, description, payment_date,
                    receipt_image, is_recurring, recurrence_type, recurrence_interval,
                    recurrence_end_date, recurrence_weekdays, recurrence_monthdays,
                    recurrence_months, receiver_account_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(entity_id)
            .bind(member.project_id)
            .bind(payment_data.get("payer_id").and_then(|v| v.as_i64()))
            .bind(
                payment_data
                    .get("amount")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            )
            .bind(
                payment_data
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )
            .bind(payment_data.get("payment_date").and_then(|v| v.as_str()))
            .bind(payment_data.get("receipt_image").and_then(|v| v.as_str()))
            .bind(
                payment_data
                    .get("is_recurring")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )
            .bind(payment_data.get("recurrence_type").and_then(|v| v.as_str()))
            .bind(
                payment_data
                    .get("recurrence_interval")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
            )
            .bind(
                payment_data
                    .get("recurrence_end_date")
                    .and_then(|v| v.as_str()),
            )
            .bind(
                payment_data
                    .get("recurrence_weekdays")
                    .and_then(|v| v.as_str()),
            )
            .bind(
                payment_data
                    .get("recurrence_monthdays")
                    .and_then(|v| v.as_str()),
            )
            .bind(
                payment_data
                    .get("recurrence_months")
                    .and_then(|v| v.as_str()),
            )
            .bind(
                payment_data
                    .get("receiver_account_id")
                    .and_then(|v| v.as_i64()),
            )
            .execute(pool)
            .await?;

            // Restore contributions if they exist in the before state
            if let Some(contributions) = before.get("contributions").and_then(|v| v.as_array()) {
                for contrib in contributions {
                    let participant_id = contrib.get("participant_id").and_then(|v| v.as_i64());
                    let amount = contrib
                        .get("amount")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let weight = contrib
                        .get("weight")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0);

                    if let Some(pid) = participant_id {
                        sqlx::query(
                            "INSERT INTO contributions (participant_id, payment_id, amount, weight) VALUES (?, ?, ?, ?)"
                        )
                        .bind(pid)
                        .bind(entity_id)
                        .bind(amount)
                        .bind(weight)
                        .execute(pool)
                        .await?;
                    }
                }
            }

            // Log the undo
            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "payment",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: None,
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        _ => Err(AppError::bad_request(ErrorCode::InvalidInput)),
    }
}

/// Undo a participant action
async fn undo_participant(
    pool: &SqlitePool,
    member: &ProjectMember,
    entry: &crate::models::HistoryEntry,
    entity_id: i64,
    correlation_id: &str,
    reason: Option<&str>,
) -> AppResult<i64> {
    match entry.action.as_str() {
        "CREATE" => {
            // Undo create = delete the participant
            sqlx::query("DELETE FROM participants WHERE id = ? AND project_id = ?")
                .bind(entity_id)
                .bind(member.project_id)
                .execute(pool)
                .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "participant",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: None,
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        "UPDATE" => {
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            sqlx::query(
                r#"
                UPDATE participants SET
                    name = ?,
                    default_weight = ?,
                    account_type = ?
                WHERE id = ? AND project_id = ?
                "#,
            )
            .bind(before.get("name").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(
                before
                    .get("default_weight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0),
            )
            .bind(
                before
                    .get("account_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("user"),
            )
            .bind(entity_id)
            .bind(member.project_id)
            .execute(pool)
            .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "participant",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        "DELETE" => {
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            sqlx::query(
                r#"
                INSERT INTO participants (id, project_id, name, user_id, default_weight, account_type)
                VALUES (?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(entity_id)
            .bind(member.project_id)
            .bind(before.get("name").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(before.get("user_id").and_then(|v| v.as_i64()))
            .bind(before.get("default_weight").and_then(|v| v.as_f64()).unwrap_or(1.0))
            .bind(before.get("account_type").and_then(|v| v.as_str()).unwrap_or("user"))
            .execute(pool)
            .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "participant",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: None,
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        _ => Err(AppError::bad_request(ErrorCode::InvalidInput)),
    }
}

/// Undo a project member action
async fn undo_project_member(
    pool: &SqlitePool,
    member: &ProjectMember,
    entry: &crate::models::HistoryEntry,
    entity_id: i64,
    correlation_id: &str,
    reason: Option<&str>,
) -> AppResult<i64> {
    match entry.action.as_str() {
        "UPDATE" => {
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            // entity_id is the user_id in project_members
            sqlx::query(
                r#"
                UPDATE project_members SET
                    role = ?,
                    participant_id = ?,
                    status = ?
                WHERE user_id = ? AND project_id = ?
                "#,
            )
            .bind(
                before
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("editor"),
            )
            .bind(before.get("participant_id").and_then(|v| v.as_i64()))
            .bind(
                before
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("active"),
            )
            .bind(entity_id)
            .bind(member.project_id)
            .execute(pool)
            .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "project_member",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        "DELETE" => {
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            sqlx::query(
                r#"
                INSERT INTO project_members (project_id, user_id, role, participant_id, status)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(member.project_id)
            .bind(entity_id)
            .bind(
                before
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("editor"),
            )
            .bind(before.get("participant_id").and_then(|v| v.as_i64()))
            .bind(
                before
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("active"),
            )
            .execute(pool)
            .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "project_member",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: None,
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        _ => Err(AppError::bad_request(ErrorCode::InvalidInput)),
    }
}

/// Undo a project action
async fn undo_project(
    pool: &SqlitePool,
    member: &ProjectMember,
    entry: &crate::models::HistoryEntry,
    entity_id: i64,
    correlation_id: &str,
    reason: Option<&str>,
) -> AppResult<i64> {
    match entry.action.as_str() {
        "UPDATE" => {
            let before: serde_json::Value = entry
                .payload_before
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or_else(|| AppError::bad_request(ErrorCode::InvalidInput))?;

            sqlx::query(
                r#"
                UPDATE projects SET
                    name = ?,
                    description = ?,
                    invites_enabled = ?,
                    require_approval = ?
                WHERE id = ?
                "#,
            )
            .bind(before.get("name").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(before.get("description").and_then(|v| v.as_str()))
            .bind(
                before
                    .get("invites_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            )
            .bind(
                before
                    .get("require_approval")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )
            .bind(entity_id)
            .execute(pool)
            .await?;

            HistoryService::log_event(
                pool,
                LogEventParams {
                    correlation_id,
                    actor_user_id: Some(member.user_id),
                    project_id: Some(member.project_id),
                    entity_type: "project",
                    entity_id: Some(entity_id),
                    action: "UNDO",
                    payload_before: entry.payload_after.as_deref(),
                    payload_after: entry.payload_before.as_deref(),
                    reason,
                    undoes_history_id: Some(entry.id),
                },
            )
            .await
        }
        _ => Err(AppError::bad_request(ErrorCode::InvalidInput)),
    }
}

/// GET /projects/{id}/history/verify
/// Verify the hash chain integrity (admin only)
async fn verify_chain(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<ChainVerification>> {
    if !member.is_admin() {
        return Err(AppError::Forbidden(
            "Only admins can verify chain".to_string(),
        ));
    }

    let verification = HistoryService::verify_chain(&pool).await?;
    Ok(Json(verification))
}
