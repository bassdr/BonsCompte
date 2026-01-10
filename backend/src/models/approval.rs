use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Represents an approval record for a user in a specific project
/// Created when a sensitive event (password reset, password change) requires approval
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProjectApproval {
    pub id: i64,
    pub user_id: i64,
    pub project_id: i64,
    pub event_type: String, // 'password_reset', 'password_change', etc.
    pub event_metadata: Option<String>, // JSON with event details
    pub status: String,     // 'pending', 'approved', 'rejected'
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// Represents a vote cast by a project member on an approval
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ApprovalVote {
    pub id: i64,
    pub approval_id: i64,
    pub voter_id: i64,
    pub vote: String, // 'approve' or 'reject'
    pub reason: Option<String>,
    pub voted_at: String,
}

/// Input for casting a vote on an approval
#[derive(Debug, Deserialize)]
pub struct CastVote {
    pub vote: String, // 'approve' or 'reject'
    pub reason: Option<String>,
}

/// Extended approval view with additional details for API responses
#[derive(Debug, Serialize)]
pub struct ApprovalWithDetails {
    #[serde(flatten)]
    pub approval: ProjectApproval,
    pub project_name: String,
    pub username: String,
    pub display_name: Option<String>,
    pub votes: Vec<VoteWithVoter>,
    pub vote_count: i64,
    pub required_votes: i64,
    pub can_self_approve: bool,
}

/// Vote with voter information
#[derive(Debug, Serialize)]
pub struct VoteWithVoter {
    #[serde(flatten)]
    pub vote: ApprovalVote,
    pub voter_username: String,
    pub voter_display_name: Option<String>,
}
