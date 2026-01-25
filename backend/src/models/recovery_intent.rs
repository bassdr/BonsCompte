use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A password recovery intent awaiting approval from trusted users
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecoveryIntent {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    pub resolved_at: Option<String>,
}

/// Recovery intent with user info for display
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryIntentWithInfo {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub display_name: Option<String>,
    pub token: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    pub approvals_count: i64,
    pub required_approvals: i64,
    /// Whether the current user has already voted
    pub user_has_voted: bool,
    /// The current user's vote if they voted
    pub user_vote: Option<String>,
}

/// A vote on a recovery intent
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecoveryVote {
    pub id: i64,
    pub recovery_id: i64,
    pub voter_id: i64,
    pub vote: String,
    pub voted_at: String,
}

/// Request to initiate password recovery
#[derive(Debug, Deserialize)]
pub struct InitiateRecoveryRequest {
    pub username: String,
}

/// Response after initiating recovery
#[derive(Debug, Serialize)]
pub struct InitiateRecoveryResponse {
    pub message: String,
    /// Token for checking status (only shown to the user initiating)
    pub token: String,
    pub expires_at: String,
}

/// Public status of a recovery intent (for the user who initiated it)
#[derive(Debug, Serialize)]
pub struct RecoveryIntentStatus {
    pub status: String,
    pub approvals_count: i64,
    pub required_approvals: i64,
    pub expires_at: String,
    pub is_expired: bool,
}

/// Request to vote on a recovery intent
#[derive(Debug, Deserialize)]
pub struct RecoveryVoteRequest {
    pub vote: String, // "approve" or "reject"
}

/// Request to reset password after recovery is approved
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

impl RecoveryIntent {
    pub const EXPIRY_MINUTES: i64 = 30;
    pub const REQUIRED_APPROVALS: i64 = 2;

    pub fn is_expired(&self) -> bool {
        use chrono::{DateTime, Utc};
        if let Ok(expires) =
            DateTime::parse_from_rfc3339(&format!("{}Z", self.expires_at.replace(' ', "T")))
        {
            expires < Utc::now()
        } else {
            // If we can't parse, assume expired for safety
            true
        }
    }
}
