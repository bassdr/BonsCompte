use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Participant {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub user_id: Option<i64>,
    pub default_weight: f64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateParticipant {
    pub name: String,
    pub default_weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub default_weight: Option<f64>,
}
