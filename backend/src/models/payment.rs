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
    pub recurrence_times_per: Option<i32>, // DEPRECATED: kept for backwards compat, use weekdays/monthdays/months instead
    pub recurrence_end_date: Option<String>,
    // Enhanced recurrence patterns
    pub recurrence_weekdays: Option<String>,  // JSON: [[1,3],[0,5]] for week patterns
    pub recurrence_monthdays: Option<String>, // JSON: [1, 15] for monthly day selection
    pub recurrence_months: Option<String>,    // JSON: [1, 6, 12] for yearly month selection
    // Internal transfer support
    // NULL = external expense (money leaves system)
    // NOT NULL = internal transfer (money moves between accounts, e.g., user → pool)
    pub receiver_account_id: Option<i64>,
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
    pub recurrence_times_per: Option<i32>, // DEPRECATED
    pub recurrence_end_date: Option<String>,
    // Enhanced recurrence patterns
    pub recurrence_weekdays: Option<String>,  // JSON array
    pub recurrence_monthdays: Option<String>, // JSON array
    pub recurrence_months: Option<String>,    // JSON array
    // Internal transfer: recipient account (NULL = external expense)
    pub receiver_account_id: Option<i64>,
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
