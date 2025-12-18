use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub invites_enabled: bool,
    pub require_approval: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JoinProject {
    pub invite_code: String,
    pub participant_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectSettings {
    pub invites_enabled: Option<bool>,
    pub require_approval: Option<bool>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProjectWithRole {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub invites_enabled: bool,
    pub require_approval: bool,
    pub role: String,
}
