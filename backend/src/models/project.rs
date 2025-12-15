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
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProjectWithRole {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub role: String,
}
