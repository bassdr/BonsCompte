use axum::{
    extract::State,
    routing::{get, post, put},
    Json, Router,
};
use rand::Rng;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    auth::{AuthUser, ProjectMember, AdminMember},
    error::{AppError, AppResult},
    models::{Project, ProjectListItem, CreateProject, UpdateProject, JoinProject, UpdateProjectSettings, EntityType},
    services::{HistoryService, debt_calculator},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/join", post(join_project))
        .route("/{id}", get(get_project).put(update_project).delete(delete_project))
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
    role: String,
    owner_name: String,
    user_participant_id: Option<i64>,
}

async fn list_projects(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ProjectListItem>>> {
    use crate::models::PoolSummary;

    // Get projects with owner name and user's participant ID
    let rows: Vec<ProjectListRow> = sqlx::query_as(
        "SELECT p.id, p.name, p.description, p.invite_code, p.created_by, p.created_at,
                p.invites_enabled, p.require_approval, pm.role,
                COALESCE(u.display_name, u.username) as owner_name,
                pm.participant_id as user_participant_id
         FROM projects p
         JOIN project_members pm ON p.id = pm.project_id
         JOIN users u ON p.created_by = u.id
         WHERE pm.user_id = ? AND pm.status = 'active'
         ORDER BY p.created_at DESC"
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());

    for row in rows {
        let mut user_balance: Option<f64> = None;
        let mut user_pools: Vec<PoolSummary> = Vec::new();

        // If user has a participant in this project, get their debt summary
        if let Some(participant_id) = row.user_participant_id {
            // Use the debt calculator to get accurate balances (including recurring payments)
            if let Ok(debt_summary) = debt_calculator::calculate_debts(&pool, row.id).await {
                // Find user's balance
                if let Some(balance) = debt_summary.balances.iter().find(|b| b.participant_id == participant_id) {
                    user_balance = Some(balance.net_balance);
                }

                // Get user's pool ownerships
                for pool_ownership in &debt_summary.pool_ownerships {
                    if let Some(entry) = pool_ownership.entries.iter().find(|e| e.participant_id == participant_id) {
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

        items.push(ProjectListItem {
            id: row.id,
            name: row.name,
            description: row.description,
            invite_code: row.invite_code,
            created_by: row.created_by,
            created_at: row.created_at,
            invites_enabled: row.invites_enabled,
            require_approval: row.require_approval,
            role: row.role,
            owner_name: row.owner_name,
            user_balance,
            user_pools,
        });
    }

    Ok(Json(items))
}

async fn create_project(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(input): Json<CreateProject>,
) -> AppResult<Json<Project>> {
    let invite_code = generate_invite_code();

    // Start transaction
    let mut tx = pool.begin().await?;

    // Create project
    let result = sqlx::query(
        "INSERT INTO projects (name, description, invite_code, created_by) VALUES (?, ?, ?, ?)"
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
        return Err(AppError::BadRequest("No fields to update".to_string()));
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
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::Project,
        member.project_id,
        &before,
        &project,
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

    // Build dynamic update
    let mut updates = Vec::new();
    let mut bool_binds: Vec<bool> = Vec::new();

    if let Some(invites_enabled) = input.invites_enabled {
        updates.push("invites_enabled = ?");
        bool_binds.push(invites_enabled);
    }
    if let Some(require_approval) = input.require_approval {
        updates.push("require_approval = ?");
        bool_binds.push(require_approval);
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest("No settings to update".to_string()));
    }

    let sql = format!("UPDATE projects SET {} WHERE id = ?", updates.join(", "));
    let mut query = sqlx::query(&sql);
    for bind in &bool_binds {
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
        &correlation_id,
        member.user_id,
        member.project_id,
        EntityType::Project,
        member.project_id,
        &before,
        &project,
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
    let project: Option<Project> = sqlx::query_as(
        "SELECT * FROM projects WHERE invite_code = ?"
    )
    .bind(&input.invite_code)
    .fetch_optional(&pool)
    .await?;

    let project = project.ok_or_else(|| AppError::NotFound("Invalid invite code".to_string()))?;

    // Check if invites are enabled
    if !project.invites_enabled {
        return Err(AppError::Forbidden("Invites are disabled for this project".to_string()));
    }

    // Check if already a member
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM project_members WHERE project_id = ? AND user_id = ?"
    )
    .bind(project.id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Already a member of this project".to_string()));
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
             WHERE invite_token = ? AND project_id = ?"
        )
        .bind(token)
        .bind(project.id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(invite) = invite {
            if invite.used_by.is_some() {
                return Err(AppError::BadRequest("This invite link has already been used".to_string()));
            }

            // Check if participant already has a user linked
            let existing_user: Option<i64> = sqlx::query_scalar(
                "SELECT user_id FROM participants WHERE id = ? AND user_id IS NOT NULL"
            )
            .bind(invite.participant_id)
            .fetch_optional(&mut *tx)
            .await?;

            if existing_user.is_some() {
                return Err(AppError::BadRequest("This participant is already linked to a user".to_string()));
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

    // Determine status based on require_approval
    let status = if project.require_approval { "pending" } else { "active" };

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
