//! BonsCompte Admin CLI
//!
//! Usage:
//!   bonscompte-admin reset-password <username>
//!   bonscompte-admin approve <username>
//!   bonscompte-admin revoke <username>
//!   bonscompte-admin list-users

use clap::{Parser, Subcommand};
use rand::Rng;

use bonscompte_backend::{
    auth::password::hash_password,
    config::Config,
    db,
    models::UserState,
};

#[derive(Parser)]
#[command(name = "bonscompte-admin")]
#[command(about = "BonsCompte administration CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Reset a user's password (generates temp password, sets state to pending_approval)
    ResetPassword {
        /// Username to reset
        username: String,
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
}

fn generate_temp_password() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789!@#$%";
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
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
        Commands::ResetPassword { username } => {
            // Check if user exists
            let user: Option<(i64, String, i64)> = sqlx::query_as(
                "SELECT id, user_state, token_version FROM users WHERE username = ?"
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

            // Generate temporary password
            let temp_password = generate_temp_password();
            let password_hash = hash_password(&temp_password)?;

            // Update user: new password hash, state = pending_approval, increment token_version
            let new_version = current_version + 1;
            sqlx::query(
                "UPDATE users SET password_hash = ?, user_state = ?, token_version = ? WHERE id = ?"
            )
            .bind(&password_hash)
            .bind(UserState::PendingApproval.as_str())
            .bind(new_version)
            .bind(user_id)
            .execute(&pool)
            .await?;

            println!("Password reset for user '{}'", username);
            println!("Temporary password: {}", temp_password);
            println!("Previous state: {}", current_state);
            println!("New state: pending_approval");
            println!("Token version: {} -> {}", current_version, new_version);
            println!();
            println!("IMPORTANT: The user must:");
            println!("  1. Log in with this temporary password");
            println!("  2. Wait for admin approval (run: bonscompte-admin approve {})", username);
        }

        Commands::Approve { username } => {
            // Check if user exists and is pending
            let user: Option<(i64, String, i64)> = sqlx::query_as(
                "SELECT id, user_state, token_version FROM users WHERE username = ?"
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

            println!("User '{}' approved", username);
            println!("Previous state: {}", current_state);
            println!("New state: active");
            println!("Token version: {} (unchanged)", current_version);
        }

        Commands::Revoke { username } => {
            // Check if user exists
            let user: Option<(i64, String, i64)> = sqlx::query_as(
                "SELECT id, user_state, token_version FROM users WHERE username = ?"
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
            println!("Token version: {} -> {} (all tokens invalidated)", current_version, new_version);
        }

        Commands::ListUsers => {
            let users: Vec<(i64, String, Option<String>, String, i64, String)> = sqlx::query_as(
                "SELECT id, username, display_name, user_state, token_version, created_at FROM users ORDER BY id"
            )
            .fetch_all(&pool)
            .await?;

            if users.is_empty() {
                println!("No users found");
            } else {
                println!("{:<5} {:<20} {:<20} {:<18} {:<8} {}",
                    "ID", "Username", "Display Name", "State", "TokVer", "Created");
                println!("{}", "-".repeat(90));
                for (id, username, display_name, state, token_version, created_at) in users {
                    println!("{:<5} {:<20} {:<20} {:<18} {:<8} {}",
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
    }

    Ok(())
}
