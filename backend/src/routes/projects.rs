use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use sqlx::SqlitePool;

use crate::{
    auth::{AuthUser, ProjectMember, AdminMember},
    error::{AppError, AppResult},
    models::{Project, ProjectWithRole, CreateProject, UpdateProject, JoinProject},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/join", post(join_project))
        .route("/{id}", get(get_project).put(update_project).delete(delete_project))
        .route("/{id}/regenerate-invite", post(regenerate_invite))
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

async fn list_projects(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
) -> AppResult<Json<Vec<ProjectWithRole>>> {
    let projects: Vec<ProjectWithRole> = sqlx::query_as(
        "SELECT p.id, p.name, p.description, p.invite_code, p.created_by, p.created_at, pm.role
         FROM projects p
         JOIN project_members pm ON p.id = pm.project_id
         WHERE pm.user_id = ?
         ORDER BY p.created_at DESC"
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(projects))
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

async fn join_project(
    auth: AuthUser,
    State(pool): State<SqlitePool>,
    Json(input): Json<JoinProject>,
) -> AppResult<Json<Project>> {
    // Find project by invite code
    let project: Option<Project> = sqlx::query_as(
        "SELECT * FROM projects WHERE invite_code = ?"
    )
    .bind(&input.invite_code)
    .fetch_optional(&pool)
    .await?;

    let project = project.ok_or_else(|| AppError::NotFound("Invalid invite code".to_string()))?;

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

    // Add as editor member (without participant association initially)
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role) VALUES (?, ?, 'editor')"
    )
    .bind(project.id)
    .bind(auth.user_id)
    .execute(&pool)
    .await?;

    Ok(Json(project))
}
