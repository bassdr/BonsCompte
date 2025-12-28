use crate::error::{AppError, AppResult};
use crate::models::{ActionType, ChainVerification, EntityType, HistoryEntry, HistoryEntryResponse};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Service for managing the immutable history log
pub struct HistoryService;

impl HistoryService {
    /// Generate a new correlation ID for grouping related changes
    pub fn new_correlation_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Get the hash of the most recent history entry (for chaining)
    async fn get_previous_hash(pool: &SqlitePool) -> AppResult<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT entry_hash FROM history_log ORDER BY id DESC LIMIT 1"
        )
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|(hash,)| hash))
    }

    /// Compute the hash for a new entry
    /// Hash = SHA256(previous_hash || created_at || actor_user_id || action || entity_type || payload_after)
    fn compute_entry_hash(
        previous_hash: Option<&str>,
        created_at: &str,
        actor_user_id: Option<i64>,
        action: &str,
        entity_type: &str,
        payload_after: Option<&str>,
    ) -> String {
        let mut hasher = Sha256::new();

        // Chain to previous hash
        if let Some(prev) = previous_hash {
            hasher.update(prev.as_bytes());
        }

        hasher.update(created_at.as_bytes());
        hasher.update(actor_user_id.map(|id| id.to_string()).unwrap_or_default().as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(entity_type.as_bytes());

        if let Some(payload) = payload_after {
            hasher.update(payload.as_bytes());
        }

        hex::encode(hasher.finalize())
    }

    /// Log a single event to the history log
    pub async fn log_event(
        pool: &SqlitePool,
        correlation_id: &str,
        actor_user_id: Option<i64>,
        project_id: Option<i64>,
        entity_type: &str,
        entity_id: Option<i64>,
        action: &str,
        payload_before: Option<&str>,
        payload_after: Option<&str>,
        reason: Option<&str>,
        undoes_history_id: Option<i64>,
    ) -> AppResult<i64> {
        // Get previous hash for chaining
        let previous_hash = Self::get_previous_hash(pool).await?;

        // Generate timestamp
        let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        // Compute entry hash
        let entry_hash = Self::compute_entry_hash(
            previous_hash.as_deref(),
            &created_at,
            actor_user_id,
            action,
            entity_type,
            payload_after,
        );

        // Insert the entry
        let result = sqlx::query(
            r#"
            INSERT INTO history_log (
                created_at, correlation_id, actor_user_id, project_id,
                entity_type, entity_id, action, payload_before, payload_after,
                reason, undoes_history_id, previous_hash, entry_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&created_at)
        .bind(correlation_id)
        .bind(actor_user_id)
        .bind(project_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(action)
        .bind(payload_before)
        .bind(payload_after)
        .bind(reason)
        .bind(undoes_history_id)
        .bind(previous_hash)
        .bind(&entry_hash)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Log a CREATE action
    pub async fn log_create<T: Serialize>(
        pool: &SqlitePool,
        correlation_id: &str,
        actor_user_id: i64,
        project_id: i64,
        entity_type: EntityType,
        entity_id: i64,
        entity: &T,
    ) -> AppResult<i64> {
        let payload_after = serde_json::to_string(entity)
            .map_err(|e| AppError::Internal(format!("Failed to serialize entity: {}", e)))?;

        Self::log_event(
            pool,
            correlation_id,
            Some(actor_user_id),
            Some(project_id),
            entity_type.as_str(),
            Some(entity_id),
            ActionType::Create.as_str(),
            None,
            Some(&payload_after),
            None,
            None,
        )
        .await
    }

    /// Log an UPDATE action
    pub async fn log_update<T: Serialize>(
        pool: &SqlitePool,
        correlation_id: &str,
        actor_user_id: i64,
        project_id: i64,
        entity_type: EntityType,
        entity_id: i64,
        before: &T,
        after: &T,
    ) -> AppResult<i64> {
        let payload_before = serde_json::to_string(before)
            .map_err(|e| AppError::Internal(format!("Failed to serialize before state: {}", e)))?;
        let payload_after = serde_json::to_string(after)
            .map_err(|e| AppError::Internal(format!("Failed to serialize after state: {}", e)))?;

        Self::log_event(
            pool,
            correlation_id,
            Some(actor_user_id),
            Some(project_id),
            entity_type.as_str(),
            Some(entity_id),
            ActionType::Update.as_str(),
            Some(&payload_before),
            Some(&payload_after),
            None,
            None,
        )
        .await
    }

    /// Log a DELETE action
    pub async fn log_delete<T: Serialize>(
        pool: &SqlitePool,
        correlation_id: &str,
        actor_user_id: i64,
        project_id: i64,
        entity_type: EntityType,
        entity_id: i64,
        entity: &T,
    ) -> AppResult<i64> {
        let payload_before = serde_json::to_string(entity)
            .map_err(|e| AppError::Internal(format!("Failed to serialize entity: {}", e)))?;

        Self::log_event(
            pool,
            correlation_id,
            Some(actor_user_id),
            Some(project_id),
            entity_type.as_str(),
            Some(entity_id),
            ActionType::Delete.as_str(),
            Some(&payload_before),
            None,
            None,
            None,
        )
        .await
    }

    /// Get history entries for a project (paginated)
    pub async fn get_project_history(
        pool: &SqlitePool,
        project_id: i64,
        limit: i64,
        offset: i64,
        entity_type_filter: Option<&str>,
    ) -> AppResult<Vec<HistoryEntry>> {
        let entries = if let Some(entity_type) = entity_type_filter {
            sqlx::query_as::<_, HistoryEntry>(
                r#"
                SELECT * FROM history_log
                WHERE project_id = ? AND entity_type = ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(project_id)
            .bind(entity_type)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, HistoryEntry>(
                r#"
                SELECT * FROM history_log
                WHERE project_id = ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(project_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        };

        Ok(entries)
    }

    /// Get history entries for a specific entity
    pub async fn get_entity_history(
        pool: &SqlitePool,
        project_id: i64,
        entity_type: &str,
        entity_id: i64,
    ) -> AppResult<Vec<HistoryEntry>> {
        let entries = sqlx::query_as::<_, HistoryEntry>(
            r#"
            SELECT * FROM history_log
            WHERE project_id = ? AND entity_type = ? AND entity_id = ?
            ORDER BY id DESC
            "#
        )
        .bind(project_id)
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        Ok(entries)
    }

    /// Get a single history entry by ID
    pub async fn get_entry(pool: &SqlitePool, history_id: i64) -> AppResult<Option<HistoryEntry>> {
        let entry = sqlx::query_as::<_, HistoryEntry>(
            "SELECT * FROM history_log WHERE id = ?"
        )
        .bind(history_id)
        .fetch_optional(pool)
        .await?;

        Ok(entry)
    }

    /// Check if an entry has been undone
    pub async fn is_entry_undone(pool: &SqlitePool, history_id: i64) -> AppResult<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM history_log WHERE undoes_history_id = ? AND action = 'UNDO' LIMIT 1"
        )
        .bind(history_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.is_some())
    }

    /// Check if multiple entries have been undone
    pub async fn get_undone_status(pool: &SqlitePool, history_ids: &[i64]) -> AppResult<std::collections::HashMap<i64, bool>> {
        if history_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all UNDO entries that reference any of these history IDs
        let placeholders = history_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT undoes_history_id FROM history_log WHERE undoes_history_id IN ({}) AND action = 'UNDO'",
            placeholders
        );

        let mut query_builder = sqlx::query_as::<_, (i64,)>(&query);
        for id in history_ids {
            query_builder = query_builder.bind(*id);
        }

        let undone_ids: Vec<(i64,)> = query_builder.fetch_all(pool).await?;
        let undone_set: std::collections::HashSet<i64> = undone_ids.into_iter().map(|(id,)| id).collect();

        let mut result = std::collections::HashMap::new();
        for id in history_ids {
            result.insert(*id, undone_set.contains(id));
        }

        Ok(result)
    }

    /// Resolve actor names for history entries
    pub async fn resolve_actor_names(
        pool: &SqlitePool,
        entries: Vec<HistoryEntry>,
    ) -> AppResult<Vec<HistoryEntryResponse>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Collect unique actor user IDs
        let actor_ids: Vec<i64> = entries
            .iter()
            .filter_map(|e| e.actor_user_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch actor names
        let mut actor_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        if !actor_ids.is_empty() {
            let placeholders = actor_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, COALESCE(display_name, username) as name FROM users WHERE id IN ({})",
                placeholders
            );

            let mut query_builder = sqlx::query_as::<_, (i64, String)>(&query);
            for id in &actor_ids {
                query_builder = query_builder.bind(*id);
            }

            let names: Vec<(i64, String)> = query_builder.fetch_all(pool).await?;
            for (id, name) in names {
                actor_names.insert(id, name);
            }
        }

        // Get undone status for all entries
        let history_ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
        let undone_status = Self::get_undone_status(pool, &history_ids).await?;

        // Build responses
        let responses = entries
            .into_iter()
            .map(|entry| {
                let actor_name = entry.actor_user_id.and_then(|id| actor_names.get(&id).cloned());
                let is_undone = undone_status.get(&entry.id).copied().unwrap_or(false);
                entry.into_response(actor_name, is_undone)
            })
            .collect();

        Ok(responses)
    }

    /// Verify the hash chain integrity
    pub async fn verify_chain(pool: &SqlitePool) -> AppResult<ChainVerification> {
        let entries = sqlx::query_as::<_, HistoryEntry>(
            "SELECT * FROM history_log ORDER BY id ASC"
        )
        .fetch_all(pool)
        .await?;

        let total_entries = entries.len() as i64;

        if entries.is_empty() {
            return Ok(ChainVerification {
                is_valid: true,
                total_entries: 0,
                first_broken_id: None,
                message: "No entries in history log".to_string(),
            });
        }

        let mut previous_hash: Option<String> = None;

        for entry in entries {
            // Verify this entry's hash matches what we expect
            let expected_hash = Self::compute_entry_hash(
                previous_hash.as_deref(),
                &entry.created_at,
                entry.actor_user_id,
                &entry.action,
                &entry.entity_type,
                entry.payload_after.as_deref(),
            );

            if entry.entry_hash != expected_hash {
                return Ok(ChainVerification {
                    is_valid: false,
                    total_entries,
                    first_broken_id: Some(entry.id),
                    message: format!(
                        "Hash mismatch at entry {}. Expected: {}, Found: {}",
                        entry.id, expected_hash, entry.entry_hash
                    ),
                });
            }

            // Verify the previous_hash field matches what we tracked
            if entry.previous_hash != previous_hash {
                return Ok(ChainVerification {
                    is_valid: false,
                    total_entries,
                    first_broken_id: Some(entry.id),
                    message: format!(
                        "Chain broken at entry {}. Previous hash mismatch.",
                        entry.id
                    ),
                });
            }

            previous_hash = Some(entry.entry_hash);
        }

        Ok(ChainVerification {
            is_valid: true,
            total_entries,
            first_broken_id: None,
            message: "Hash chain is valid".to_string(),
        })
    }
}
