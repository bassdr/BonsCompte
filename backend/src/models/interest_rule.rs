use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Interest rule attached to a pool account, active over a date window.
/// A rate renewal (e.g. mortgage) is two rows: the old rule closed with
/// `end_date` and a new rule starting the same day.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct AccountInterestRule {
    pub id: i64,
    pub participant_id: i64,
    /// Rule applies from this date (exclusive for boundary ticks: the first
    /// period boundary strictly after start_date is the first credit).
    pub start_date: String,
    /// Inclusive end; NULL = open-ended.
    pub end_date: Option<String>,
    /// Nominal annual rate, e.g. 0.0761. Negative rates are allowed.
    pub annual_rate: f64,
    /// 'monthly' | 'semiannual' | 'annual' — when interest is credited.
    pub period: String,
    /// Credit-line style: interest is charged only while the balance is negative.
    pub only_when_negative: bool,
    /// Flat fee applied at each monthly boundary (independent of rate).
    pub monthly_fee: Option<f64>,
    /// Fee applies only while balance < threshold; NULL = always (when fee set).
    pub fee_threshold: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInterestRule {
    pub start_date: String,
    pub end_date: Option<String>,
    pub annual_rate: f64,
    pub period: String,
    #[serde(default)]
    pub only_when_negative: bool,
    pub monthly_fee: Option<f64>,
    pub fee_threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInterestRule {
    pub start_date: Option<String>,
    pub end_date: Option<Option<String>>,
    pub annual_rate: Option<f64>,
    pub period: Option<String>,
    pub only_when_negative: Option<bool>,
    pub monthly_fee: Option<Option<f64>>,
    pub fee_threshold: Option<Option<f64>>,
}

pub const INTEREST_PERIODS: [&str; 3] = ["monthly", "semiannual", "annual"];
