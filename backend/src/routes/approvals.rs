use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;

use crate::{
    auth::AuthUser,
    error::AppResult,
    models::{ApprovalWithDetails, CastVote, PendingMemberWithProject},
    services::approval_service,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/my-pending", get(get_my_pending_approvals))
        .route("/actionable", get(get_actionable_approvals))
        .route("/pending-members", get(get_pending_members_to_approve))
        .route("/{id}", get(get_approval))
        .route("/{id}/vote", post(cast_vote))
}

/// Get all pending approvals for the current user
/// This endpoint is accessible even for users in PendingApproval state
async fn get_my_pending_approvals(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ApprovalWithDetails>>> {
    let approvals = approval_service::get_pending_approvals_for_user(&pool, auth.user_id).await?;
    Ok(Json(approvals))
}

/// Get all approvals that the current user can vote on
/// Excludes approvals they've already voted on and their own approvals
async fn get_actionable_approvals(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ApprovalWithDetails>>> {
    let approvals = approval_service::get_actionable_approvals(&pool, auth.user_id).await?;
    Ok(Json(approvals))
}

/// Get all pending members across all projects where the current user is an admin
/// This allows admins to see all join requests in one place
async fn get_pending_members_to_approve(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<PendingMemberWithProject>>> {
    // Get pending members from all projects where the current user is an admin
    let pending_members: Vec<PendingMemberWithProject> = sqlx::query_as(
        r#"
        SELECT
            pm.id,
            pm.project_id,
            p.name as project_name,
            pm.user_id,
            u.username,
            u.display_name,
            pm.role,
            pm.joined_at,
            pm.status
        FROM project_members pm
        JOIN users u ON pm.user_id = u.id
        JOIN projects p ON pm.project_id = p.id
        WHERE pm.status = 'pending'
        AND pm.project_id IN (
            SELECT project_id
            FROM project_members
            WHERE user_id = ? AND role = 'admin' AND status = 'active'
        )
        ORDER BY pm.joined_at DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(pending_members))
}

/// Get details of a specific approval
async fn get_approval(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> AppResult<Json<ApprovalWithDetails>> {
    let approval = approval_service::get_approval_with_details(&pool, id).await?;

    // Verify user has access to this approval
    // Either they are the subject of the approval OR they are a member of the project
    let has_access = approval.approval.user_id == auth.user_id || {
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM project_members WHERE project_id = ? AND user_id = ?)",
        )
        .bind(approval.approval.project_id)
        .bind(auth.user_id)
        .fetch_one(&pool)
        .await?;
        is_member
    };

    if !has_access {
        return Err(crate::error::AppError::Forbidden(
            "You do not have access to this approval".to_string(),
        ));
    }

    Ok(Json(approval))
}

/// Cast a vote on an approval
async fn cast_vote(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(input): Json<CastVote>,
) -> AppResult<Json<ApprovalWithDetails>> {
    let approval =
        approval_service::cast_vote(&pool, id, auth.user_id, &input.vote, input.reason).await?;

    Ok(Json(approval))
}
