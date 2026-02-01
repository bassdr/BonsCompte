//! BonsCompte Admin CLI
//!
//! Usage:
//!   bonscompte-admin recovery list              # List pending recovery requests
//!   bonscompte-admin recovery approve <username> # Approve recovery (bypasses trusted users)
//!   bonscompte-admin recovery block <username>   # Block recovery request permanently
//!   bonscompte-admin approve <username>          # Approve a user (sets state to active)
//!   bonscompte-admin revoke <username>           # Revoke a user's access
//!   bonscompte-admin list-users                  # List all users
//!   bonscompte-admin merge-users <source> <target> # Merge source user into target user

use clap::{Parser, Subcommand};

use bonscompte_backend::{config::Config, db, models::UserState};

#[derive(Parser)]
#[command(name = "bonscompte-admin")]
#[command(about = "BonsCompte administration CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage password recovery requests
    Recovery {
        #[command(subcommand)]
        action: RecoveryCommands,
    },
    /// Approve a user (sets state to active)
    Approve {
        /// Username to approve
        username: String,
    },
    /// Revoke a user's access (sets state to revoked)
    Revoke {
        /// Username to revoke
        username: String,
    },
    /// List all users with their states
    ListUsers,
    /// Merge two users into one (keeps projects from both)
    MergeUsers {
        /// Username to merge FROM (will be deactivated)
        source: String,
        /// Username to merge INTO (will receive all projects)
        target: String,
    },
}

#[derive(Subcommand)]
enum RecoveryCommands {
    /// List all pending recovery requests with statistics
    List,
    /// Approve a recovery request (bypasses trusted user approval)
    Approve {
        /// Username whose recovery to approve
        username: String,
    },
    /// Block a recovery request permanently (cannot be approved even by trusted users)
    Block {
        /// Username whose recovery to block
        username: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_env();

    // Initialize database
    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Recovery { action } => match action {
            RecoveryCommands::List => {
                // Get statistics
                let stats: (i64, i64, i64, i64) = sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(*) as total,
                        SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending,
                        SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END) as approved,
                        SUM(CASE WHEN status = 'blocked' THEN 1 ELSE 0 END) as blocked
                    FROM recovery_intents
                    WHERE created_at > datetime('now', '-24 hours')
                    "#,
                )
                .fetch_one(&pool)
                .await?;

                let (total, pending, approved, blocked) = stats;

                println!("=== Recovery Request Statistics (Last 24 hours) ===");
                println!("Total requests: {}", total);
                println!("Pending: {}", pending);
                println!("Approved: {}", approved);
                println!("Blocked: {}", blocked);

                // Attack detection heuristics
                if total > 10 {
                    println!();
                    println!("⚠️  WARNING: High volume of recovery requests detected!");
                    println!("   This may indicate a brute-force or enumeration attack.");
                }

                // Get requests per user (last 24h)
                let per_user: Vec<(String, i64)> = sqlx::query_as(
                    r#"
                    SELECT u.username, COUNT(*) as count
                    FROM recovery_intents ri
                    JOIN users u ON ri.user_id = u.id
                    WHERE ri.created_at > datetime('now', '-24 hours')
                    GROUP BY ri.user_id
                    HAVING count > 1
                    ORDER BY count DESC
                    "#,
                )
                .fetch_all(&pool)
                .await?;

                if !per_user.is_empty() {
                    println!();
                    println!("⚠️  Users with multiple recovery attempts:");
                    for (username, count) in per_user {
                        println!("   {} - {} attempts", username, count);
                    }
                }

                println!();
                println!("=== Pending Recovery Requests ===");

                // List pending recovery requests
                let requests: Vec<(i64, String, Option<String>, String, String, String)> =
                    sqlx::query_as(
                        r#"
                    SELECT ri.id, u.username, u.display_name, ri.status, ri.created_at, ri.expires_at
                    FROM recovery_intents ri
                    JOIN users u ON ri.user_id = u.id
                    WHERE ri.status = 'pending'
                    ORDER BY ri.created_at DESC
                    "#,
                    )
                    .fetch_all(&pool)
                    .await?;

                if requests.is_empty() {
                    println!("No pending recovery requests.");
                } else {
                    println!(
                        "{:<5} {:<20} {:<20} {:<10} {:<20} Expires",
                        "ID", "Username", "Display Name", "Status", "Created"
                    );
                    println!("{}", "-".repeat(95));

                    for (id, username, display_name, status, created_at, expires_at) in &requests {
                        // Get vote counts
                        let votes: (i64, i64) = sqlx::query_as(
                            r#"
                            SELECT
                                SUM(CASE WHEN vote = 'approve' THEN 1 ELSE 0 END) as approvals,
                                SUM(CASE WHEN vote = 'reject' THEN 1 ELSE 0 END) as rejections
                            FROM recovery_votes
                            WHERE recovery_id = ?
                            "#,
                        )
                        .bind(id)
                        .fetch_one(&pool)
                        .await
                        .unwrap_or((0, 0));

                        println!(
                            "{:<5} {:<20} {:<20} {:<10} {:<20} {}",
                            id,
                            username,
                            display_name.clone().unwrap_or_else(|| "-".to_string()),
                            status,
                            created_at,
                            expires_at
                        );
                        println!(
                            "      Votes: {} approve, {} reject (need 2 to approve)",
                            votes.0, votes.1
                        );
                    }
                }

                println!();
                println!("Commands:");
                println!("  bonscompte-admin recovery approve <username>  # Approve request");
                println!("  bonscompte-admin recovery block <username>    # Block request");
            }

            RecoveryCommands::Approve { username } => {
                // Find the pending recovery intent for this user
                let intent: Option<(i64, i64, String)> = sqlx::query_as(
                    r#"
                    SELECT ri.id, ri.user_id, ri.status
                    FROM recovery_intents ri
                    JOIN users u ON ri.user_id = u.id
                    WHERE u.username = ? AND ri.status = 'pending'
                    ORDER BY ri.created_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(&username)
                .fetch_optional(&pool)
                .await?;

                let (intent_id, user_id, _status) = match intent {
                    Some(i) => i,
                    None => {
                        eprintln!(
                            "Error: No pending recovery request found for user '{}'",
                            username
                        );
                        std::process::exit(1);
                    }
                };

                // Mark recovery as approved (admin bypass)
                sqlx::query(
                    "UPDATE recovery_intents SET status = 'approved', resolved_at = datetime('now') WHERE id = ?",
                )
                .bind(intent_id)
                .execute(&pool)
                .await?;

                println!("✓ Recovery request approved for user '{}'", username);
                println!("  The user can now set a new password at:");
                println!("  /recovery/reset/<token>");
                println!();
                println!("Note: User's project memberships will be set to 'recovered' status");
                println!("      after they reset their password. Project admins may need to");
                println!(
                    "      re-approve them, depending on each project's pending_member_access setting."
                );

                // Log the admin action
                println!();
                println!(
                    "Admin action logged: recovery approved for user_id={}",
                    user_id
                );
            }

            RecoveryCommands::Block { username } => {
                // Find the pending recovery intent for this user
                let intent: Option<(i64, i64, String)> = sqlx::query_as(
                    r#"
                    SELECT ri.id, ri.user_id, ri.status
                    FROM recovery_intents ri
                    JOIN users u ON ri.user_id = u.id
                    WHERE u.username = ? AND ri.status = 'pending'
                    ORDER BY ri.created_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(&username)
                .fetch_optional(&pool)
                .await?;

                let (intent_id, user_id, _status) = match intent {
                    Some(i) => i,
                    None => {
                        eprintln!(
                            "Error: No pending recovery request found for user '{}'",
                            username
                        );
                        std::process::exit(1);
                    }
                };

                // Mark recovery as blocked
                sqlx::query(
                    "UPDATE recovery_intents SET status = 'blocked', resolved_at = datetime('now') WHERE id = ?",
                )
                .bind(intent_id)
                .execute(&pool)
                .await?;

                println!("✗ Recovery request BLOCKED for user '{}'", username);
                println!("  This request cannot be approved, even by trusted users.");
                println!();
                println!(
                    "  If this was a legitimate request, the user must initiate a new recovery."
                );

                // Log the admin action
                println!();
                println!(
                    "Admin action logged: recovery blocked for user_id={}",
                    user_id
                );
            }
        },

        Commands::Approve { username } => {
            // Check if user exists and is pending
            let user: Option<(i64, String, i64)> = sqlx::query_as(
                "SELECT id, user_state, token_version FROM users WHERE username = ?",
            )
            .bind(&username)
            .fetch_optional(&pool)
            .await?;

            let (user_id, current_state, current_version) = match user {
                Some(u) => u,
                None => {
                    eprintln!("Error: User '{}' not found", username);
                    std::process::exit(1);
                }
            };

            if current_state == UserState::Active.as_str() {
                println!("User '{}' is already active", username);
                return Ok(());
            }

            // Update user state to active
            sqlx::query("UPDATE users SET user_state = ? WHERE id = ?")
                .bind(UserState::Active.as_str())
                .bind(user_id)
                .execute(&pool)
                .await?;

            // Activate ALL project memberships (system admin bypass)
            let rows_updated =
                sqlx::query("UPDATE project_members SET status = 'active' WHERE user_id = ?")
                    .bind(user_id)
                    .execute(&pool)
                    .await?
                    .rows_affected();

            println!("User '{}' approved", username);
            println!("Previous state: {}", current_state);
            println!("New state: active");
            println!("Token version: {} (unchanged)", current_version);
            println!("Project memberships activated: {}", rows_updated);
        }

        Commands::Revoke { username } => {
            // Check if user exists
            let user: Option<(i64, String, i64)> = sqlx::query_as(
                "SELECT id, user_state, token_version FROM users WHERE username = ?",
            )
            .bind(&username)
            .fetch_optional(&pool)
            .await?;

            let (user_id, current_state, current_version) = match user {
                Some(u) => u,
                None => {
                    eprintln!("Error: User '{}' not found", username);
                    std::process::exit(1);
                }
            };

            if current_state == UserState::Revoked.as_str() {
                println!("User '{}' is already revoked", username);
                return Ok(());
            }

            // Update user state to revoked and increment token_version to invalidate tokens
            let new_version = current_version + 1;
            sqlx::query("UPDATE users SET user_state = ?, token_version = ? WHERE id = ?")
                .bind(UserState::Revoked.as_str())
                .bind(new_version)
                .bind(user_id)
                .execute(&pool)
                .await?;

            println!("User '{}' revoked", username);
            println!("Previous state: {}", current_state);
            println!("New state: revoked");
            println!(
                "Token version: {} -> {} (all tokens invalidated)",
                current_version, new_version
            );
        }

        Commands::ListUsers => {
            let users: Vec<(i64, String, Option<String>, String, i64, String)> = sqlx::query_as(
                "SELECT id, username, display_name, user_state, token_version, created_at FROM users ORDER BY id",
            )
            .fetch_all(&pool)
            .await?;

            if users.is_empty() {
                println!("No users found");
            } else {
                println!(
                    "{:<5} {:<20} {:<20} {:<18} {:<8} Created",
                    "ID", "Username", "Display Name", "State", "TokVer"
                );
                println!("{}", "-".repeat(90));
                for (id, username, display_name, state, token_version, created_at) in users {
                    println!(
                        "{:<5} {:<20} {:<20} {:<18} {:<8} {}",
                        id,
                        username,
                        display_name.unwrap_or_else(|| "-".to_string()),
                        state,
                        token_version,
                        created_at
                    );
                }
            }
        }

        Commands::MergeUsers { source, target } => {
            // Validate source and target are different
            if source == target {
                eprintln!("Error: Source and target users must be different");
                std::process::exit(1);
            }

            // Get source user
            let source_user: Option<(i64, String, Option<String>)> =
                sqlx::query_as("SELECT id, username, display_name FROM users WHERE username = ?")
                    .bind(&source)
                    .fetch_optional(&pool)
                    .await?;

            let (source_id, source_username, source_display) = match source_user {
                Some(u) => u,
                None => {
                    eprintln!("Error: Source user '{}' not found", source);
                    std::process::exit(1);
                }
            };

            // Get target user
            let target_user: Option<(i64, String, Option<String>)> =
                sqlx::query_as("SELECT id, username, display_name FROM users WHERE username = ?")
                    .bind(&target)
                    .fetch_optional(&pool)
                    .await?;

            let (target_id, target_username, target_display) = match target_user {
                Some(u) => u,
                None => {
                    eprintln!("Error: Target user '{}' not found", target);
                    std::process::exit(1);
                }
            };

            println!("=== Merge Users ===");
            println!(
                "Source: {} (id={}, display={})",
                source_username,
                source_id,
                source_display.clone().unwrap_or_else(|| "-".to_string())
            );
            println!(
                "Target: {} (id={}, display={})",
                target_username,
                target_id,
                target_display.unwrap_or_else(|| "-".to_string())
            );
            println!();

            // Start transaction
            let mut tx = pool.begin().await?;

            // 1. Transfer project memberships (skip if target already member)
            let source_memberships: Vec<(i64, String, Option<i64>, String)> = sqlx::query_as(
                r#"
                SELECT pm.project_id, pm.role, pm.participant_id, pm.status
                FROM project_members pm
                WHERE pm.user_id = ?
                "#,
            )
            .bind(source_id)
            .fetch_all(&mut *tx)
            .await?;

            let mut memberships_transferred = 0;
            let mut memberships_skipped = 0;

            for (project_id, role, participant_id, status) in source_memberships {
                // Check if target is already a member
                let existing: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM project_members WHERE project_id = ? AND user_id = ?",
                )
                .bind(project_id)
                .bind(target_id)
                .fetch_optional(&mut *tx)
                .await?;

                if existing.is_some() {
                    // Target already member - just delete source membership
                    sqlx::query("DELETE FROM project_members WHERE project_id = ? AND user_id = ?")
                        .bind(project_id)
                        .bind(source_id)
                        .execute(&mut *tx)
                        .await?;
                    memberships_skipped += 1;

                    // If source had a linked participant, transfer it to target
                    if let Some(pid) = participant_id {
                        // Check if target already has a participant in this project
                        let target_has_participant: Option<(i64,)> = sqlx::query_as(
                            "SELECT participant_id FROM project_members WHERE project_id = ? AND user_id = ? AND participant_id IS NOT NULL",
                        )
                        .bind(project_id)
                        .bind(target_id)
                        .fetch_optional(&mut *tx)
                        .await?;

                        if target_has_participant.is_none() {
                            // Transfer participant link to target
                            sqlx::query(
                                "UPDATE project_members SET participant_id = ? WHERE project_id = ? AND user_id = ?",
                            )
                            .bind(pid)
                            .bind(project_id)
                            .bind(target_id)
                            .execute(&mut *tx)
                            .await?;

                            // Update participant's linked_user_id
                            sqlx::query("UPDATE participants SET linked_user_id = ? WHERE id = ?")
                                .bind(target_id)
                                .bind(pid)
                                .execute(&mut *tx)
                                .await?;
                        }
                    }
                } else {
                    // Transfer membership to target
                    sqlx::query(
                        "UPDATE project_members SET user_id = ? WHERE project_id = ? AND user_id = ?",
                    )
                    .bind(target_id)
                    .bind(project_id)
                    .bind(source_id)
                    .execute(&mut *tx)
                    .await?;

                    // Update participant's linked_user_id if exists
                    if let Some(pid) = participant_id {
                        sqlx::query("UPDATE participants SET linked_user_id = ? WHERE id = ?")
                            .bind(target_id)
                            .bind(pid)
                            .execute(&mut *tx)
                            .await?;
                    }

                    memberships_transferred += 1;
                }

                // Get project name for logging
                let project_name: Option<(String,)> =
                    sqlx::query_as("SELECT name FROM projects WHERE id = ?")
                        .bind(project_id)
                        .fetch_optional(&mut *tx)
                        .await?;

                println!(
                    "  Project '{}' (id={}): role={}, status={} -> {}",
                    project_name.map(|p| p.0).unwrap_or_else(|| "?".to_string()),
                    project_id,
                    role,
                    status,
                    if existing.is_some() {
                        "skipped (target already member)"
                    } else {
                        "transferred"
                    }
                );
            }

            // 2. Transfer trusted users relationships
            // Source as trustee (people who trust source)
            let trustees_updated = sqlx::query(
                "UPDATE trusted_users SET trusted_user_id = ? WHERE trusted_user_id = ? AND user_id != ?",
            )
            .bind(target_id)
            .bind(source_id)
            .bind(target_id) // Don't create self-trust
            .execute(&mut *tx)
            .await?
            .rows_affected();

            // Source as truster (people source trusts) - transfer to target
            let trusters_updated = sqlx::query(
                "UPDATE trusted_users SET user_id = ? WHERE user_id = ? AND trusted_user_id != ?",
            )
            .bind(target_id)
            .bind(source_id)
            .bind(target_id) // Don't create self-trust
            .execute(&mut *tx)
            .await?
            .rows_affected();

            // Delete any remaining trusted_users entries for source (including duplicates)
            sqlx::query("DELETE FROM trusted_users WHERE user_id = ? OR trusted_user_id = ?")
                .bind(source_id)
                .bind(source_id)
                .execute(&mut *tx)
                .await?;

            // 3. Transfer any remaining participants linked to source
            let participants_updated =
                sqlx::query("UPDATE participants SET linked_user_id = ? WHERE linked_user_id = ?")
                    .bind(target_id)
                    .bind(source_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();

            // 4. Transfer project ownership (created_by)
            let projects_updated =
                sqlx::query("UPDATE projects SET created_by = ? WHERE created_by = ?")
                    .bind(target_id)
                    .bind(source_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();

            // 5. Deactivate source user (revoke access)
            let new_version: i64 =
                sqlx::query_scalar("SELECT token_version FROM users WHERE id = ?")
                    .bind(source_id)
                    .fetch_one(&mut *tx)
                    .await?;

            sqlx::query("UPDATE users SET user_state = ?, token_version = ? WHERE id = ?")
                .bind(UserState::Revoked.as_str())
                .bind(new_version + 1)
                .bind(source_id)
                .execute(&mut *tx)
                .await?;

            // Commit transaction
            tx.commit().await?;

            println!();
            println!("=== Merge Complete ===");
            println!("Memberships transferred: {}", memberships_transferred);
            println!(
                "Memberships skipped (target already member): {}",
                memberships_skipped
            );
            println!(
                "Trusted user relationships updated: {}",
                trustees_updated + trusters_updated
            );
            println!("Participants re-linked: {}", participants_updated);
            println!("Projects ownership transferred: {}", projects_updated);
            println!();
            println!(
                "Source user '{}' has been revoked and can no longer log in.",
                source_username
            );
            println!(
                "Target user '{}' now has access to all projects from both accounts.",
                target_username
            );
        }
    }

    Ok(())
}
