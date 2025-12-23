use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// User account state for security workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserState {
    Active,
    PendingApproval,
    Revoked,
}

impl UserState {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserState::Active => "active",
            UserState::PendingApproval => "pending_approval",
            UserState::Revoked => "revoked",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(UserState::Active),
            "pending_approval" => Some(UserState::PendingApproval),
            "revoked" => Some(UserState::Revoked),
            _ => None,
        }
    }
}

impl Default for UserState {
    fn default() -> Self {
        UserState::Active
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
    #[serde(skip_serializing)]
    pub user_state: String,
    #[serde(skip_serializing)]
    pub token_version: i64,
}

impl User {
    /// Get the parsed user state
    pub fn state(&self) -> UserState {
        UserState::from_str(&self.user_state).unwrap_or(UserState::Active)
    }

    /// Check if user can access protected resources
    pub fn is_active(&self) -> bool {
        self.state() == UserState::Active
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub display_name: Option<String>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}
