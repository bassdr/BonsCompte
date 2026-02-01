use crate::error::{AppError, AppResult};
use crate::models::{
    ActionType, ChainVerification, EntityType, HistoryEntry, HistoryEntryResponse,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Parameters for logging a history event
pub struct LogEventParams<'a> {
    pub correlation_id: &'a str,
    pub actor_user_id: Option<i64>,
    pub project_id: Option<i64>,
    pub entity_type: &'a str,
    pub entity_id: Option<i64>,
    pub action: &'a str,
    pub payload_before: Option<&'a str>,
    pub payload_after: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub undoes_history_id: Option<i64>,
}

/// Parameters for logging an update event
pub struct LogUpdateParams<'a, T: Serialize> {
    pub correlation_id: &'a str,
    pub actor_user_id: i64,
    pub project_id: i64,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub before: &'a T,
    pub after: &'a T,
}

/// Service for managing the immutable history log
pub struct HistoryService;

impl HistoryService {
    /// Generate a new correlation ID for grouping related changes
    pub fn new_correlation_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Get the hash of the most recent history entry (for chaining)
    async fn get_previous_hash(pool: &SqlitePool) -> AppResult<Option<String>> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT entry_hash FROM history_log ORDER BY id DESC LIMIT 1")
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
        // codeql[rust/cleartext-logging] Intentional: actor_user_id is part of audit trail hash.
        // This data is stored in the access-controlled history_log table (only accessible to
        // project members) and is necessary for maintaining an immutable audit trail.
        // The user ID is not sensitive PII - it's an internal identifier for accountability.
        hasher.update(
            actor_user_id
                .map(|id| id.to_string())
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(action.as_bytes());
        hasher.update(entity_type.as_bytes());

        if let Some(payload) = payload_after {
            hasher.update(payload.as_bytes());
        }

        hex::encode(hasher.finalize())
    }

    /// Log a single event to the history log
    pub async fn log_event(pool: &SqlitePool, params: LogEventParams<'_>) -> AppResult<i64> {
        // Get previous hash for chaining
        let previous_hash = Self::get_previous_hash(pool).await?;

        // Generate timestamp
        let created_at = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        // Compute entry hash
        let entry_hash = Self::compute_entry_hash(
            previous_hash.as_deref(),
            &created_at,
            params.actor_user_id,
            params.action,
            params.entity_type,
            params.payload_after,
        );

        // Insert the entry
        let result = sqlx::query(
            r#"
            INSERT INTO history_log (
                created_at, correlation_id, actor_user_id, project_id,
                entity_type, entity_id, action, payload_before, payload_after,
                reason, undoes_history_id, previous_hash, entry_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&created_at)
        .bind(params.correlation_id)
        .bind(params.actor_user_id)
        .bind(params.project_id)
        .bind(params.entity_type)
        .bind(params.entity_id)
        .bind(params.action)
        .bind(params.payload_before)
        .bind(params.payload_after)
        .bind(params.reason)
        .bind(params.undoes_history_id)
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
            LogEventParams {
                correlation_id,
                actor_user_id: Some(actor_user_id),
                project_id: Some(project_id),
                entity_type: entity_type.as_str(),
                entity_id: Some(entity_id),
                action: ActionType::Create.as_str(),
                payload_before: None,
                payload_after: Some(&payload_after),
                reason: None,
                undoes_history_id: None,
            },
        )
        .await
    }

    /// Log an UPDATE action
    pub async fn log_update<T: Serialize>(
        pool: &SqlitePool,
        params: LogUpdateParams<'_, T>,
    ) -> AppResult<i64> {
        let payload_before = serde_json::to_string(params.before)
            .map_err(|e| AppError::Internal(format!("Failed to serialize before state: {}", e)))?;
        let payload_after = serde_json::to_string(params.after)
            .map_err(|e| AppError::Internal(format!("Failed to serialize after state: {}", e)))?;

        Self::log_event(
            pool,
            LogEventParams {
                correlation_id: params.correlation_id,
                actor_user_id: Some(params.actor_user_id),
                project_id: Some(params.project_id),
                entity_type: params.entity_type.as_str(),
                entity_id: Some(params.entity_id),
                action: ActionType::Update.as_str(),
                payload_before: Some(&payload_before),
                payload_after: Some(&payload_after),
                reason: None,
                undoes_history_id: None,
            },
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
            LogEventParams {
                correlation_id,
                actor_user_id: Some(actor_user_id),
                project_id: Some(project_id),
                entity_type: entity_type.as_str(),
                entity_id: Some(entity_id),
                action: ActionType::Delete.as_str(),
                payload_before: Some(&payload_before),
                payload_after: None,
                reason: None,
                undoes_history_id: None,
            },
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
                "#,
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
                "#,
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
            "#,
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
        let entry = sqlx::query_as::<_, HistoryEntry>("SELECT * FROM history_log WHERE id = ?")
            .bind(history_id)
            .fetch_optional(pool)
            .await?;

        Ok(entry)
    }

    /// Check if an entry has been undone
    pub async fn is_entry_undone(pool: &SqlitePool, history_id: i64) -> AppResult<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM history_log WHERE undoes_history_id = ? AND action = 'UNDO' LIMIT 1",
        )
        .bind(history_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.is_some())
    }

    /// Check if multiple entries have been undone
    pub async fn get_undone_status(
        pool: &SqlitePool,
        history_ids: &[i64],
    ) -> AppResult<std::collections::HashMap<i64, bool>> {
        if history_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all UNDO entries that reference any of these history IDs
        let placeholders = history_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT undoes_history_id FROM history_log WHERE undoes_history_id IN ({}) AND action = 'UNDO'",
            placeholders
        );

        let mut query_builder = sqlx::query_as::<_, (i64,)>(&query);
        for id in history_ids {
            query_builder = query_builder.bind(*id);
        }

        let undone_ids: Vec<(i64,)> = query_builder.fetch_all(pool).await?;
        let undone_set: std::collections::HashSet<i64> =
            undone_ids.into_iter().map(|(id,)| id).collect();

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
        let mut actor_names: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();
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
                let actor_name = entry
                    .actor_user_id
                    .and_then(|id| actor_names.get(&id).cloned());
                let is_undone = undone_status.get(&entry.id).copied().unwrap_or(false);
                entry.into_response(actor_name, is_undone)
            })
            .collect();

        Ok(responses)
    }

    /// Verify the hash chain integrity
    pub async fn verify_chain(pool: &SqlitePool) -> AppResult<ChainVerification> {
        let entries =
            sqlx::query_as::<_, HistoryEntry>("SELECT * FROM history_log ORDER BY id ASC")
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Helper to create LogEventParams for tests
    fn test_params<'a>(
        correlation_id: &'a str,
        action: &'a str,
        entity_id: Option<i64>,
        payload_before: Option<&'a str>,
        payload_after: Option<&'a str>,
    ) -> LogEventParams<'a> {
        LogEventParams {
            correlation_id,
            actor_user_id: Some(1),
            project_id: Some(1),
            entity_type: "payment",
            entity_id,
            action,
            payload_before,
            payload_after,
            reason: None,
            undoes_history_id: None,
        }
    }

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create history_log table
        sqlx::query(
            r#"
            CREATE TABLE history_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                correlation_id TEXT NOT NULL,
                actor_user_id INTEGER,
                project_id INTEGER,
                entity_type TEXT NOT NULL,
                entity_id INTEGER,
                action TEXT NOT NULL,
                payload_before TEXT,
                payload_after TEXT,
                reason TEXT,
                undoes_history_id INTEGER,
                previous_hash TEXT,
                entry_hash TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create history_log table");

        // Create append-only triggers
        sqlx::query(
            r#"
            CREATE TRIGGER history_no_update
            BEFORE UPDATE ON history_log
            BEGIN
                SELECT RAISE(FAIL, 'history_log is append-only');
            END
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create update trigger");

        sqlx::query(
            r#"
            CREATE TRIGGER history_no_delete
            BEFORE DELETE ON history_log
            BEGIN
                SELECT RAISE(FAIL, 'history_log is append-only');
            END
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create delete trigger");

        pool
    }

    #[tokio::test]
    async fn test_trigger_blocks_update() {
        let pool = setup_test_db().await;

        // Insert a valid entry
        HistoryService::log_event(
            &pool,
            test_params(
                "test-correlation",
                "CREATE",
                Some(1),
                None,
                Some(r#"{"amount":100}"#),
            ),
        )
        .await
        .expect("Failed to insert entry");

        // Attempt to UPDATE - should fail
        let result = sqlx::query("UPDATE history_log SET action = 'HACKED' WHERE id = 1")
            .execute(&pool)
            .await;

        assert!(result.is_err(), "UPDATE should be blocked by trigger");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("append-only"),
            "Error should mention append-only: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_trigger_blocks_delete() {
        let pool = setup_test_db().await;

        // Insert a valid entry
        HistoryService::log_event(
            &pool,
            test_params(
                "test-correlation",
                "CREATE",
                Some(1),
                None,
                Some(r#"{"amount":100}"#),
            ),
        )
        .await
        .expect("Failed to insert entry");

        // Attempt to DELETE - should fail
        let result = sqlx::query("DELETE FROM history_log WHERE id = 1")
            .execute(&pool)
            .await;

        assert!(result.is_err(), "DELETE should be blocked by trigger");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("append-only"),
            "Error should mention append-only: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_valid_chain_passes_verification() {
        let pool = setup_test_db().await;

        // Insert multiple entries with proper hash chaining
        HistoryService::log_event(
            &pool,
            test_params("corr-1", "CREATE", Some(1), None, Some(r#"{"amount":100}"#)),
        )
        .await
        .expect("Failed to insert entry 1");

        HistoryService::log_event(
            &pool,
            test_params(
                "corr-2",
                "UPDATE",
                Some(1),
                Some(r#"{"amount":100}"#),
                Some(r#"{"amount":200}"#),
            ),
        )
        .await
        .expect("Failed to insert entry 2");

        HistoryService::log_event(
            &pool,
            test_params("corr-3", "DELETE", Some(1), Some(r#"{"amount":200}"#), None),
        )
        .await
        .expect("Failed to insert entry 3");

        // Verify chain
        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(
            verification.is_valid,
            "Valid chain should pass verification"
        );
        assert_eq!(verification.total_entries, 3);
        assert!(verification.first_broken_id.is_none());
    }

    #[tokio::test]
    async fn test_tampered_hash_detected() {
        let pool = setup_test_db().await;

        // Insert entries normally
        HistoryService::log_event(
            &pool,
            test_params("corr-1", "CREATE", Some(1), None, Some(r#"{"amount":100}"#)),
        )
        .await
        .expect("Failed to insert entry 1");

        HistoryService::log_event(
            &pool,
            test_params("corr-2", "UPDATE", Some(1), None, Some(r#"{"amount":200}"#)),
        )
        .await
        .expect("Failed to insert entry 2");

        // Drop trigger temporarily to simulate bypass attack
        sqlx::query("DROP TRIGGER history_no_update")
            .execute(&pool)
            .await
            .expect("Failed to drop trigger");

        // Tamper with entry_hash directly (simulating file-level corruption)
        sqlx::query("UPDATE history_log SET entry_hash = 'TAMPERED_HASH' WHERE id = 2")
            .execute(&pool)
            .await
            .expect("Failed to tamper with entry");

        // Verify chain - should detect tampering
        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(
            !verification.is_valid,
            "Tampered chain should fail verification"
        );
        assert_eq!(
            verification.first_broken_id,
            Some(2),
            "Should identify tampered entry"
        );
        assert!(
            verification.message.contains("Hash mismatch"),
            "Should report hash mismatch: {}",
            verification.message
        );
    }

    #[tokio::test]
    async fn test_tampered_payload_detected() {
        let pool = setup_test_db().await;

        // Insert entries normally
        HistoryService::log_event(
            &pool,
            test_params("corr-1", "CREATE", Some(1), None, Some(r#"{"amount":100}"#)),
        )
        .await
        .expect("Failed to insert entry");

        // Drop trigger to simulate bypass
        sqlx::query("DROP TRIGGER history_no_update")
            .execute(&pool)
            .await
            .expect("Failed to drop trigger");

        // Tamper with payload (without updating hash)
        sqlx::query(r#"UPDATE history_log SET payload_after = '{"amount":9999999}' WHERE id = 1"#)
            .execute(&pool)
            .await
            .expect("Failed to tamper with payload");

        // Verify chain - should detect tampering
        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(
            !verification.is_valid,
            "Payload tampering should be detected"
        );
        assert_eq!(verification.first_broken_id, Some(1));
    }

    #[tokio::test]
    async fn test_broken_chain_link_detected() {
        let pool = setup_test_db().await;

        // Insert entries normally
        HistoryService::log_event(
            &pool,
            test_params("corr-1", "CREATE", Some(1), None, Some(r#"{"amount":100}"#)),
        )
        .await
        .expect("Failed to insert entry 1");

        HistoryService::log_event(
            &pool,
            test_params("corr-2", "UPDATE", Some(1), None, Some(r#"{"amount":200}"#)),
        )
        .await
        .expect("Failed to insert entry 2");

        // Drop trigger to simulate bypass
        sqlx::query("DROP TRIGGER history_no_update")
            .execute(&pool)
            .await
            .expect("Failed to drop trigger");

        // Corrupt the previous_hash link
        sqlx::query("UPDATE history_log SET previous_hash = 'WRONG_PREVIOUS' WHERE id = 2")
            .execute(&pool)
            .await
            .expect("Failed to corrupt chain link");

        // Verify chain - should detect broken link
        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(!verification.is_valid, "Broken chain should be detected");
        assert_eq!(verification.first_broken_id, Some(2));
        assert!(
            verification.message.contains("Previous hash mismatch")
                || verification.message.contains("Chain broken"),
            "Should report chain break: {}",
            verification.message
        );
    }

    #[tokio::test]
    async fn test_deleted_entry_detected() {
        let pool = setup_test_db().await;

        // Insert 3 entries
        for i in 1..=3 {
            let corr_id = format!("corr-{}", i);
            let payload = format!(r#"{{"id":{}}}"#, i);
            HistoryService::log_event(
                &pool,
                test_params(&corr_id, "CREATE", Some(i), None, Some(&payload)),
            )
            .await
            .expect("Failed to insert entry");
        }

        // Verify initial chain is valid
        let initial = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed initial verification");
        assert!(initial.is_valid, "Initial chain should be valid");

        // Drop trigger to simulate bypass
        sqlx::query("DROP TRIGGER history_no_delete")
            .execute(&pool)
            .await
            .expect("Failed to drop trigger");

        // Delete middle entry (simulating malicious removal)
        sqlx::query("DELETE FROM history_log WHERE id = 2")
            .execute(&pool)
            .await
            .expect("Failed to delete entry");

        // Verify chain - should detect deletion via broken link
        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(
            !verification.is_valid,
            "Deletion should be detected via broken chain"
        );
        // Entry 3 now has wrong previous_hash (points to deleted entry 2's hash)
        assert_eq!(
            verification.first_broken_id,
            Some(3),
            "Should detect break at entry after deletion"
        );
    }

    #[tokio::test]
    async fn test_empty_log_is_valid() {
        let pool = setup_test_db().await;

        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(verification.is_valid, "Empty log should be valid");
        assert_eq!(verification.total_entries, 0);
    }

    #[tokio::test]
    async fn test_single_entry_is_valid() {
        let pool = setup_test_db().await;

        HistoryService::log_event(
            &pool,
            test_params("corr-1", "CREATE", Some(1), None, Some(r#"{"amount":100}"#)),
        )
        .await
        .expect("Failed to insert entry");

        let verification = HistoryService::verify_chain(&pool)
            .await
            .expect("Failed to verify chain");

        assert!(verification.is_valid, "Single entry should be valid");
        assert_eq!(verification.total_entries, 1);
    }
}
