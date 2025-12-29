use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

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

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&connect_url)
        .await?;

    // Enable foreign keys
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    // Enable WAL mode for better concurrency
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;

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
        )"
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
        )"
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
        )"
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
        )"
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
        )"
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
        )"
    )
    .execute(pool)
    .await?;

    // Indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_project_members_project ON project_members(project_id)")
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

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_contributions_participant ON contributions(participant_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_contributions_payment ON contributions(payment_id)")
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
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN receipt_image TEXT"
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // Add recurrence columns if not exist
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN is_recurring INTEGER NOT NULL DEFAULT 0"
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_type TEXT"
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_interval INTEGER DEFAULT 1"
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_times_per INTEGER"
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_end_date TEXT"
    )
    .execute(pool)
    .await
    .ok();

    // =====================
    // Migration 004: User Management & Advanced Invite System
    // =====================

    // Add project settings for invite control
    sqlx::query(
        "ALTER TABLE projects ADD COLUMN invites_enabled INTEGER NOT NULL DEFAULT 1"
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "ALTER TABLE projects ADD COLUMN require_approval INTEGER NOT NULL DEFAULT 0"
    )
    .execute(pool)
    .await
    .ok();

    // Add status to project_members for approval workflow
    sqlx::query(
        "ALTER TABLE project_members ADD COLUMN status TEXT NOT NULL DEFAULT 'active'"
    )
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
        )"
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
    sqlx::query(
        "ALTER TABLE participants ADD COLUMN account_type TEXT NOT NULL DEFAULT 'user'"
    )
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
    sqlx::query(
        "ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 1"
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    // =====================
    // Migration 008: Enhanced Recurrence Patterns
    // =====================
    // Store selected weekdays for weekly recurrence (JSON array of arrays)
    // Example: [[1,3],[0,5]] means week 1: Mon/Wed, week 2: Sun/Fri
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_weekdays TEXT"
    )
    .execute(pool)
    .await
    .ok();

    // Store selected days for monthly recurrence (JSON array)
    // Example: [1, 15] means 1st and 15th of each month
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_monthdays TEXT"
    )
    .execute(pool)
    .await
    .ok();

    // Store selected months for yearly recurrence (JSON array)
    // Example: [1, 6, 12] means January, June, December
    sqlx::query(
        "ALTER TABLE payments ADD COLUMN recurrence_months TEXT"
    )
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
        )"
    )
    .execute(pool)
    .await?;

    // Append-only enforcement: prevent UPDATE
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS history_no_update
        BEFORE UPDATE ON history_log
        BEGIN
            SELECT RAISE(FAIL, 'history_log is append-only');
        END"
    )
    .execute(pool)
    .await?;

    // Append-only enforcement: prevent DELETE
    sqlx::query(
        "CREATE TRIGGER IF NOT EXISTS history_no_delete
        BEFORE DELETE ON history_log
        BEGIN
            SELECT RAISE(FAIL, 'history_log is append-only');
        END"
    )
    .execute(pool)
    .await?;

    // Indexes for efficient querying
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_project ON history_log(project_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_entity ON history_log(entity_type, entity_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_correlation ON history_log(correlation_id)")
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

    tracing::info!("Database migrations completed");
    Ok(())
}
