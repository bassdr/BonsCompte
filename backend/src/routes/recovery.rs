use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::error::{AppError, ErrorCode};
use crate::models::{
    InitiateRecoveryRequest, InitiateRecoveryResponse, RecoveryIntent, RecoveryIntentStatus,
    RecoveryIntentWithInfo, RecoveryVoteRequest, ResetPasswordRequest,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/initiate", post(initiate_recovery))
        .route("/{token}/status", get(get_recovery_status))
        .route("/{token}/reset", post(reset_password))
        // These require authentication (for trusted users to vote)
        .route("/pending", get(get_pending_recoveries))
        .route("/{token}/vote", post(vote_on_recovery))
}

/// Initiate a password recovery request (no auth required)
async fn initiate_recovery(
    State(pool): State<SqlitePool>,
    Json(req): Json<InitiateRecoveryRequest>,
) -> Result<Json<InitiateRecoveryResponse>, AppError> {
    // Find the user by username
    let user = sqlx::query_as::<_, (i64, String)>(
        "SELECT id, username FROM users WHERE username = ? AND user_state = 'active'",
    )
    .bind(&req.username)
    .fetch_optional(&pool)
    .await?;

    let Some((user_id, _username)) = user else {
        // Don't reveal if user exists or not
        return Err(AppError::bad_request(
            ErrorCode::RecoveryInsufficientTrustedUsers,
        ));
    };

    // Check if user has enough trusted users
    let trusted_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM trusted_users WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;

    if trusted_count.0 < RecoveryIntent::REQUIRED_APPROVALS {
        // Don't reveal specific reason
        return Err(AppError::bad_request(
            ErrorCode::RecoveryInsufficientTrustedUsers,
        ));
    }

    // Check for existing pending recovery
    let existing = sqlx::query_as::<_, RecoveryIntent>(
        "SELECT * FROM recovery_intents WHERE user_id = ? AND status = 'pending' AND expires_at > datetime('now')",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;

    if let Some(intent) = existing {
        // Return existing intent info
        return Ok(Json(InitiateRecoveryResponse {
            message: "A recovery request is already pending.".to_string(),
            token: intent.token,
            expires_at: intent.expires_at,
        }));
    }

    // Expire any old pending intents
    sqlx::query(
        "UPDATE recovery_intents SET status = 'expired' WHERE user_id = ? AND status = 'pending' AND expires_at <= datetime('now')",
    )
    .bind(user_id)
    .execute(&pool)
    .await?;

    // Create new recovery intent
    let token = Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(RecoveryIntent::EXPIRY_MINUTES);
    let expires_at_str = expires_at.format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query("INSERT INTO recovery_intents (user_id, token, expires_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(&token)
        .bind(&expires_at_str)
        .execute(&pool)
        .await?;

    Ok(Json(InitiateRecoveryResponse {
        message: "Recovery request created. Contact your trusted users to approve it.".to_string(),
        token,
        expires_at: expires_at_str,
    }))
}

/// Get the status of a recovery intent (no auth required, by token)
async fn get_recovery_status(
    State(pool): State<SqlitePool>,
    Path(token): Path<String>,
) -> Result<Json<RecoveryIntentStatus>, AppError> {
    let intent =
        sqlx::query_as::<_, RecoveryIntent>("SELECT * FROM recovery_intents WHERE token = ?")
            .bind(&token)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::not_found(ErrorCode::RecoveryNotFound))?;

    // Count approvals
    let approvals: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'approve'",
    )
    .bind(intent.id)
    .fetch_one(&pool)
    .await?;

    let is_expired = intent.is_expired();

    // Auto-expire if needed
    if is_expired && intent.status == "pending" {
        sqlx::query("UPDATE recovery_intents SET status = 'expired' WHERE id = ?")
            .bind(intent.id)
            .execute(&pool)
            .await?;
    }

    Ok(Json(RecoveryIntentStatus {
        status: if is_expired && intent.status == "pending" {
            "expired".to_string()
        } else {
            intent.status
        },
        approvals_count: approvals.0,
        required_approvals: RecoveryIntent::REQUIRED_APPROVALS,
        expires_at: intent.expires_at,
        is_expired,
    }))
}

/// Get pending recovery requests where the current user is a trusted user (requires auth)
async fn get_pending_recoveries(
    State(pool): State<SqlitePool>,
    auth_user: AuthUser,
) -> Result<Json<Vec<RecoveryIntentWithInfo>>, AppError> {
    // Find recovery intents where:
    // 1. The current user is a trusted user of the account owner
    // 2. The intent is still pending and not expired
    let intents = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
        ),
    >(
        r#"
        SELECT
            ri.id,
            ri.user_id,
            u.username,
            u.display_name,
            ri.token,
            ri.status,
            ri.created_at,
            ri.expires_at
        FROM recovery_intents ri
        JOIN users u ON u.id = ri.user_id
        JOIN trusted_users tu ON tu.user_id = ri.user_id AND tu.trusted_user_id = ?
        WHERE ri.status = 'pending' AND ri.expires_at > datetime('now')
        ORDER BY ri.created_at DESC
        "#,
    )
    .bind(auth_user.user_id)
    .fetch_all(&pool)
    .await?;

    let mut result = Vec::new();
    for (id, user_id, username, display_name, token, status, created_at, expires_at) in intents {
        // Count approvals for this intent
        let approvals: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'approve'",
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;

        // Count rejections for this intent
        let rejections: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'reject'",
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;

        // Check if current user has voted
        let user_vote = sqlx::query_as::<_, (String,)>(
            "SELECT vote FROM recovery_votes WHERE recovery_id = ? AND voter_id = ?",
        )
        .bind(id)
        .bind(auth_user.user_id)
        .fetch_optional(&pool)
        .await?;

        result.push(RecoveryIntentWithInfo {
            id,
            user_id,
            username,
            display_name,
            token,
            status,
            created_at,
            expires_at,
            approvals_count: approvals.0,
            rejections_count: rejections.0,
            required_approvals: RecoveryIntent::REQUIRED_APPROVALS,
            is_blocked: rejections.0 > 0,
            user_has_voted: user_vote.is_some(),
            user_vote: user_vote.map(|(v,)| v),
        });
    }

    Ok(Json(result))
}

/// Vote on a recovery intent (requires auth, must be a trusted user)
async fn vote_on_recovery(
    State(pool): State<SqlitePool>,
    auth_user: AuthUser,
    Path(token): Path<String>,
    Json(req): Json<RecoveryVoteRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.vote != "approve" && req.vote != "reject" {
        return Err(AppError::bad_request(ErrorCode::InvalidVote));
    }

    // Find the recovery intent
    let intent =
        sqlx::query_as::<_, RecoveryIntent>("SELECT * FROM recovery_intents WHERE token = ?")
            .bind(&token)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::not_found(ErrorCode::RecoveryNotFound))?;

    // Check if expired
    if intent.is_expired() {
        sqlx::query(
            "UPDATE recovery_intents SET status = 'expired' WHERE id = ? AND status = 'pending'",
        )
        .bind(intent.id)
        .execute(&pool)
        .await?;
        return Err(AppError::bad_request(ErrorCode::RecoveryExpired));
    }

    // Check if still pending
    if intent.status != "pending" {
        return Err(AppError::bad_request(ErrorCode::RecoveryNotPending));
    }

    // Verify the current user is a trusted user of the account owner
    let is_trusted: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM trusted_users WHERE user_id = ? AND trusted_user_id = ?",
    )
    .bind(intent.user_id)
    .bind(auth_user.user_id)
    .fetch_one(&pool)
    .await?;

    if is_trusted.0 == 0 {
        return Err(AppError::forbidden(ErrorCode::NotTrustedUser));
    }

    // Check if already voted - allow changing vote
    let existing_vote = sqlx::query_as::<_, (i64, String)>(
        "SELECT id, vote FROM recovery_votes WHERE recovery_id = ? AND voter_id = ?",
    )
    .bind(intent.id)
    .bind(auth_user.user_id)
    .fetch_optional(&pool)
    .await?;

    let vote_changed = if let Some((vote_id, old_vote)) = existing_vote {
        if old_vote == req.vote {
            // Same vote, nothing to do
            false
        } else {
            // Update existing vote
            sqlx::query(
                "UPDATE recovery_votes SET vote = ?, voted_at = datetime('now') WHERE id = ?",
            )
            .bind(&req.vote)
            .bind(vote_id)
            .execute(&pool)
            .await?;
            true
        }
    } else {
        // Record new vote
        sqlx::query("INSERT INTO recovery_votes (recovery_id, voter_id, vote) VALUES (?, ?, ?)")
            .bind(intent.id)
            .bind(auth_user.user_id)
            .bind(&req.vote)
            .execute(&pool)
            .await?;
        true
    };

    // Skip status check if vote didn't change
    if !vote_changed {
        let approvals: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'approve'",
        )
        .bind(intent.id)
        .fetch_one(&pool)
        .await?;

        return Ok(Json(serde_json::json!({
            "message": "Vote unchanged",
            "vote": req.vote,
            "approvals_count": approvals.0,
            "required_approvals": RecoveryIntent::REQUIRED_APPROVALS,
            "status": intent.status
        })));
    }

    // Check if we have enough approvals
    let approvals: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'approve'",
    )
    .bind(intent.id)
    .fetch_one(&pool)
    .await?;

    // Check if there are any rejections (blocks approval)
    let rejections: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recovery_votes WHERE recovery_id = ? AND vote = 'reject'",
    )
    .bind(intent.id)
    .fetch_one(&pool)
    .await?;

    let mut new_status = "pending".to_string();
    let is_blocked = rejections.0 > 0;

    // Only approve if we have enough approvals AND no rejections
    if approvals.0 >= RecoveryIntent::REQUIRED_APPROVALS && !is_blocked {
        // Mark as approved
        sqlx::query(
            "UPDATE recovery_intents SET status = 'approved', resolved_at = datetime('now') WHERE id = ?",
        )
        .bind(intent.id)
        .execute(&pool)
        .await?;
        new_status = "approved".to_string();
    }

    Ok(Json(serde_json::json!({
        "message": "Vote recorded",
        "vote": req.vote,
        "approvals_count": approvals.0,
        "rejections_count": rejections.0,
        "required_approvals": RecoveryIntent::REQUIRED_APPROVALS,
        "is_blocked": is_blocked,
        "status": new_status
    })))
}

/// Reset password after recovery is approved (no auth, uses token)
async fn reset_password(
    State(pool): State<SqlitePool>,
    Path(token): Path<String>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate password strength
    if req.new_password.len() < 6 {
        return Err(AppError::bad_request(ErrorCode::PasswordTooWeak));
    }

    // Find the recovery intent
    let intent =
        sqlx::query_as::<_, RecoveryIntent>("SELECT * FROM recovery_intents WHERE token = ?")
            .bind(&token)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::not_found(ErrorCode::RecoveryNotFound))?;

    // Check status
    if intent.status != "approved" {
        return Err(AppError::bad_request(ErrorCode::RecoveryNotApproved));
    }

    // Check if expired (even approved ones can expire if not used quickly)
    if intent.is_expired() {
        return Err(AppError::bad_request(ErrorCode::RecoveryExpired));
    }

    // Hash the new password
    let password_hash = crate::auth::password::hash_password(&req.new_password)?;

    // Update the user's password and increment token_version to invalidate existing sessions
    sqlx::query(
        "UPDATE users SET password_hash = ?, token_version = token_version + 1 WHERE id = ?",
    )
    .bind(&password_hash)
    .bind(intent.user_id)
    .execute(&pool)
    .await?;

    // Mark recovery intent as used
    sqlx::query(
        "UPDATE recovery_intents SET status = 'used', resolved_at = datetime('now') WHERE id = ?",
    )
    .bind(intent.id)
    .execute(&pool)
    .await?;

    // TODO: In Step 3, we'll also set the user's project memberships to RECOVERED state

    Ok(Json(serde_json::json!({
        "message": "Password has been reset successfully. You can now log in with your new password."
    })))
}
