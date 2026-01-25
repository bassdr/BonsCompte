use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A trusted user relationship for password recovery
/// If user A adds user B as trusted, B can help A recover their password
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TrustedUser {
    pub id: i64,
    pub user_id: i64,
    pub trusted_user_id: i64,
    pub created_at: String,
}

/// Trusted user with display info for API responses
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TrustedUserWithInfo {
    pub id: i64,
    pub trusted_user_id: i64,
    pub username: String,
    pub display_name: Option<String>,
    pub created_at: String,
}

/// Request to add a trusted user by username
#[derive(Debug, Deserialize)]
pub struct AddTrustedUserRequest {
    pub username: String,
}

/// Recovery status for the current user
#[derive(Debug, Serialize)]
pub struct RecoveryStatus {
    /// Whether the account can be recovered via trusted users
    pub recoverable: bool,
    /// Number of trusted users configured
    pub trusted_user_count: i64,
    /// Minimum required for recovery (currently 2)
    pub required_count: i64,
}

impl RecoveryStatus {
    pub const REQUIRED_TRUSTED_USERS: i64 = 2;

    pub fn new(trusted_user_count: i64) -> Self {
        Self {
            recoverable: trusted_user_count >= Self::REQUIRED_TRUSTED_USERS,
            trusted_user_count,
            required_count: Self::REQUIRED_TRUSTED_USERS,
        }
    }
}
