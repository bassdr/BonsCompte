use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Reader = 0,
    Editor = 1,
    Admin = 2,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "reader" => Some(Role::Reader),
            "editor" => Some(Role::Editor),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ProjectMemberRow {
    pub id: i64,
    pub project_id: i64,
    pub user_id: i64,
    pub role: String,
    pub participant_id: Option<i64>,
    pub joined_at: String,
    pub status: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProjectMemberResponse {
    pub id: i64,
    pub project_id: i64,
    pub user_id: i64,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    pub participant_id: Option<i64>,
    pub participant_name: Option<String>,
    pub joined_at: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRole {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct SetMemberParticipant {
    pub participant_id: Option<i64>,
}
