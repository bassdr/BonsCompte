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
    pub account_type: String,                    // "user" or "pool"
    pub warning_horizon_account: Option<String>, // Pool warning horizon for account total (NULL = disabled)
    pub warning_horizon_users: Option<String>, // Pool warning horizon for user ownership (NULL = disabled)
}

#[derive(Debug, Deserialize)]
pub struct CreateParticipant {
    pub name: String,
    pub default_weight: Option<f64>,
    pub account_type: Option<String>, // defaults to "user"
}

#[derive(Debug, Deserialize)]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub default_weight: Option<f64>,
    pub account_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePoolWarningSettings {
    pub warning_horizon_account: Option<String>, // null or empty = disable, value = set
    pub warning_horizon_users: Option<String>,   // null or empty = disable, value = set
}
