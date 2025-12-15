use axum::{extract::State, routing::get, Json, Router};
use sqlx::SqlitePool;

use crate::{
    auth::ProjectMember,
    error::AppResult,
    services::DebtSummary,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_debts))
}

async fn get_debts(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<DebtSummary>> {
    let summary = crate::services::calculate_debts(&pool, member.project_id).await?;
    Ok(Json(summary))
}
