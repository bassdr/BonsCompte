use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    auth::ProjectMember,
    error::{AppError, AppResult},
    models::{ContributionWithParticipant, CreatePayment, Payment, PaymentWithContributions},
    services::validate_image_base64,
    AppState,
};

#[derive(Deserialize)]
struct PaymentPath {
    id: i64,
    payment_id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_payments).post(create_payment))
        .route("/{payment_id}", get(get_payment).put(update_payment).delete(delete_payment))
}

async fn list_payments(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<PaymentWithContributions>>> {
    let payments: Vec<Payment> = sqlx::query_as(
        "SELECT * FROM payments WHERE project_id = ? ORDER BY payment_date DESC"
    )
    .bind(member.project_id)
    .fetch_all(&pool)
    .await?;

    let mut result = Vec::new();
    for payment in payments {
        // Get payer name
        let payer_name: Option<String> = if let Some(payer_id) = payment.payer_id {
            sqlx::query_scalar("SELECT name FROM participants WHERE id = ?")
                .bind(payer_id)
                .fetch_optional(&pool)
                .await?
        } else {
            None
        };

        // Get contributions with participant names
        let contributions: Vec<ContributionWithParticipant> = sqlx::query_as(
            "SELECT c.id, c.participant_id, p.name as participant_name, c.payment_id, c.amount, c.weight
             FROM contributions c
             JOIN participants p ON c.participant_id = p.id
             WHERE c.payment_id = ?"
        )
        .bind(payment.id)
        .fetch_all(&pool)
        .await?;

        result.push(PaymentWithContributions {
            payment,
            payer_name,
            contributions,
        });
    }

    Ok(Json(result))
}

async fn get_payment(
    Path(path): Path<PaymentPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<PaymentWithContributions>> {
    let payment: Option<Payment> = sqlx::query_as(
        "SELECT * FROM payments WHERE id = ? AND project_id = ?"
    )
    .bind(path.payment_id)
    .bind(member.project_id)
    .fetch_optional(&pool)
    .await?;

    let payment = payment.ok_or_else(|| AppError::NotFound("Payment not found".to_string()))?;

    // Get payer name
    let payer_name: Option<String> = if let Some(payer_id) = payment.payer_id {
        sqlx::query_scalar("SELECT name FROM participants WHERE id = ?")
            .bind(payer_id)
            .fetch_optional(&pool)
            .await?
    } else {
        None
    };

    let contributions: Vec<ContributionWithParticipant> = sqlx::query_as(
        "SELECT c.id, c.participant_id, p.name as participant_name, c.payment_id, c.amount, c.weight
         FROM contributions c
         JOIN participants p ON c.participant_id = p.id
         WHERE c.payment_id = ?"
    )
    .bind(payment.id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(PaymentWithContributions {
        payment,
        payer_name,
        contributions,
    }))
}

async fn create_payment(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreatePayment>,
) -> AppResult<Json<PaymentWithContributions>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::Forbidden("Editor access required".to_string()));
    }

    // Validate
    if input.amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".to_string()));
    }
    if input.contributions.is_empty() {
        return Err(AppError::BadRequest("At least one contribution required".to_string()));
    }

    // Validate payer belongs to project
    if let Some(payer_id) = input.payer_id {
        let payer_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM participants WHERE id = ? AND project_id = ?"
        )
        .bind(payer_id)
        .bind(member.project_id)
        .fetch_optional(&pool)
        .await?;

        if payer_exists.is_none() {
            return Err(AppError::BadRequest("Invalid payer".to_string()));
        }
    }

    // Validate all participants belong to project
    for contrib in &input.contributions {
        let participant_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM participants WHERE id = ? AND project_id = ?"
        )
        .bind(contrib.participant_id)
        .bind(member.project_id)
        .fetch_optional(&pool)
        .await?;

        if participant_exists.is_none() {
            return Err(AppError::BadRequest(format!(
                "Invalid participant: {}", contrib.participant_id
            )));
        }
    }

    // Calculate total weight
    let total_weight: f64 = input.contributions.iter().map(|c| c.weight).sum();
    if total_weight <= 0.0 {
        return Err(AppError::BadRequest("Total weight must be positive".to_string()));
    }

    // Validate receipt image if provided
    if let Some(ref image) = input.receipt_image {
        validate_image_base64(image)?;
    }

    // Insert payment
    let payment_date = input.payment_date.unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });

    let is_recurring = input.is_recurring.unwrap_or(false);

    let result = sqlx::query(
        "INSERT INTO payments (project_id, payer_id, amount, description, payment_date, receipt_image, is_recurring, recurrence_type, recurrence_interval, recurrence_times_per, recurrence_end_date)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(member.project_id)
    .bind(input.payer_id)
    .bind(input.amount)
    .bind(&input.description)
    .bind(&payment_date)
    .bind(&input.receipt_image)
    .bind(is_recurring)
    .bind(&input.recurrence_type)
    .bind(input.recurrence_interval)
    .bind(input.recurrence_times_per)
    .bind(&input.recurrence_end_date)
    .execute(&pool)
    .await?;

    let payment_id = result.last_insert_rowid();

    // Calculate and insert contributions
    let mut contributions = Vec::new();
    for contrib in &input.contributions {
        let share_amount = (input.amount * contrib.weight / total_weight * 100.0).round() / 100.0;

        // Get participant name
        let participant_name: String = sqlx::query_scalar(
            "SELECT name FROM participants WHERE id = ?"
        )
        .bind(contrib.participant_id)
        .fetch_one(&pool)
        .await?;

        let result = sqlx::query(
            "INSERT INTO contributions (participant_id, payment_id, amount, weight) VALUES (?, ?, ?, ?)"
        )
        .bind(contrib.participant_id)
        .bind(payment_id)
        .bind(share_amount)
        .bind(contrib.weight)
        .execute(&pool)
        .await?;

        contributions.push(ContributionWithParticipant {
            id: result.last_insert_rowid(),
            participant_id: contrib.participant_id,
            participant_name,
            payment_id,
            amount: share_amount,
            weight: contrib.weight,
        });
    }

    // Fetch created payment
    let payment: Payment = sqlx::query_as("SELECT * FROM payments WHERE id = ?")
        .bind(payment_id)
        .fetch_one(&pool)
        .await?;

    // Get payer name
    let payer_name: Option<String> = if let Some(payer_id) = payment.payer_id {
        sqlx::query_scalar("SELECT name FROM participants WHERE id = ?")
            .bind(payer_id)
            .fetch_optional(&pool)
            .await?
    } else {
        None
    };

    Ok(Json(PaymentWithContributions {
        payment,
        payer_name,
        contributions,
    }))
}

async fn update_payment(
    Path(path): Path<PaymentPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreatePayment>,
) -> AppResult<Json<PaymentWithContributions>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::Forbidden("Editor access required".to_string()));
    }

    // Verify payment exists and belongs to project
    let existing: Option<Payment> = sqlx::query_as(
        "SELECT * FROM payments WHERE id = ? AND project_id = ?"
    )
    .bind(path.payment_id)
    .bind(member.project_id)
    .fetch_optional(&pool)
    .await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Payment not found".to_string()));
    }

    // Validate
    if input.amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".to_string()));
    }
    if input.contributions.is_empty() {
        return Err(AppError::BadRequest("At least one contribution required".to_string()));
    }

    // Validate payer belongs to project
    if let Some(payer_id) = input.payer_id {
        let payer_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM participants WHERE id = ? AND project_id = ?"
        )
        .bind(payer_id)
        .bind(member.project_id)
        .fetch_optional(&pool)
        .await?;

        if payer_exists.is_none() {
            return Err(AppError::BadRequest("Invalid payer".to_string()));
        }
    }

    // Validate all participants belong to project
    for contrib in &input.contributions {
        let participant_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM participants WHERE id = ? AND project_id = ?"
        )
        .bind(contrib.participant_id)
        .bind(member.project_id)
        .fetch_optional(&pool)
        .await?;

        if participant_exists.is_none() {
            return Err(AppError::BadRequest(format!(
                "Invalid participant: {}", contrib.participant_id
            )));
        }
    }

    // Calculate total weight
    let total_weight: f64 = input.contributions.iter().map(|c| c.weight).sum();
    if total_weight <= 0.0 {
        return Err(AppError::BadRequest("Total weight must be positive".to_string()));
    }

    // Validate receipt image if provided
    if let Some(ref image) = input.receipt_image {
        validate_image_base64(image)?;
    }

    let payment_date = input.payment_date.clone().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });

    let is_recurring = input.is_recurring.unwrap_or(false);

    // Update payment
    sqlx::query(
        "UPDATE payments SET payer_id = ?, amount = ?, description = ?, payment_date = ?,
         receipt_image = ?, is_recurring = ?, recurrence_type = ?, recurrence_interval = ?,
         recurrence_times_per = ?, recurrence_end_date = ?
         WHERE id = ? AND project_id = ?"
    )
    .bind(input.payer_id)
    .bind(input.amount)
    .bind(&input.description)
    .bind(&payment_date)
    .bind(&input.receipt_image)
    .bind(is_recurring)
    .bind(&input.recurrence_type)
    .bind(input.recurrence_interval)
    .bind(input.recurrence_times_per)
    .bind(&input.recurrence_end_date)
    .bind(path.payment_id)
    .bind(member.project_id)
    .execute(&pool)
    .await?;

    // Delete old contributions
    sqlx::query("DELETE FROM contributions WHERE payment_id = ?")
        .bind(path.payment_id)
        .execute(&pool)
        .await?;

    // Insert new contributions
    let mut contributions = Vec::new();
    for contrib in &input.contributions {
        let share_amount = (input.amount * contrib.weight / total_weight * 100.0).round() / 100.0;

        let participant_name: String = sqlx::query_scalar(
            "SELECT name FROM participants WHERE id = ?"
        )
        .bind(contrib.participant_id)
        .fetch_one(&pool)
        .await?;

        let result = sqlx::query(
            "INSERT INTO contributions (participant_id, payment_id, amount, weight) VALUES (?, ?, ?, ?)"
        )
        .bind(contrib.participant_id)
        .bind(path.payment_id)
        .bind(share_amount)
        .bind(contrib.weight)
        .execute(&pool)
        .await?;

        contributions.push(ContributionWithParticipant {
            id: result.last_insert_rowid(),
            participant_id: contrib.participant_id,
            participant_name,
            payment_id: path.payment_id,
            amount: share_amount,
            weight: contrib.weight,
        });
    }

    // Fetch updated payment
    let payment: Payment = sqlx::query_as("SELECT * FROM payments WHERE id = ?")
        .bind(path.payment_id)
        .fetch_one(&pool)
        .await?;

    // Get payer name
    let payer_name: Option<String> = if let Some(payer_id) = payment.payer_id {
        sqlx::query_scalar("SELECT name FROM participants WHERE id = ?")
            .bind(payer_id)
            .fetch_optional(&pool)
            .await?
    } else {
        None
    };

    Ok(Json(PaymentWithContributions {
        payment,
        payer_name,
        contributions,
    }))
}

async fn delete_payment(
    Path(path): Path<PaymentPath>,
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    // Check editor permission
    if !member.can_edit() {
        return Err(AppError::Forbidden("Editor access required".to_string()));
    }

    let result = sqlx::query("DELETE FROM payments WHERE id = ? AND project_id = ?")
        .bind(path.payment_id)
        .bind(member.project_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Payment not found".to_string()));
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
