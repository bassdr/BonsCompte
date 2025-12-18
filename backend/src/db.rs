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

    tracing::info!("Database migrations completed");
    Ok(())
}
