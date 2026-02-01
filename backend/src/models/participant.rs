use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::bounded::{ParticipantName, ShortString, WarningHorizon};

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
    pub name: ParticipantName,
    pub default_weight: Option<f64>,
    pub account_type: Option<ShortString>, // defaults to "user"
}

#[derive(Debug, Deserialize)]
pub struct UpdateParticipant {
    pub name: Option<ParticipantName>,
    pub default_weight: Option<f64>,
    pub account_type: Option<ShortString>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePoolWarningSettings {
    /// Warning horizon for pool account total. Bounded to 30 chars at deserialization.
    /// null or empty = disable, value = set
    pub warning_horizon_account: Option<WarningHorizon>,
    /// Warning horizon for user ownership. Bounded to 30 chars at deserialization.
    /// null or empty = disable, value = set
    pub warning_horizon_users: Option<WarningHorizon>,
}
