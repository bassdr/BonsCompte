use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
    RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::models::{Role, UserState};

use super::jwt::validate_token;

/// Extractor for authenticated users
/// Validates JWT token and checks user_state and token_version against the database
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized)?;

        // Get JWT secret from extensions (set by middleware layer)
        let jwt_secret = parts
            .extensions
            .get::<JwtSecret>()
            .ok_or(AppError::Internal("JWT secret not configured".to_string()))?;

        // Validate token signature and expiration
        let claims = validate_token(bearer.token(), &jwt_secret.0)?;

        // Get database pool from extensions to validate user state and token version
        let pool = parts
            .extensions
            .get::<SqlitePool>()
            .ok_or(AppError::Internal(
                "Database pool not configured".to_string(),
            ))?;

        // Fetch current user state and token version from database
        let user_data: Option<(String, i64)> =
            sqlx::query_as("SELECT user_state, token_version FROM users WHERE id = ?")
                .bind(claims.sub)
                .fetch_optional(pool)
                .await
                .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

        let (user_state_str, db_token_version) = user_data.ok_or(AppError::Unauthorized)?;

        // Check if token version matches (reject if password was reset or token invalidated)
        if claims.token_version != db_token_version {
            return Err(AppError::TokenInvalidated);
        }

        // Check user state
        let user_state = user_state_str.parse().unwrap_or_default();

        match user_state {
            UserState::Active => {
                // User is active, proceed
            }
            UserState::PendingApproval => {
                return Err(AppError::AccountPendingApproval);
            }
            UserState::Revoked => {
                return Err(AppError::AccountRevoked);
            }
        }

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}

/// JWT secret wrapper for request extensions
#[derive(Clone)]
pub struct JwtSecret(pub String);

/// Extractor for project members - verifies user has access to the project
#[derive(Debug, Clone)]
pub struct ProjectMember {
    pub user_id: i64,
    pub username: String,
    pub project_id: i64,
    pub role: Role,
    pub participant_id: Option<i64>,
}

impl ProjectMember {
    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }

    pub fn is_editor_or_above(&self) -> bool {
        self.role >= Role::Editor
    }

    pub fn can_edit(&self) -> bool {
        self.is_editor_or_above()
    }
}

impl<S> FromRequestParts<S> for ProjectMember
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // First, extract the authenticated user
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        // Extract project_id from path - use explicit struct to match the {id} parameter name
        #[derive(Deserialize)]
        struct ProjectPath {
            id: i64,
        }

        let Path(project_path): Path<ProjectPath> = parts
            .extract::<Path<ProjectPath>>()
            .await
            .map_err(|_| AppError::BadRequest("Invalid project ID".to_string()))?;

        let project_id = project_path.id;

        // Get database pool from extensions
        let pool = parts
            .extensions
            .get::<SqlitePool>()
            .ok_or(AppError::Internal(
                "Database pool not configured".to_string(),
            ))?;

        // Check if user is a member of this project
        let member: Option<(String, Option<i64>)> = sqlx::query_as(
            "SELECT role, participant_id FROM project_members
             WHERE project_id = ? AND user_id = ?",
        )
        .bind(project_id)
        .bind(auth_user.user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

        match member {
            Some((role_str, participant_id)) => {
                let role = role_str
                    .parse::<Role>()
                    .map_err(|_| AppError::Internal("Invalid role in database".to_string()))?;

                Ok(ProjectMember {
                    user_id: auth_user.user_id,
                    username: auth_user.username,
                    project_id,
                    role,
                    participant_id,
                })
            }
            None => Err(AppError::NotFound(
                "Project not found or access denied".to_string(),
            )),
        }
    }
}

/// Require admin role
#[derive(Debug, Clone)]
pub struct AdminMember(pub ProjectMember);

impl<S> FromRequestParts<S> for AdminMember
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let member = ProjectMember::from_request_parts(parts, state).await?;

        if member.role != Role::Admin {
            return Err(AppError::Forbidden("Admin access required".to_string()));
        }

        Ok(AdminMember(member))
    }
}

/// Require editor or admin role
#[derive(Debug, Clone)]
pub struct EditorMember(pub ProjectMember);

impl<S> FromRequestParts<S> for EditorMember
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let member = ProjectMember::from_request_parts(parts, state).await?;

        if member.role < Role::Editor {
            return Err(AppError::Forbidden("Editor access required".to_string()));
        }

        Ok(EditorMember(member))
    }
}
