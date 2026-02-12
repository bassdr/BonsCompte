use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::{
    auth::ProjectMember,
    error::{AppError, AppResult, ErrorCode},
    AppState,
};

#[derive(Deserialize)]
struct OverridePath {
    override_id: i64,
}

#[derive(Deserialize)]
struct BudgetQuery {
    participant_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct BudgetOverride {
    pub id: i64,
    pub project_id: i64,
    pub participant_id: Option<i64>,
    pub tag: Option<String>,
    pub name: String,
    pub yearly_amount: f64,
    pub override_type: String,
    pub linked_payment_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudgetOverride {
    pub participant_id: Option<i64>,
    pub tag: Option<String>,
    pub name: String,
    pub yearly_amount: f64,
    pub override_type: String,
    pub linked_payment_id: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_budget))
        .route("/overrides", get(list_overrides).post(create_override))
        .route(
            "/overrides/{override_id}",
            get(get_override).put(update_override).delete(delete_override),
        )
}

/// Get budget summary (budgeted vs observed)
async fn get_budget(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Query(query): Query<BudgetQuery>,
) -> AppResult<Json<crate::services::BudgetResponse>> {
    let budget = crate::services::calculate_budget(&pool, member.project_id, query.participant_id).await?;
    Ok(Json(budget))
}

/// List all budget overrides for a project
async fn list_overrides(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<BudgetOverride>>> {
    let overrides: Vec<BudgetOverride> = sqlx::query_as(
        "SELECT * FROM budget_overrides WHERE project_id = ? ORDER BY created_at DESC",
    )
    .bind(member.project_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(overrides))
}

/// Get a single budget override
async fn get_override(
    Path(path): Path<OverridePath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<BudgetOverride>> {
    let override_entry: Option<BudgetOverride> =
        sqlx::query_as("SELECT * FROM budget_overrides WHERE id = ? AND project_id = ?")
            .bind(path.override_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    override_entry
        .map(Json)
        .ok_or_else(|| AppError::not_found(ErrorCode::NotFound))
}

/// Create a new budget override
async fn create_override(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreateBudgetOverride>,
) -> AppResult<Json<BudgetOverride>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::forbidden(ErrorCode::EditorRequired));
    }

    // Validate override_type
    if !["add", "adjust", "exclude"].contains(&input.override_type.as_str()) {
        return Err(AppError::bad_request(ErrorCode::InvalidInput));
    }

    // Validate participant belongs to project if specified
    if let Some(participant_id) = input.participant_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM participants WHERE id = ? AND project_id = ?")
                .bind(participant_id)
                .bind(member.project_id)
                .fetch_optional(&pool)
                .await?;

        if exists.is_none() {
            return Err(AppError::bad_request(ErrorCode::InvalidParticipant));
        }
    }

    // Validate linked_payment_id belongs to project if specified
    if let Some(payment_id) = input.linked_payment_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM payments WHERE id = ? AND project_id = ?")
                .bind(payment_id)
                .bind(member.project_id)
                .fetch_optional(&pool)
                .await?;

        if exists.is_none() {
            return Err(AppError::bad_request(ErrorCode::PaymentNotFound));
        }
    }

    let result = sqlx::query(
        "INSERT INTO budget_overrides (project_id, participant_id, tag, name, yearly_amount, override_type, linked_payment_id)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(member.project_id)
    .bind(input.participant_id)
    .bind(&input.tag)
    .bind(&input.name)
    .bind(input.yearly_amount)
    .bind(&input.override_type)
    .bind(input.linked_payment_id)
    .execute(&pool)
    .await?;

    let override_id = result.last_insert_rowid();

    // Fetch and return the created override
    let created: BudgetOverride = sqlx::query_as("SELECT * FROM budget_overrides WHERE id = ?")
        .bind(override_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(created))
}

/// Update a budget override
async fn update_override(
    Path(path): Path<OverridePath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreateBudgetOverride>,
) -> AppResult<Json<BudgetOverride>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::forbidden(ErrorCode::EditorRequired));
    }

    // Verify override exists and belongs to project
    let existing: Option<BudgetOverride> =
        sqlx::query_as("SELECT * FROM budget_overrides WHERE id = ? AND project_id = ?")
            .bind(path.override_id)
            .bind(member.project_id)
            .fetch_optional(&pool)
            .await?;

    if existing.is_none() {
        return Err(AppError::not_found(ErrorCode::NotFound));
    }

    // Validate override_type
    if !["add", "adjust", "exclude"].contains(&input.override_type.as_str()) {
        return Err(AppError::bad_request(ErrorCode::InvalidInput));
    }

    // Validate participant if specified
    if let Some(participant_id) = input.participant_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM participants WHERE id = ? AND project_id = ?")
                .bind(participant_id)
                .bind(member.project_id)
                .fetch_optional(&pool)
                .await?;

        if exists.is_none() {
            return Err(AppError::bad_request(ErrorCode::InvalidParticipant));
        }
    }

    // Validate linked_payment_id if specified
    if let Some(payment_id) = input.linked_payment_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM payments WHERE id = ? AND project_id = ?")
                .bind(payment_id)
                .bind(member.project_id)
                .fetch_optional(&pool)
                .await?;

        if exists.is_none() {
            return Err(AppError::bad_request(ErrorCode::PaymentNotFound));
        }
    }

    sqlx::query(
        "UPDATE budget_overrides SET participant_id = ?, tag = ?, name = ?, yearly_amount = ?, override_type = ?, linked_payment_id = ?
         WHERE id = ? AND project_id = ?",
    )
    .bind(input.participant_id)
    .bind(&input.tag)
    .bind(&input.name)
    .bind(input.yearly_amount)
    .bind(&input.override_type)
    .bind(input.linked_payment_id)
    .bind(path.override_id)
    .bind(member.project_id)
    .execute(&pool)
    .await?;

    // Fetch and return the updated override
    let updated: BudgetOverride = sqlx::query_as("SELECT * FROM budget_overrides WHERE id = ?")
        .bind(path.override_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(updated))
}

/// Delete a budget override
async fn delete_override(
    Path(path): Path<OverridePath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::forbidden(ErrorCode::EditorRequired));
    }

    let result = sqlx::query("DELETE FROM budget_overrides WHERE id = ? AND project_id = ?")
        .bind(path.override_id)
        .bind(member.project_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found(ErrorCode::NotFound));
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
