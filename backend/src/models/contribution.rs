use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Contribution {
    pub id: i64,
    pub participant_id: i64,
    pub payment_id: i64,
    pub amount: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ContributionWithParticipant {
    pub id: i64,
    pub participant_id: i64,
    pub participant_name: String,
    pub payment_id: i64,
    pub amount: f64,
    pub weight: f64,
}
