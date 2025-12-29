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
    // User preferences (nullable, defaults applied in UserPreferences)
    #[serde(skip_serializing)]
    pub language: Option<String>,
    #[serde(skip_serializing)]
    pub date_format: Option<String>,
    #[serde(skip_serializing)]
    pub decimal_separator: Option<String>,
    #[serde(skip_serializing)]
    pub currency_symbol: Option<String>,
    #[serde(skip_serializing)]
    pub currency_symbol_position: Option<String>,
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

/// User preferences with server-side defaults applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub language: String,
    pub date_format: String,
    pub decimal_separator: String,
    pub currency_symbol: String,
    pub currency_symbol_position: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            date_format: "mdy".to_string(),
            decimal_separator: ".".to_string(),
            currency_symbol: "$".to_string(),
            currency_symbol_position: "before".to_string(),
        }
    }
}

impl UserPreferences {
    /// Create preferences from User, applying defaults for NULL values
    pub fn from_user(user: &User) -> Self {
        let defaults = Self::default();
        Self {
            language: user.language.clone().unwrap_or(defaults.language),
            date_format: user.date_format.clone().unwrap_or(defaults.date_format),
            decimal_separator: user.decimal_separator.clone().unwrap_or(defaults.decimal_separator),
            currency_symbol: user.currency_symbol.clone().unwrap_or(defaults.currency_symbol),
            currency_symbol_position: user.currency_symbol_position.clone().unwrap_or(defaults.currency_symbol_position),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub display_name: Option<String>,
    pub preferences: UserPreferences,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        let preferences = UserPreferences::from_user(&user);
        Self {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            preferences,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}
