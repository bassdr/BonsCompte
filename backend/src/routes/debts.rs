use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{auth::ProjectMember, error::AppResult, services::DebtSummary, AppState};

#[derive(Deserialize)]
struct DebtsQuery {
    date: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_debts))
}

async fn get_debts(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
    Query(query): Query<DebtsQuery>,
) -> AppResult<Json<DebtSummary>> {
    let summary = match query.date {
        Some(target_date) => {
            crate::services::calculate_debts_at_date(&pool, member.project_id, &target_date).await?
        }
        None => crate::services::calculate_debts(&pool, member.project_id).await?,
    };
    Ok(Json(summary))
}
