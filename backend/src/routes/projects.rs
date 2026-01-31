use axum::{
    extract::State,
    routing::{get, post, put},
    Json, Router,
};
use rand::Rng;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    auth::{AdminMember, AuthUser, ProjectMember},
    error::{AppError, AppResult, ErrorCode},
    models::{
        CreateProject, EntityType, JoinProject, Project, ProjectListItem, UpdateProject,
        UpdateProjectSettings,
    },
    services::{debt_calculator, HistoryService},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/join", post(join_project))
        .route(
            "/{id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/{id}/regenerate-invite", post(regenerate_invite))
        .route("/{id}/settings", put(update_project_settings))
}

fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Row from the project list query with owner and participant info
#[derive(sqlx::FromRow)]
struct ProjectListRow {
    id: i64,
    name: String,
    description: Option<String>,
    invite_code: Option<String>,
    created_by: i64,
    created_at: String,
    invites_enabled: bool,
    require_approval: bool,
    pool_warning_horizon: String,
    pending_member_access: String,
    role: String,
    owner_name: String,
    user_participant_id: Option<i64>,
    member_status: String,
}

async fn list_projects(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ProjectListItem>>> {
    use crate::models::PoolSummary;

    // Get projects with owner name and user's participant ID
    // Include 'active', 'recovered', and 'pending' members
    let rows: Vec<ProjectListRow> = sqlx::query_as(
        "SELECT p.id, p.name, p.description, p.invite_code, p.created_by, p.created_at,
                p.invites_enabled, p.require_approval, p.pool_warning_horizon,
                COALESCE(p.pending_member_access, 'read_only') as pending_member_access,
                pm.role,
                COALESCE(u.display_name, u.username) as owner_name,
                pm.participant_id as user_participant_id,
                pm.status as member_status
         FROM projects p
         JOIN project_members pm ON p.id = pm.project_id
         JOIN users u ON p.created_by = u.id
         WHERE pm.user_id = ? AND pm.status IN ('active', 'recovered', 'pending')
         ORDER BY p.created_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());

    for row in rows {
        let mut user_balance: Option<f64> = None;
        let mut user_pools: Vec<PoolSummary> = Vec::new();

        // Only calculate debt summary if user has access (not pending with 'none' access)
        let has_data_access = row.member_status == "active"
            || row.member_status == "recovered"
            || (row.member_status == "pending" && row.pending_member_access != "none");

        // If user has a participant in this project and has data access, get their debt summary
        if let Some(participant_id) = row.user_participant_id {
            if has_data_access {
                // Use the debt calculator to get accurate balances (including recurring payments)
                // Exclude drafts from project list summary
                if let Ok(debt_summary) =
                    debt_calculator::calculate_debts(&pool, row.id, false).await
                {
                    // Find user's balance
                    if let Some(balance) = debt_summary
                        .balances
                        .iter()
                        .find(|b| b.participant_id == participant_id)
                    {
                        user_balance = Some(balance.net_balance);
                    }

                    // Get user's pool ownerships
                    for pool_ownership in &debt_summary.pool_ownerships {
                        if let Some(entry) = pool_ownership
                            .entries
                            .iter()
                            .find(|e| e.participant_id == participant_id)
                        {
                            if entry.ownership.abs() >= 0.01 {
                                user_pools.push(PoolSummary {
                                    pool_name: pool_ownership.pool_name.clone(),
                                    ownership: entry.ownership,
                                });
                            }
                        }
                    }
                }
            }
        }

        items.push(ProjectListItem {
            id: row.id,
            name: row.name,
            description: row.description,
            invite_code: row.invite_code,
            created_by: row.created_by,
            created_at: row.created_at,
            invites_enabled: row.invites_enabled,
            require_approval: row.require_approval,
            pool_warning_horizon: row.pool_warning_horizon,
            pending_member_access: row.pending_member_access,
            role: row.role,
            owner_name: row.owner_name,
            user_balance,
            user_pools,
            member_status: row.member_status,
        });
    }

    Ok(Json(items))
}

async fn create_project(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    State(state): State<AppState>,
    Json(input): Json<CreateProject>,
) -> AppResult<Json<Project>> {
    // Check project creation limit if configured
    if let Some(max_projects) = state.config.max_projects_per_user {
        let project_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE created_by = ?")
                .bind(auth.user_id)
                .fetch_one(&pool)
                .await?;

        if project_count >= max_projects {
            return Err(AppError::forbidden(ErrorCode::ProjectLimitReached));
        }
    }

    let invite_code = generate_invite_code();

    // Start transaction
    let mut tx = pool.begin().await?;

    // Create project
    let result = sqlx::query(
        "INSERT INTO projects (name, description, invite_code, created_by) VALUES (?, ?, ?, ?)",
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(&invite_code)
    .bind(auth.user_id)
    .execute(&mut *tx)
    .await?;

    let project_id = result.last_insert_rowid();

    // Create participant for the creator
    let participant_result = sqlx::query(
        "INSERT INTO participants (project_id, name, user_id, default_weight) VALUES (?, ?, ?, 1.0)"
    )
    .bind(project_id)
    .bind(&auth.username)
    .bind(auth.user_id)
    .execute(&mut *tx)
    .await?;

    let participant_id = participant_result.last_insert_rowid();

    // Add creator as admin member linked to participant
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role, participant_id) VALUES (?, ?, 'admin', ?)"
    )
    .bind(project_id)
    .bind(auth.user_id)
    .bind(participant_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Fetch created project
    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(project_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(project))
}

async fn get_project(
    member: ProjectMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Project>> {
    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(project))
}

async fn update_project(
    admin: AdminMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<UpdateProject>,
) -> AppResult<Json<Project>> {
    let member = admin.0;

    // Capture before state for history
    let before: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    // Build dynamic update
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        updates.push("name = ?");
        binds.push(name.clone());
    }
    if let Some(desc) = &input.description {
        updates.push("description = ?");
        binds.push(desc.clone());
    }

    if updates.is_empty() {
        return Err(AppError::bad_request(ErrorCode::NoFieldsToUpdate));
    }

    let sql = format!("UPDATE projects SET {} WHERE id = ?", updates.join(", "));
    let mut query = sqlx::query(&sql);
    for bind in &binds {
        query = query.bind(bind);
    }
    query = query.bind(member.project_id);
    query.execute(&pool).await?;

    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    // Log the update to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_update(
        &pool,
        crate::services::history::LogUpdateParams {
            correlation_id: &correlation_id,
            actor_user_id: member.user_id,
            project_id: member.project_id,
            entity_type: EntityType::Project,
            entity_id: member.project_id,
            before: &before,
            after: &project,
        },
    )
    .await;

    Ok(Json(project))
}

async fn delete_project(
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<serde_json::Value>> {
    let member = admin.0;

    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(member.project_id)
        .execute(&pool)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn regenerate_invite(
    admin: AdminMember,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Project>> {
    let member = admin.0;
    let new_code = generate_invite_code();

    sqlx::query("UPDATE projects SET invite_code = ? WHERE id = ?")
        .bind(&new_code)
        .bind(member.project_id)
        .execute(&pool)
        .await?;

    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    Ok(Json(project))
}

async fn update_project_settings(
    admin: AdminMember,
    State(pool): State<SqlitePool>,
    Json(input): Json<UpdateProjectSettings>,
) -> AppResult<Json<Project>> {
    let member = admin.0;

    // Capture before state for history
    let before: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    // Validate pending_member_access if provided
    if let Some(ref pending_access) = input.pending_member_access {
        if !matches!(
            pending_access.as_str(),
            "none" | "read_only" | "auto_approve"
        ) {
            return Err(AppError::bad_request(
                ErrorCode::InvalidPendingAccessSetting,
            ));
        }
    }

    // Build dynamic update - we need to track bool and string binds separately
    // since they go into different bind positions
    let mut updates = Vec::new();
    let mut bool_binds: Vec<bool> = Vec::new();
    let mut string_binds: Vec<String> = Vec::new();

    if let Some(invites_enabled) = input.invites_enabled {
        updates.push(("invites_enabled = ?", "bool"));
        bool_binds.push(invites_enabled);
    }
    if let Some(require_approval) = input.require_approval {
        updates.push(("require_approval = ?", "bool"));
        bool_binds.push(require_approval);
    }
    if let Some(pending_member_access) = input.pending_member_access {
        updates.push(("pending_member_access = ?", "string"));
        string_binds.push(pending_member_access);
    }

    if updates.is_empty() {
        return Err(AppError::bad_request(ErrorCode::NoFieldsToUpdate));
    }

    let update_clauses: Vec<&str> = updates.iter().map(|(clause, _)| *clause).collect();
    let sql = format!(
        "UPDATE projects SET {} WHERE id = ?",
        update_clauses.join(", ")
    );

    // Build the query with binds in correct order
    let mut query = sqlx::query(&sql);
    let mut bool_idx = 0;
    let mut string_idx = 0;
    for (_, bind_type) in &updates {
        match *bind_type {
            "bool" => {
                query = query.bind(bool_binds[bool_idx]);
                bool_idx += 1;
            }
            "string" => {
                query = query.bind(&string_binds[string_idx]);
                string_idx += 1;
            }
            _ => {}
        }
    }
    query = query.bind(member.project_id);
    query.execute(&pool).await?;

    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(member.project_id)
        .fetch_one(&pool)
        .await?;

    // Log the update to history
    let correlation_id = HistoryService::new_correlation_id();
    let _ = HistoryService::log_update(
        &pool,
        crate::services::history::LogUpdateParams {
            correlation_id: &correlation_id,
            actor_user_id: member.user_id,
            project_id: member.project_id,
            entity_type: EntityType::Project,
            entity_id: member.project_id,
            before: &before,
            after: &project,
        },
    )
    .await;

    Ok(Json(project))
}

#[derive(Serialize)]
struct JoinProjectResponse {
    project: Project,
    status: String,
    participant_id: Option<i64>,
}

async fn join_project(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(input): Json<JoinProject>,
) -> AppResult<Json<JoinProjectResponse>> {
    // Find project by invite code
    let project: Option<Project> = sqlx::query_as("SELECT * FROM projects WHERE invite_code = ?")
        .bind(&input.invite_code)
        .fetch_optional(&pool)
        .await?;

    let project = project.ok_or_else(|| AppError::not_found(ErrorCode::InviteNotFound))?;

    // Check if invites are enabled
    if !project.invites_enabled {
        return Err(AppError::Forbidden(
            "Invites are disabled for this project".to_string(),
        ));
    }

    // Check if already a member
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM project_members WHERE project_id = ? AND user_id = ?")
            .bind(project.id)
            .bind(auth.user_id)
            .fetch_optional(&pool)
            .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "Already a member of this project".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Handle participant token if provided
    let mut participant_id: Option<i64> = None;
    if let Some(token) = &input.participant_token {
        // Look up the participant invite
        #[derive(sqlx::FromRow)]
        struct ParticipantInvite {
            id: i64,
            participant_id: i64,
            used_by: Option<i64>,
        }

        let invite: Option<ParticipantInvite> = sqlx::query_as(
            "SELECT id, participant_id, used_by FROM participant_invites
             WHERE invite_token = ? AND project_id = ?",
        )
        .bind(token)
        .bind(project.id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(invite) = invite {
            if invite.used_by.is_some() {
                return Err(AppError::BadRequest(
                    "This invite link has already been used".to_string(),
                ));
            }

            // Check if participant already has a user linked
            let existing_user: Option<i64> = sqlx::query_scalar(
                "SELECT user_id FROM participants WHERE id = ? AND user_id IS NOT NULL",
            )
            .bind(invite.participant_id)
            .fetch_optional(&mut *tx)
            .await?;

            if existing_user.is_some() {
                return Err(AppError::BadRequest(
                    "This participant is already linked to a user".to_string(),
                ));
            }

            // Link user to participant
            sqlx::query("UPDATE participants SET user_id = ? WHERE id = ?")
                .bind(auth.user_id)
                .bind(invite.participant_id)
                .execute(&mut *tx)
                .await?;

            // Mark invite as used
            sqlx::query("UPDATE participant_invites SET used_by = ?, used_at = datetime('now') WHERE id = ?")
                .bind(auth.user_id)
                .bind(invite.id)
                .execute(&mut *tx)
                .await?;

            participant_id = Some(invite.participant_id);
        }
        // If token doesn't match, silently ignore (allow joining without participant link)
    }

    // Determine status based on require_approval and pending_member_access
    let status = if !project.require_approval || project.pending_member_access == "auto_approve" {
        "active"
    } else {
        "pending"
    };

    // Add as editor member
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role, participant_id, status) VALUES (?, ?, 'editor', ?, ?)"
    )
    .bind(project.id)
    .bind(auth.user_id)
    .bind(participant_id)
    .bind(status)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(JoinProjectResponse {
        project,
        status: status.to_string(),
        participant_id,
    }))
}
