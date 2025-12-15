use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Payment {
    pub id: i64,
    pub project_id: Option<i64>,
    pub payer_id: Option<i64>,
    pub amount: f64,
    pub description: String,
    pub payment_date: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayment {
    pub payer_id: Option<i64>,
    pub amount: f64,
    pub description: String,
    pub payment_date: Option<String>,
    pub contributions: Vec<CreateContribution>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContribution {
    pub participant_id: i64,
    pub weight: f64,
}

#[derive(Debug, Serialize)]
pub struct PaymentWithContributions {
    #[serde(flatten)]
    pub payment: Payment,
    pub payer_name: Option<String>,
    pub contributions: Vec<super::ContributionWithParticipant>,
}
