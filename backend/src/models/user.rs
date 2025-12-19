use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
    pub language: String,
    pub date_format: String,
    pub currency_position: String,
    pub decimal_separator: String,
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
    pub language: String,
    pub date_format: String,
    pub currency_position: String,
    pub decimal_separator: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            language: user.language,
            date_format: user.date_format,
            currency_position: user.currency_position,
            decimal_separator: user.decimal_separator,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPreferences {
    pub language: Option<String>,
    pub date_format: Option<String>,
    pub currency_position: Option<String>,
    pub decimal_separator: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}
