use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::bounded::{ProjectDescription, ProjectName, ShortString};

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
    #[sqlx(default)]
    pub pool_warning_horizon: String, // Deprecated: now per-pool in participants table
    #[sqlx(default)]
    pub pending_member_access: String, // 'none', 'read_only', 'auto_approve'
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    /// Project name bounded to 100 chars at deserialization
    pub name: ProjectName,
    /// Project description bounded to 500 chars at deserialization
    pub description: Option<ProjectDescription>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    /// Project name bounded to 100 chars at deserialization
    pub name: Option<ProjectName>,
    /// Project description bounded to 500 chars at deserialization
    pub description: Option<ProjectDescription>,
}

#[derive(Debug, Deserialize)]
pub struct JoinProject {
    /// Invite code bounded to 50 chars at deserialization
    pub invite_code: ShortString,
    pub participant_token: Option<ShortString>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectSettings {
    pub invites_enabled: Option<bool>,
    pub require_approval: Option<bool>,
    /// Bounded to 50 chars at deserialization
    pub pending_member_access: Option<ShortString>,
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
    #[sqlx(default)]
    pub pool_warning_horizon: String, // Deprecated: now per-pool in participants table
    #[sqlx(default)]
    pub pending_member_access: String, // 'none', 'read_only', 'auto_approve'
    pub role: String,
}

/// Summary of a user's ownership in a pool account
#[derive(Debug, Clone, Serialize)]
pub struct PoolSummary {
    pub pool_name: String,
    pub ownership: f64,
}

/// Extended project info for the project list, including owner and user's debt summary
#[derive(Debug, Clone, Serialize)]
pub struct ProjectListItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub invites_enabled: bool,
    pub require_approval: bool,
    pub pool_warning_horizon: String, // Deprecated: now per-pool in participants table
    pub pending_member_access: String, // 'none', 'read_only', 'auto_approve'
    pub role: String,
    /// Display name or username of the project owner
    pub owner_name: String,
    /// Current user's net balance (positive = they are owed, negative = they owe)
    pub user_balance: Option<f64>,
    /// Current user's pool ownership summaries
    pub user_pools: Vec<PoolSummary>,
    /// Current user's membership status in this project
    pub member_status: String,
}
