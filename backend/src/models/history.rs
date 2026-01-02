use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Entity types that can be logged
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Payment,
    Contribution,
    Participant,
    ProjectMember,
    Project,
    ParticipantInvite,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Payment => "payment",
            EntityType::Contribution => "contribution",
            EntityType::Participant => "participant",
            EntityType::ProjectMember => "project_member",
            EntityType::Project => "project",
            EntityType::ParticipantInvite => "participant_invite",
        }
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Action types for history entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActionType {
    Create,
    Update,
    Delete,
    Undo,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::Create => "CREATE",
            ActionType::Update => "UPDATE",
            ActionType::Delete => "DELETE",
            ActionType::Undo => "UNDO",
        }
    }
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Raw history entry from database
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub created_at: String,
    pub correlation_id: String,
    pub actor_user_id: Option<i64>,
    pub project_id: Option<i64>,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub action: String,
    pub payload_before: Option<String>,
    pub payload_after: Option<String>,
    pub reason: Option<String>,
    pub undoes_history_id: Option<i64>,
    pub previous_hash: Option<String>,
    pub entry_hash: String,
}

/// History entry response with resolved actor name and parsed payloads
#[derive(Debug, Serialize)]
pub struct HistoryEntryResponse {
    pub id: i64,
    pub created_at: String,
    pub correlation_id: String,
    pub actor_user_id: Option<i64>,
    pub actor_name: Option<String>,
    pub project_id: Option<i64>,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub action: String,
    pub payload_before: Option<serde_json::Value>,
    pub payload_after: Option<serde_json::Value>,
    pub reason: Option<String>,
    pub undoes_history_id: Option<i64>,
    pub is_undone: bool,
}

impl HistoryEntry {
    /// Convert to response format with resolved actor name and undone status
    pub fn into_response(
        self,
        actor_name: Option<String>,
        is_undone: bool,
    ) -> HistoryEntryResponse {
        HistoryEntryResponse {
            id: self.id,
            created_at: self.created_at,
            correlation_id: self.correlation_id,
            actor_user_id: self.actor_user_id,
            actor_name,
            project_id: self.project_id,
            entity_type: self.entity_type,
            entity_id: self.entity_id,
            action: self.action,
            payload_before: self
                .payload_before
                .and_then(|s| serde_json::from_str(&s).ok()),
            payload_after: self
                .payload_after
                .and_then(|s| serde_json::from_str(&s).ok()),
            reason: self.reason,
            undoes_history_id: self.undoes_history_id,
            is_undone,
        }
    }
}

/// Result of chain verification
#[derive(Debug, Serialize)]
pub struct ChainVerification {
    pub is_valid: bool,
    pub total_entries: i64,
    pub first_broken_id: Option<i64>,
    pub message: String,
}

/// Query parameters for history listing
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub entity_type: Option<String>,
}

/// Request body for undo action
#[derive(Debug, Deserialize)]
pub struct UndoRequest {
    pub reason: Option<String>,
}
