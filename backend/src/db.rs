use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    ConnectOptions, SqlitePool,
};
use std::{path::Path, str::FromStr, time::Duration};

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Extract path from sqlite: URL
    let db_path = database_url.strip_prefix("sqlite:").unwrap_or(database_url);

    // Create parent directory if needed
    if let Some(parent) = Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    // Add ?mode=rwc to create the file if it doesn't exist
    let connect_url = if database_url.contains('?') {
        database_url.to_string()
    } else {
        format!("{}?mode=rwc", database_url)
    };

    // Configure connection options optimized for SQLite
    // - WAL mode for better read/write concurrency
    // - busy_timeout to wait for locks instead of immediate failure
    // - Higher slow query threshold for large receipt images
    let connect_options = SqliteConnectOptions::from_str(&connect_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal) // Faster writes, still safe with WAL
        .busy_timeout(Duration::from_secs(30)) // Wait up to 30s for locks
        .log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(5));

    // SQLite prefers smaller connection pools (1-3 connections)
    // Larger pools cause contention since SQLite has a single writer
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(30))
        .connect_with(connect_options)
        .await?;

    // Enable foreign keys (must be set per-connection, but pool handles this)
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    tracing::info!(
        "SQLite pool initialized: max_connections=2, busy_timeout=30s, journal_mode=WAL"
    );

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // =====================
    // Migration 001: Initial schema
    // =====================

    // Users table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            display_name TEXT,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // =====================
    // Migration 002: Projects & Participants
    // =====================

    // Projects table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            invite_code TEXT UNIQUE,
            created_by INTEGER NOT NULL REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Participants table (must be created before project_members due to FK)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS participants (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            user_id INTEGER REFERENCES users(id),
            default_weight REAL NOT NULL DEFAULT 1.0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Project members table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS project_members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'editor',
            participant_id INTEGER REFERENCES participants(id),
            joined_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(project_id, user_id)
        )",
    )
    .execute(pool)
    .await?;

    // Payments table (with project_id)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS payments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
            payer_id INTEGER REFERENCES participants(id) ON DELETE SET NULL,
            amount REAL NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            payment_date TEXT NOT NULL DEFAULT (datetime('now')),
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Contributions table (references participant_id)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS contributions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            participant_id INTEGER NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
            payment_id INTEGER NOT NULL REFERENCES payments(id) ON DELETE CASCADE,
            amount REAL NOT NULL,
            weight REAL NOT NULL DEFAULT 1.0,
            UNIQUE(participant_id, payment_id)
        )",
    )
    .execute(pool)
    .await?;

    // Indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_project_members_project ON project_members(project_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_project_members_user ON project_members(user_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_participants_project ON participants(project_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_participants_user ON participants(user_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_payments_project ON payments(project_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_payments_payer ON payments(payer_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_payments_date ON payments(payment_date)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_contributions_participant ON contributions(participant_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_contributions_payment ON contributions(payment_id)",
    )
    .execute(pool)
    .await?;

    // Unique constraint: one user can only be linked to one participant per project
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_participants_project_user ON participants(project_id, user_id) WHERE user_id IS NOT NULL")
        .execute(pool)
        .await?;

    // =====================
    // Migration 003: Payment enhancements (receipt image, recurrence)
    // =====================

    // Add receipt_image column if not exists
    sqlx::query("ALTER TABLE payments ADD COLUMN receipt_image TEXT")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // Add recurrence columns if not exist
    sqlx::query("ALTER TABLE payments ADD COLUMN is_recurring INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await
        .ok();

    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_type TEXT")
        .execute(pool)
        .await
        .ok();

    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_interval INTEGER DEFAULT 1")
        .execute(pool)
        .await
        .ok();

    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_times_per INTEGER")
        .execute(pool)
        .await
        .ok();

    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_end_date TEXT")
        .execute(pool)
        .await
        .ok();

    // =====================
    // Migration 004: User Management & Advanced Invite System
    // =====================

    // Add project settings for invite control
    sqlx::query("ALTER TABLE projects ADD COLUMN invites_enabled INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await
        .ok();

    sqlx::query("ALTER TABLE projects ADD COLUMN require_approval INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await
        .ok();

    // Add status to project_members for approval workflow
    sqlx::query("ALTER TABLE project_members ADD COLUMN status TEXT NOT NULL DEFAULT 'active'")
        .execute(pool)
        .await
        .ok();

    // Add invite tokens for participant-specific invites
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS participant_invites (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            participant_id INTEGER NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
            invite_token TEXT UNIQUE NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT,
            used_by INTEGER REFERENCES users(id),
            used_at TEXT,
            UNIQUE(participant_id)
        )",
    )
    .execute(pool)
    .await?;

    // Index for looking up invites by token
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_participant_invites_token ON participant_invites(invite_token)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_participant_invites_participant ON participant_invites(participant_id)")
        .execute(pool)
        .await?;

    // =====================
    // Migration 005: Pool Account Support
    // =====================

    // Add account_type column to participants (user or pool)
    sqlx::query("ALTER TABLE participants ADD COLUMN account_type TEXT NOT NULL DEFAULT 'user'")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // =====================
    // Migration 006: Internal Transfer Support
    // =====================

    // Add receiver_account_id to payments for internal transfers
    // NULL = external expense (money leaves system)
    // NOT NULL = internal transfer (money moves between accounts)
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN receiver_account_id INTEGER REFERENCES participants(id) ON DELETE SET NULL"
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // =====================
    // Migration 007: User State & Token Versioning
    // =====================

    // Add user_state column (ACTIVE, PENDING_APPROVAL, REVOKED)
    sqlx::query(
        "ALTER TABLE users ADD COLUMN user_state TEXT NOT NULL DEFAULT 'active' CHECK(user_state IN ('active', 'pending_approval', 'revoked'))"
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // Add token_version for JWT invalidation
    sqlx::query("ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // =====================
    // Migration 008: Enhanced Recurrence Patterns
    // =====================
    // Store selected weekdays for weekly recurrence (JSON array of arrays)
    // Example: [[1,3],[0,5]] means week 1: Mon/Wed, week 2: Sun/Fri
    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_weekdays TEXT")
        .execute(pool)
        .await
        .ok();

    // Store selected days for monthly recurrence (JSON array)
    // Example: [1, 15] means 1st and 15th of each month
    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_monthdays TEXT")
        .execute(pool)
        .await
        .ok();

    // Store selected months for yearly recurrence (JSON array)
    // Example: [1, 6, 12] means January, June, December
    sqlx::query("ALTER TABLE payments ADD COLUMN recurrence_months TEXT")
        .execute(pool)
        .await
        .ok();

    // =====================
    // Migration 009: Immutable History Log
    // =====================
    // Append-only audit log for all state-changing operations
    // Hash-chained for tamper detection
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS history_log (
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
        )",
    )
    .execute(pool)
    .await?;

    // Append-only enforcement: prevent UPDATE
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS history_no_update
        BEFORE UPDATE ON history_log
        BEGIN
            SELECT RAISE(FAIL, 'history_log is append-only');
        END",
    )
    .execute(pool)
    .await?;

    // Append-only enforcement: prevent DELETE
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS history_no_delete
        BEFORE DELETE ON history_log
        BEGIN
            SELECT RAISE(FAIL, 'history_log is append-only');
        END",
    )
    .execute(pool)
    .await?;

    // Indexes for efficient querying
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_project ON history_log(project_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_history_entity ON history_log(entity_type, entity_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_history_correlation ON history_log(correlation_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_created ON history_log(created_at)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_undoes ON history_log(undoes_history_id)")
        .execute(pool)
        .await?;

    // =====================
    // Migration 010: User Preferences (i18n & Formatting)
    // =====================

    // Add language preference (e.g., 'en', 'fr')
    sqlx::query("ALTER TABLE users ADD COLUMN language TEXT")
        .execute(pool)
        .await
        .ok();

    // Add date format preference ('mdy', 'dmy', 'ymd', 'iso')
    sqlx::query("ALTER TABLE users ADD COLUMN date_format TEXT")
        .execute(pool)
        .await
        .ok();

    // Add decimal separator preference ('.' or ',')
    sqlx::query("ALTER TABLE users ADD COLUMN decimal_separator TEXT")
        .execute(pool)
        .await
        .ok();

    // Add currency symbol (e.g., '$', 'EUR', 'CAD')
    sqlx::query("ALTER TABLE users ADD COLUMN currency_symbol TEXT")
        .execute(pool)
        .await
        .ok();

    // Add currency symbol position ('before' or 'after')
    sqlx::query("ALTER TABLE users ADD COLUMN currency_symbol_position TEXT")
        .execute(pool)
        .await
        .ok();

    // =====================
    // Migration 011: Fix internal transfer contributions
    // =====================
    // Internal transfers (user-to-user) should have only the payer as contributor
    // This migration fixes any transfers that were incorrectly saved with other contributions

    // First, delete all existing contributions for internal transfers
    sqlx::query(
        "DELETE FROM contributions WHERE payment_id IN (
            SELECT id FROM payments
            WHERE receiver_account_id IS NOT NULL
            AND payer_id IS NOT NULL
        )",
    )
    .execute(pool)
    .await
    .ok();

    // Then, insert the correct contribution (payer with weight 1, amount = payment amount)
    // Use INSERT OR IGNORE to make this idempotent
    sqlx::query(
        "INSERT OR IGNORE INTO contributions (participant_id, payment_id, amount, weight)
        SELECT payer_id, id, amount, 1.0
        FROM payments
        WHERE receiver_account_id IS NOT NULL
        AND payer_id IS NOT NULL",
    )
    .execute(pool)
    .await
    .ok();

    // =====================
    // Migration 012: Payment Status (Draft Support)
    // =====================

    // Add status column to payments (final or draft)
    // Existing payments default to 'final' to maintain backwards compatibility
    sqlx::query("ALTER TABLE payments ADD COLUMN status TEXT NOT NULL DEFAULT 'final'")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // Index for efficient filtering by status
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status)")
        .execute(pool)
        .await?;

    // =====================
    // Migration 013: Pool Warning Horizon Setting
    // =====================

    // Add pool_warning_horizon column to projects
    // Configures how far ahead to check for pool going negative
    // Default: 'end_of_next_month' means check from today until end of next calendar month
    sqlx::query(
        "ALTER TABLE projects ADD COLUMN pool_warning_horizon TEXT NOT NULL DEFAULT 'end_of_next_month'",
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // =====================
    // Migration 014: Per-Pool Warning Settings
    // =====================

    // Add warning horizon settings to participants table (only used for pool accounts)
    // warning_horizon_account: How far ahead to warn if the pool total goes negative
    //   NULL = disabled (no warning)
    //   'end_of_current_month', 'end_of_next_month', '3_months', '6_months'
    sqlx::query(
        "ALTER TABLE participants ADD COLUMN warning_horizon_account TEXT DEFAULT 'end_of_next_month'",
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // warning_horizon_users: How far ahead to warn if any user's ownership goes negative
    //   NULL = disabled (no warning for user ownership)
    //   'end_of_current_month', 'end_of_next_month', '3_months', '6_months'
    sqlx::query(
        "ALTER TABLE participants ADD COLUMN warning_horizon_users TEXT DEFAULT 'end_of_next_month'",
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // =====================
    // Migration 015: Cleanup from failed migration attempts
    // =====================
    // Note: We no longer remove pool_warning_horizon from projects table
    // The column is simply unused - removing it caused too many issues with SQLite

    // Clean up any leftover projects_new table from failed migration attempts
    sqlx::query("DROP TABLE IF EXISTS projects_new")
        .execute(pool)
        .await
        .ok();

    // =====================
    // Migration 016: Dual Ledger Flags for Pool Expected Minimum
    // =====================
    // Two independent flags to control which ledger a transaction affects:
    // - affects_balance (default 1): Transaction affects actual pool balance
    // - affects_expectation (default 0): Transaction affects expected minimum
    //
    // Combinations:
    // - (1, 0): Normal transaction - affects balance only (default, current behavior)
    // - (0, 1): Rule only - sets expected minimum without moving money
    // - (1, 1): Protected - affects both balance and expected minimum
    // - (0, 0): No effect (not useful)

    sqlx::query("ALTER TABLE payments ADD COLUMN affects_balance INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    sqlx::query("ALTER TABLE payments ADD COLUMN affects_expectation INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // =====================
    // Migration 017: Convert status to is_final boolean
    // =====================
    // Convert 'status' TEXT field ('final'/'draft') to 'is_final' INTEGER (1/0)
    // Also add CHECK constraints for all boolean fields

    // Step 1: Add is_final column
    sqlx::query("ALTER TABLE payments ADD COLUMN is_final INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // Step 2: Migrate data from status to is_final
    // 'final' -> 1, 'draft' -> 0
    sqlx::query("UPDATE payments SET is_final = CASE WHEN status = 'final' THEN 1 ELSE 0 END WHERE is_final = 1 AND status = 'draft'")
        .execute(pool)
        .await
        .ok();

    // Note: SQLite doesn't support adding CHECK constraints to existing tables via ALTER TABLE.
    // The CHECK constraints would need to be added when creating the table.
    // For existing databases, the application layer enforces the boolean constraints.
    // New databases created from scratch could have these constraints in the initial CREATE TABLE.

    // =====================
    // Migration 018: Separate payer and receiver expectation flags
    // =====================
    // Split the single 'affects_expectation' flag into two independent flags:
    // - affects_payer_expectation: When payer is a pool and this is true, reduces payer's expected minimum
    //   (Used for "Approved" withdrawals from pools)
    // - affects_receiver_expectation: When receiver is a pool and this is true, increases receiver's expected minimum
    //   (Used for "Earmarked" deposits to pools)
    //
    // This allows pool-to-pool transfers to independently affect each pool's expected minimum.

    // Add affects_payer_expectation column (for approved pool withdrawals)
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN affects_payer_expectation INTEGER NOT NULL DEFAULT 0",
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // Add affects_receiver_expectation column (for earmarked pool deposits)
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN affects_receiver_expectation INTEGER NOT NULL DEFAULT 0",
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // Migrate data from old affects_expectation to new affects_receiver_expectation
    // The old affects_expectation was used for deposits to pools (earmarking)
    sqlx::query(
        "UPDATE payments SET affects_receiver_expectation = affects_expectation WHERE affects_expectation = 1",
    )
    .execute(pool)
    .await
    .ok();

    // =====================
    // Migration 019: Project Approvals System
    // =====================
    // Used for approving user actions like password changes within projects
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS project_approvals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            event_type TEXT NOT NULL,
            event_metadata TEXT,
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'approved', 'rejected')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS approval_votes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            approval_id INTEGER NOT NULL REFERENCES project_approvals(id) ON DELETE CASCADE,
            voter_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            vote TEXT NOT NULL CHECK(vote IN ('approve', 'reject')),
            reason TEXT,
            voted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(approval_id, voter_id)
        )",
    )
    .execute(pool)
    .await?;

    // Indexes for approval queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_project_approvals_user ON project_approvals(user_id, status)")
        .execute(pool)
        .await
        .ok();

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_project_approvals_project ON project_approvals(project_id, status)")
        .execute(pool)
        .await
        .ok();

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_approval_votes_approval ON approval_votes(approval_id)",
    )
    .execute(pool)
    .await
    .ok();

    // =====================
    // Migration 020: Trusted Users for Password Recovery
    // =====================
    // Asymmetric trust relationships for account recovery
    // If user A trusts user B, B can help A recover their password
    // B trusting A requires a separate entry
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS trusted_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            trusted_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(user_id, trusted_user_id),
            CHECK(user_id != trusted_user_id)
        )",
    )
    .execute(pool)
    .await?;

    // Index for efficient lookup of trusted users
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_trusted_users_user ON trusted_users(user_id)")
        .execute(pool)
        .await?;

    // Index for finding who trusts a given user (for admin/stats)
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_trusted_users_trusted ON trusted_users(trusted_user_id)",
    )
    .execute(pool)
    .await?;

    // =====================
    // Migration 021: Recovery Intents for Password Recovery
    // =====================
    // Stores password recovery requests that need approval from trusted users
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS recovery_intents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'approved', 'rejected', 'expired', 'used')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    // Votes from trusted users on recovery intents
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS recovery_votes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recovery_id INTEGER NOT NULL REFERENCES recovery_intents(id) ON DELETE CASCADE,
            voter_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            vote TEXT NOT NULL CHECK(vote IN ('approve', 'reject')),
            voted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(recovery_id, voter_id)
        )",
    )
    .execute(pool)
    .await?;

    // Index for efficient lookup of recovery intents by token
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recovery_intents_token ON recovery_intents(token)")
        .execute(pool)
        .await
        .ok();

    // Index for finding recovery intents by user
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_recovery_intents_user ON recovery_intents(user_id, status)",
    )
    .execute(pool)
    .await
    .ok();

    // Migration 022: Support 'recovered' status for project_members
    // =====================
    // When a user recovers their password via trusted users, their project memberships
    // are set to 'recovered' status. Project admins must re-approve them.
    // Valid statuses are now: 'active', 'pending', 'recovered'
    // Note: SQLite doesn't easily support modifying CHECK constraints, but since the
    // project_members.status column was added via ALTER TABLE (Migration 006), it
    // doesn't have a CHECK constraint. We just document the valid values here.
    // This migration is a no-op but documents the schema change.

    // Migration 023: Configurable Pending Member Access
    // =====================
    // Add a setting to control how pending members can access the project:
    // - 'none': No access until approved (most restrictive)
    // - 'read_only': View-only access until approved (default)
    // - 'auto_approve': Automatically approve on join (bypasses approval)
    sqlx::query(
        "ALTER TABLE projects ADD COLUMN pending_member_access TEXT NOT NULL DEFAULT 'read_only'",
    )
    .execute(pool)
    .await
    .ok();

    // =====================
    // Migration 024: Remove deprecated columns from payments table
    // =====================
    // Remove deprecated columns:
    // - status (replaced by is_final)
    // - affects_expectation (replaced by affects_payer_expectation and affects_receiver_expectation)
    //
    // Note: SQLite 3.35+ supports ALTER TABLE DROP COLUMN natively.
    // We must drop any indexes on these columns first before dropping them.

    // Check if payments table has deprecated columns
    let has_deprecated: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('payments')
         WHERE name IN ('status', 'affects_expectation')",
    )
    .fetch_optional(pool)
    .await?;

    if has_deprecated.unwrap_or(0) > 0 {
        tracing::info!("Removing deprecated columns from payments table");

        // Drop any indexes on deprecated columns (they won't be valid after column removal)
        sqlx::query("DROP INDEX IF EXISTS idx_payments_status")
            .execute(pool)
            .await?;

        // Drop the deprecated columns using SQLite 3.35+ native support
        sqlx::query("ALTER TABLE payments DROP COLUMN status")
            .execute(pool)
            .await?;

        sqlx::query("ALTER TABLE payments DROP COLUMN affects_expectation")
            .execute(pool)
            .await?;

        // Verify the columns are gone
        let still_deprecated: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('payments')
             WHERE name IN ('status', 'affects_expectation')",
        )
        .fetch_optional(pool)
        .await?;

        if still_deprecated.unwrap_or(0) == 0 {
            tracing::info!("Deprecated columns successfully removed from payments table");
        } else {
            tracing::warn!("Some deprecated columns still exist after migration attempt");
        }
    }

    tracing::info!("Database migrations completed");
    Ok(())
}
