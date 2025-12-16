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
    // Receipt image (Base64 encoded)
    pub receipt_image: Option<String>,
    // Recurrence fields
    pub is_recurring: bool,
    pub recurrence_type: Option<String>,  // 'daily', 'weekly', 'monthly', 'yearly'
    pub recurrence_interval: Option<i32>, // every X periods
    pub recurrence_times_per: Option<i32>, // NULL = every X, non-NULL = X times per period
    pub recurrence_end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayment {
    pub payer_id: Option<i64>,
    pub amount: f64,
    pub description: String,
    pub payment_date: Option<String>,
    pub contributions: Vec<CreateContribution>,
    // Receipt image (Base64 encoded)
    pub receipt_image: Option<String>,
    // Recurrence fields
    pub is_recurring: Option<bool>,
    pub recurrence_type: Option<String>,
    pub recurrence_interval: Option<i32>,
    pub recurrence_times_per: Option<i32>,
    pub recurrence_end_date: Option<String>,
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
