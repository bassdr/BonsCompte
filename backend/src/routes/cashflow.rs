use axum::{extract::{Query, State}, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::middleware::ProjectMember;
use crate::error::AppResult;
use crate::services::debt_calculator::CashflowProjection;
use crate::AppState;

#[derive(Deserialize)]
struct CashflowQuery {
    horizon_months: Option<u32>,          // Default: 6
    recommendation_mode: Option<String>,  // Default: "simple_average"
    frequency_type: Option<String>,       // Default: "monthly"
    frequency_interval: Option<i32>,      // Default: 1
    consolidate_mode: Option<bool>,       // Default: false
}

async fn get_cashflow(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Query(query): Query<CashflowQuery>,
) -> AppResult<Json<CashflowProjection>> {
    let horizon_months = query.horizon_months.unwrap_or(6).min(60); // Cap at 5 years
    let recommendation_mode = query.recommendation_mode.as_deref().unwrap_or("simple_average");
    let frequency_type = query.frequency_type.as_deref().unwrap_or("monthly");
    let frequency_interval = query.frequency_interval.unwrap_or(1).max(1);
    let consolidate_mode = query.consolidate_mode.unwrap_or(false);

    let projection = crate::services::debt_calculator::calculate_cashflow_projection(
        &pool,
        member.project_id,
        horizon_months,
        recommendation_mode,
        frequency_type,
        frequency_interval,
        consolidate_mode,
    ).await?;

    Ok(Json(projection))
}

#[derive(Deserialize)]
struct ConsolidateRequest {
    payer_id: i64,
    pool_id: i64,
    amount: f64,
    description: String,
    payment_date: String,
    recurrence_type: String,
    recurrence_interval: i32,
    payment_ids_to_delete: Vec<i64>,
}

#[derive(Serialize)]
struct ConsolidateResponse {
    deleted_count: usize,
    new_payment_id: i64,
}

async fn consolidate_payments(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(req): Json<ConsolidateRequest>,
) -> AppResult<Json<ConsolidateResponse>> {
    // Start transaction
    let mut tx = pool.begin().await?;

    // Verify all payments belong to this project and payer
    for payment_id in &req.payment_ids_to_delete {
        let payment: Option<(i64, i64)> = sqlx::query_as(
            "SELECT project_id, payer_id FROM payments WHERE id = ?"
        )
        .bind(payment_id)
        .fetch_optional(&mut *tx)
        .await?;

        match payment {
            Some((project_id, payer_id)) => {
                if project_id != member.project_id {
                    return Err(crate::error::AppError::Forbidden("Payment belongs to different project".to_string()));
                }
                if Some(payer_id) != Some(req.payer_id) {
                    return Err(crate::error::AppError::Forbidden("Payment belongs to different payer".to_string()));
                }
            }
            None => return Err(crate::error::AppError::NotFound(format!("Payment {} not found", payment_id))),
        }
    }

    // Delete old recurring payments
    for payment_id in &req.payment_ids_to_delete {
        // Delete contributions first (foreign key constraint)
        sqlx::query("DELETE FROM contributions WHERE payment_id = ?")
            .bind(payment_id)
            .execute(&mut *tx)
            .await?;

        // Delete payment
        sqlx::query("DELETE FROM payments WHERE id = ?")
            .bind(payment_id)
            .execute(&mut *tx)
            .await?;
    }

    // Create new consolidated payment
    let result = sqlx::query(
        "INSERT INTO payments (project_id, payer_id, amount, description, payment_date,
         is_recurring, recurrence_type, recurrence_interval, receiver_account_id)
         VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?)"
    )
    .bind(member.project_id)
    .bind(req.payer_id)
    .bind(req.amount)
    .bind(&req.description)
    .bind(&req.payment_date)
    .bind(&req.recurrence_type)
    .bind(req.recurrence_interval)
    .bind(req.pool_id)
    .execute(&mut *tx)
    .await?;

    let new_payment_id = result.last_insert_rowid();

    // Commit transaction
    tx.commit().await?;

    Ok(Json(ConsolidateResponse {
        deleted_count: req.payment_ids_to_delete.len(),
        new_payment_id,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_cashflow))
        .route("/consolidate", post(consolidate_payments))
}
