use crate::{
    error::{AppError, AppResult},
    models::{ApprovalVote, ApprovalWithDetails, ProjectApproval, UserState, VoteWithVoter},
};
use sqlx::SqlitePool;

/// Calculate the number of required approvals based on project member count and voter role
///
/// Rules:
/// - Project admin (role='admin'): 1 vote (instant approval)
/// - Single-user project: i64::MAX (can't self-approve, sysadmin only)
/// - Multiple users: ceil(33% * member_count)
fn calculate_required_votes(member_count: i64, voter_is_admin: bool) -> i64 {
    if voter_is_admin {
        return 1; // Admin vote is instant approval
    }

    if member_count == 1 {
        return i64::MAX; // Single user can't self-approve, only sysadmin can
    }

    // 33% quorum, round up
    ((member_count as f64 * 0.33).ceil()) as i64
}

/// Create approval records in all projects where the user is a member
/// Sets all project_members.status to 'pending' and user.state to 'pending_approval'
pub async fn create_approval_for_all_projects(
    pool: &SqlitePool,
    user_id: i64,
    event_type: &str,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    // Get all projects where user is a member
    let project_ids: Vec<i64> =
        sqlx::query_scalar("SELECT project_id FROM project_members WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&mut *tx)
            .await?;

    // Create one approval record per project
    for project_id in project_ids {
        sqlx::query(
            "INSERT INTO project_approvals (user_id, project_id, event_type)
             VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(event_type)
        .execute(&mut *tx)
        .await?;
    }

    // Set all project memberships to pending
    sqlx::query("UPDATE project_members SET status = 'pending' WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // Set user state to pending_approval
    sqlx::query("UPDATE users SET user_state = ? WHERE id = ?")
        .bind(UserState::PendingApproval.as_str())
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

/// Get all pending approvals for a specific user across all projects
pub async fn get_pending_approvals_for_user(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<ApprovalWithDetails>> {
    #[derive(sqlx::FromRow)]
    struct ApprovalRow {
        id: i64,
        user_id: i64,
        project_id: i64,
        event_type: String,
        event_metadata: Option<String>,
        status: String,
        created_at: String,
        resolved_at: Option<String>,
        project_name: String,
        username: String,
        display_name: Option<String>,
    }

    let approvals: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT pa.*, p.name as project_name, u.username, u.display_name
         FROM project_approvals pa
         JOIN projects p ON pa.project_id = p.id
         JOIN users u ON pa.user_id = u.id
         WHERE pa.user_id = ? AND pa.status = 'pending'
         ORDER BY pa.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();

    for approval_row in approvals {
        // Get votes for this approval
        let votes = get_votes_for_approval(pool, approval_row.id).await?;
        let vote_count = votes.iter().filter(|v| v.vote.vote == "approve").count() as i64;

        // Get member count to calculate required votes
        let member_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND status = 'active'",
        )
        .bind(approval_row.project_id)
        .fetch_one(pool)
        .await?;

        // For pending approvals, we can't determine admin status of future voters
        // so we show the regular quorum
        let required_votes = calculate_required_votes(member_count, false);

        result.push(ApprovalWithDetails {
            approval: ProjectApproval {
                id: approval_row.id,
                user_id: approval_row.user_id,
                project_id: approval_row.project_id,
                event_type: approval_row.event_type,
                event_metadata: approval_row.event_metadata,
                status: approval_row.status,
                created_at: approval_row.created_at,
                resolved_at: approval_row.resolved_at,
            },
            project_name: approval_row.project_name,
            username: approval_row.username,
            display_name: approval_row.display_name,
            votes,
            vote_count,
            required_votes,
            can_self_approve: member_count > 1,
        });
    }

    Ok(result)
}

/// Get approvals that the current user can vote on
/// Excludes approvals they've already voted on and approvals for themselves
pub async fn get_actionable_approvals(
    pool: &SqlitePool,
    voter_id: i64,
) -> AppResult<Vec<ApprovalWithDetails>> {
    #[derive(sqlx::FromRow)]
    struct ApprovalRow {
        id: i64,
        user_id: i64,
        project_id: i64,
        event_type: String,
        event_metadata: Option<String>,
        status: String,
        created_at: String,
        resolved_at: Option<String>,
        project_name: String,
        username: String,
        display_name: Option<String>,
    }

    // Find approvals in projects where voter is a member (and status='active')
    // Exclude approvals they've already voted on
    // Exclude approvals for themselves
    let approvals: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT pa.*, p.name as project_name, u.username, u.display_name
         FROM project_approvals pa
         JOIN projects p ON pa.project_id = p.id
         JOIN users u ON pa.user_id = u.id
         JOIN project_members pm ON pm.project_id = pa.project_id AND pm.user_id = ?
         WHERE pa.status = 'pending'
           AND pa.user_id != ?
           AND pm.status = 'active'
           AND NOT EXISTS (
               SELECT 1 FROM approval_votes av
               WHERE av.approval_id = pa.id AND av.voter_id = ?
           )
         ORDER BY pa.created_at DESC",
    )
    .bind(voter_id)
    .bind(voter_id)
    .bind(voter_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();

    for approval_row in approvals {
        // Get existing votes
        let votes = get_votes_for_approval(pool, approval_row.id).await?;
        let vote_count = votes.iter().filter(|v| v.vote.vote == "approve").count() as i64;

        // Get member count
        let member_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND status = 'active'",
        )
        .bind(approval_row.project_id)
        .fetch_one(pool)
        .await?;

        // Check if voter is admin
        let voter_role: String = sqlx::query_scalar(
            "SELECT role FROM project_members WHERE project_id = ? AND user_id = ?",
        )
        .bind(approval_row.project_id)
        .bind(voter_id)
        .fetch_one(pool)
        .await?;

        let voter_is_admin = voter_role == "admin";
        let required_votes = calculate_required_votes(member_count, voter_is_admin);

        result.push(ApprovalWithDetails {
            approval: ProjectApproval {
                id: approval_row.id,
                user_id: approval_row.user_id,
                project_id: approval_row.project_id,
                event_type: approval_row.event_type,
                event_metadata: approval_row.event_metadata,
                status: approval_row.status,
                created_at: approval_row.created_at,
                resolved_at: approval_row.resolved_at,
            },
            project_name: approval_row.project_name,
            username: approval_row.username,
            display_name: approval_row.display_name,
            votes,
            vote_count,
            required_votes,
            can_self_approve: member_count > 1,
        });
    }

    Ok(result)
}

/// Get votes for a specific approval with voter details
async fn get_votes_for_approval(
    pool: &SqlitePool,
    approval_id: i64,
) -> AppResult<Vec<VoteWithVoter>> {
    #[derive(sqlx::FromRow)]
    struct VoteRow {
        id: i64,
        approval_id: i64,
        voter_id: i64,
        vote: String,
        reason: Option<String>,
        voted_at: String,
        voter_username: String,
        voter_display_name: Option<String>,
    }

    let votes: Vec<VoteRow> = sqlx::query_as(
        "SELECT av.*, u.username as voter_username, u.display_name as voter_display_name
         FROM approval_votes av
         JOIN users u ON av.voter_id = u.id
         WHERE av.approval_id = ?
         ORDER BY av.voted_at ASC",
    )
    .bind(approval_id)
    .fetch_all(pool)
    .await?;

    Ok(votes
        .into_iter()
        .map(|v| VoteWithVoter {
            vote: ApprovalVote {
                id: v.id,
                approval_id: v.approval_id,
                voter_id: v.voter_id,
                vote: v.vote,
                reason: v.reason,
                voted_at: v.voted_at,
            },
            voter_username: v.voter_username,
            voter_display_name: v.voter_display_name,
        })
        .collect())
}

/// Cast a vote on an approval
/// Returns the updated approval with details
pub async fn cast_vote(
    pool: &SqlitePool,
    approval_id: i64,
    voter_id: i64,
    vote: &str,
    reason: Option<String>,
) -> AppResult<ApprovalWithDetails> {
    let mut tx = pool.begin().await?;

    // Validate vote value
    if vote != "approve" && vote != "reject" {
        return Err(AppError::BadRequest(
            "Vote must be 'approve' or 'reject'".to_string(),
        ));
    }

    // Get approval details
    #[derive(sqlx::FromRow)]
    struct ApprovalInfo {
        user_id: i64,
        project_id: i64,
        status: String,
    }

    let approval: Option<ApprovalInfo> =
        sqlx::query_as("SELECT user_id, project_id, status FROM project_approvals WHERE id = ?")
            .bind(approval_id)
            .fetch_optional(&mut *tx)
            .await?;

    let approval = approval.ok_or_else(|| AppError::NotFound("Approval not found".to_string()))?;

    // Check approval is still pending
    if approval.status != "pending" {
        return Err(AppError::BadRequest(format!(
            "Approval is already {}",
            approval.status
        )));
    }

    // Check voter is a member of the project
    let voter_membership: Option<String> = sqlx::query_scalar(
        "SELECT status FROM project_members WHERE project_id = ? AND user_id = ?",
    )
    .bind(approval.project_id)
    .bind(voter_id)
    .fetch_optional(&mut *tx)
    .await?;

    if voter_membership.is_none() {
        return Err(AppError::Forbidden(
            "You are not a member of this project".to_string(),
        ));
    }

    if voter_membership.as_deref() != Some("active") {
        return Err(AppError::Forbidden(
            "Your membership in this project is not active".to_string(),
        ));
    }

    // Check voter is not voting on their own approval
    if approval.user_id == voter_id {
        return Err(AppError::BadRequest(
            "Cannot vote on your own approval".to_string(),
        ));
    }

    // Insert the vote (will fail if already voted due to UNIQUE constraint)
    sqlx::query(
        "INSERT INTO approval_votes (approval_id, voter_id, vote, reason)
         VALUES (?, ?, ?, ?)",
    )
    .bind(approval_id)
    .bind(voter_id)
    .bind(vote)
    .bind(&reason)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            AppError::BadRequest("You have already voted on this approval".to_string())
        } else {
            AppError::from(e)
        }
    })?;

    tx.commit().await?;

    // Check if approval should be resolved
    check_and_resolve_approval(pool, approval_id).await?;

    // Return updated approval with details
    get_approval_with_details(pool, approval_id).await
}

/// Check if an approval has met the threshold and resolve it accordingly
async fn check_and_resolve_approval(pool: &SqlitePool, approval_id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    // Get approval info
    #[derive(sqlx::FromRow)]
    struct ApprovalInfo {
        user_id: i64,
        project_id: i64,
        status: String,
    }

    let approval: ApprovalInfo =
        sqlx::query_as("SELECT user_id, project_id, status FROM project_approvals WHERE id = ?")
            .bind(approval_id)
            .fetch_one(&mut *tx)
            .await?;

    if approval.status != "pending" {
        return Ok(()); // Already resolved
    }

    // Check for any rejections
    let rejection_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM approval_votes WHERE approval_id = ? AND vote = 'reject'",
    )
    .bind(approval_id)
    .fetch_one(&mut *tx)
    .await?;

    if rejection_count > 0 {
        // Permanent rejection
        sqlx::query(
            "UPDATE project_approvals SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?"
        )
        .bind(approval_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        return Ok(());
    }

    // Count approve votes
    let approve_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM approval_votes WHERE approval_id = ? AND vote = 'approve'",
    )
    .bind(approval_id)
    .fetch_one(&mut *tx)
    .await?;

    // Get member count (active members only)
    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND status = 'active'",
    )
    .bind(approval.project_id)
    .fetch_one(&mut *tx)
    .await?;

    // Check if any voter is admin
    let admin_voted: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM approval_votes av
            JOIN project_members pm ON av.voter_id = pm.user_id AND pm.project_id = ?
            WHERE av.approval_id = ? AND pm.role = 'admin' AND av.vote = 'approve'
        )",
    )
    .bind(approval.project_id)
    .bind(approval_id)
    .fetch_one(&mut *tx)
    .await?;

    let required_votes = calculate_required_votes(member_count, admin_voted);

    if approve_count >= required_votes {
        // Approval threshold met
        sqlx::query(
            "UPDATE project_approvals SET status = 'approved', resolved_at = datetime('now') WHERE id = ?"
        )
        .bind(approval_id)
        .execute(&mut *tx)
        .await?;

        // Activate the project membership
        sqlx::query(
            "UPDATE project_members SET status = 'active' WHERE project_id = ? AND user_id = ?",
        )
        .bind(approval.project_id)
        .bind(approval.user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
    } else {
        tx.commit().await?;
    }

    Ok(())
}

/// Get a single approval with full details
pub async fn get_approval_with_details(
    pool: &SqlitePool,
    approval_id: i64,
) -> AppResult<ApprovalWithDetails> {
    #[derive(sqlx::FromRow)]
    struct ApprovalRow {
        id: i64,
        user_id: i64,
        project_id: i64,
        event_type: String,
        event_metadata: Option<String>,
        status: String,
        created_at: String,
        resolved_at: Option<String>,
        project_name: String,
        username: String,
        display_name: Option<String>,
    }

    let approval_row: ApprovalRow = sqlx::query_as(
        "SELECT pa.*, p.name as project_name, u.username, u.display_name
         FROM project_approvals pa
         JOIN projects p ON pa.project_id = p.id
         JOIN users u ON pa.user_id = u.id
         WHERE pa.id = ?",
    )
    .bind(approval_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::NotFound("Approval not found".to_string()))?;

    let votes = get_votes_for_approval(pool, approval_row.id).await?;
    let vote_count = votes.iter().filter(|v| v.vote.vote == "approve").count() as i64;

    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND status = 'active'",
    )
    .bind(approval_row.project_id)
    .fetch_one(pool)
    .await?;

    // Check if any voter is admin
    let admin_voted: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM approval_votes av
            JOIN project_members pm ON av.voter_id = pm.user_id AND pm.project_id = ?
            WHERE av.approval_id = ? AND pm.role = 'admin' AND av.vote = 'approve'
        )",
    )
    .bind(approval_row.project_id)
    .bind(approval_row.id)
    .fetch_one(pool)
    .await?;

    let required_votes = calculate_required_votes(member_count, admin_voted);

    Ok(ApprovalWithDetails {
        approval: ProjectApproval {
            id: approval_row.id,
            user_id: approval_row.user_id,
            project_id: approval_row.project_id,
            event_type: approval_row.event_type,
            event_metadata: approval_row.event_metadata,
            status: approval_row.status,
            created_at: approval_row.created_at,
            resolved_at: approval_row.resolved_at,
        },
        project_name: approval_row.project_name,
        username: approval_row.username,
        display_name: approval_row.display_name,
        votes,
        vote_count,
        required_votes,
        can_self_approve: member_count > 1,
    })
}
