use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Standardized error codes for frontend translation
/// All user-facing errors should use these codes for consistent i18n support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Authentication errors
    Unauthorized,
    InvalidCredentials,
    AccountPending,
    AccountRevoked,
    TokenExpired,
    TokenInvalidated,
    InvalidToken,
    UsernameExists,
    PasswordTooWeak,
    UsernameRequired,

    // Validation errors
    InvalidInput,
    AmountMustBePositive,
    ContributionRequired,
    TotalWeightMustBePositive,
    InvalidPayer,
    InvalidReceiver,
    InvalidParticipant,
    InvalidRole,
    InvalidDateFormat,
    InvalidDecimalSeparator,
    InvalidCurrencyPosition,
    InvalidAccountType,
    InvalidVote,
    InvalidWarningHorizon,
    InvalidPendingAccessSetting,
    PasswordMismatch,
    NoFieldsToUpdate,

    // Not found errors
    NotFound,
    UserNotFound,
    ProjectNotFound,
    ParticipantNotFound,
    PaymentNotFound,
    MemberNotFound,
    InviteNotFound,
    RecoveryNotFound,
    ApprovalNotFound,

    // Permission/access errors
    Forbidden,
    AdminRequired,
    EditorRequired,
    NotTrustedUser,
    CannotModifySelf,
    CannotRemoveSelf,

    // Business logic errors
    InvitesDisabled,
    AlreadyMember,
    InviteAlreadyUsed,
    ParticipantAlreadyClaimed,
    ParticipantAlreadyLinked,
    AlreadyHasParticipant,
    LinkedUserCannotBePool,
    PoolWarningOnlyForPools,
    ProjectLimitReached,
    MemberAlreadyActive,
    CannotApproveMember,
    CannotRejectMember,
    CannotRejectActiveMember,
    RecoveryExpired,
    RecoveryNotPending,
    RecoveryNotApproved,
    RecoveryInsufficientTrustedUsers,
    CannotTrustSelf,
    AlreadyTrustedUser,
    CannotDeleteAccountNoAdmin,

    // Image validation errors
    InvalidBase64Image,
    ImageTooLarge,
    ImageEmpty,
    InvalidImageFormat,

    // Internal errors
    InternalError,
    DatabaseError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            // Authentication
            Self::Unauthorized => "UNAUTHORIZED",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::AccountPending => "ACCOUNT_PENDING",
            Self::AccountRevoked => "ACCOUNT_REVOKED",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::TokenInvalidated => "TOKEN_INVALIDATED",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::UsernameExists => "USERNAME_EXISTS",
            Self::PasswordTooWeak => "PASSWORD_TOO_WEAK",
            Self::UsernameRequired => "USERNAME_REQUIRED",

            // Validation
            Self::InvalidInput => "INVALID_INPUT",
            Self::AmountMustBePositive => "AMOUNT_MUST_BE_POSITIVE",
            Self::ContributionRequired => "CONTRIBUTION_REQUIRED",
            Self::TotalWeightMustBePositive => "TOTAL_WEIGHT_MUST_BE_POSITIVE",
            Self::InvalidPayer => "INVALID_PAYER",
            Self::InvalidReceiver => "INVALID_RECEIVER",
            Self::InvalidParticipant => "INVALID_PARTICIPANT",
            Self::InvalidRole => "INVALID_ROLE",
            Self::InvalidDateFormat => "INVALID_DATE_FORMAT",
            Self::InvalidDecimalSeparator => "INVALID_DECIMAL_SEPARATOR",
            Self::InvalidCurrencyPosition => "INVALID_CURRENCY_POSITION",
            Self::InvalidAccountType => "INVALID_ACCOUNT_TYPE",
            Self::InvalidVote => "INVALID_VOTE",
            Self::InvalidWarningHorizon => "INVALID_WARNING_HORIZON",
            Self::InvalidPendingAccessSetting => "INVALID_PENDING_ACCESS_SETTING",
            Self::PasswordMismatch => "PASSWORD_MISMATCH",
            Self::NoFieldsToUpdate => "NO_FIELDS_TO_UPDATE",

            // Not found
            Self::NotFound => "NOT_FOUND",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::ProjectNotFound => "PROJECT_NOT_FOUND",
            Self::ParticipantNotFound => "PARTICIPANT_NOT_FOUND",
            Self::PaymentNotFound => "PAYMENT_NOT_FOUND",
            Self::MemberNotFound => "MEMBER_NOT_FOUND",
            Self::InviteNotFound => "INVITE_NOT_FOUND",
            Self::RecoveryNotFound => "RECOVERY_NOT_FOUND",
            Self::ApprovalNotFound => "APPROVAL_NOT_FOUND",

            // Permission
            Self::Forbidden => "FORBIDDEN",
            Self::AdminRequired => "ADMIN_REQUIRED",
            Self::EditorRequired => "EDITOR_REQUIRED",
            Self::NotTrustedUser => "NOT_TRUSTED_USER",
            Self::CannotModifySelf => "CANNOT_MODIFY_SELF",
            Self::CannotRemoveSelf => "CANNOT_REMOVE_SELF",

            // Business logic
            Self::InvitesDisabled => "INVITES_DISABLED",
            Self::AlreadyMember => "ALREADY_MEMBER",
            Self::InviteAlreadyUsed => "INVITE_ALREADY_USED",
            Self::ParticipantAlreadyClaimed => "PARTICIPANT_ALREADY_CLAIMED",
            Self::ParticipantAlreadyLinked => "PARTICIPANT_ALREADY_LINKED",
            Self::AlreadyHasParticipant => "ALREADY_HAS_PARTICIPANT",
            Self::LinkedUserCannotBePool => "LINKED_USER_CANNOT_BE_POOL",
            Self::PoolWarningOnlyForPools => "POOL_WARNING_ONLY_FOR_POOLS",
            Self::ProjectLimitReached => "PROJECT_LIMIT_REACHED",
            Self::MemberAlreadyActive => "MEMBER_ALREADY_ACTIVE",
            Self::CannotApproveMember => "CANNOT_APPROVE_MEMBER",
            Self::CannotRejectMember => "CANNOT_REJECT_MEMBER",
            Self::CannotRejectActiveMember => "CANNOT_REJECT_ACTIVE_MEMBER",
            Self::RecoveryExpired => "RECOVERY_EXPIRED",
            Self::RecoveryNotPending => "RECOVERY_NOT_PENDING",
            Self::RecoveryNotApproved => "RECOVERY_NOT_APPROVED",
            Self::RecoveryInsufficientTrustedUsers => "RECOVERY_INSUFFICIENT_TRUSTED_USERS",
            Self::CannotTrustSelf => "CANNOT_TRUST_SELF",
            Self::AlreadyTrustedUser => "ALREADY_TRUSTED_USER",
            Self::CannotDeleteAccountNoAdmin => "CANNOT_DELETE_ACCOUNT_NO_ADMIN",

            // Image
            Self::InvalidBase64Image => "INVALID_BASE64_IMAGE",
            Self::ImageTooLarge => "IMAGE_TOO_LARGE",
            Self::ImageEmpty => "IMAGE_EMPTY",
            Self::InvalidImageFormat => "INVALID_IMAGE_FORMAT",

            // Internal
            Self::InternalError => "INTERNAL_ERROR",
            Self::DatabaseError => "DATABASE_ERROR",
        }
    }
}

/// Legacy auth failure reasons - kept for backward compatibility with logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailureReason {
    PasswordTooWeak,
    UsernameExists,
    InvalidCredentials,
    InvalidInput,
    InternalError,
    AccountPending,
    AccountRevoked,
}

impl AuthFailureReason {
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::PasswordTooWeak => "PASSWORD_TOO_WEAK",
            Self::UsernameExists => "USERNAME_EXISTS",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::InvalidInput => "INVALID_INPUT",
            Self::InternalError => "INTERNAL_ERROR",
            Self::AccountPending => "ACCOUNT_PENDING",
            Self::AccountRevoked => "ACCOUNT_REVOKED",
        }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Account pending approval")]
    AccountPendingApproval,

    #[error("Account revoked")]
    AccountRevoked,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token invalidated")]
    TokenInvalidated,

    #[error("Invalid token")]
    InvalidToken,

    #[error("User already exists")]
    UserExists,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    // New: Coded error with standardized error code for frontend translation
    #[error("{1}")]
    Coded(ErrorCode, String),
}

impl AppError {
    /// Create a coded bad request error
    pub fn bad_request(code: ErrorCode) -> Self {
        Self::Coded(code, code.as_str().to_string())
    }

    /// Create a coded not found error
    pub fn not_found(code: ErrorCode) -> Self {
        Self::Coded(code, code.as_str().to_string())
    }

    /// Create a coded forbidden error
    pub fn forbidden(code: ErrorCode) -> Self {
        Self::Coded(code, code.as_str().to_string())
    }

    /// Create a coded validation error
    pub fn validation(code: ErrorCode) -> Self {
        Self::Coded(code, code.as_str().to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::Unauthorized.as_str(),
                self.to_string(),
            ),
            AppError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::InvalidCredentials.as_str(),
                self.to_string(),
            ),
            AppError::AccountPendingApproval => (
                StatusCode::FORBIDDEN,
                ErrorCode::AccountPending.as_str(),
                self.to_string(),
            ),
            AppError::AccountRevoked => (
                StatusCode::FORBIDDEN,
                ErrorCode::AccountRevoked.as_str(),
                self.to_string(),
            ),
            AppError::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::TokenExpired.as_str(),
                ErrorCode::TokenExpired.as_str().to_string(),
            ),
            AppError::TokenInvalidated => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::TokenInvalidated.as_str(),
                ErrorCode::TokenInvalidated.as_str().to_string(),
            ),
            AppError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::InvalidToken.as_str(),
                ErrorCode::InvalidToken.as_str().to_string(),
            ),
            AppError::UserExists => (
                StatusCode::CONFLICT,
                ErrorCode::UsernameExists.as_str(),
                self.to_string(),
            ),
            AppError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound.as_str(),
                ErrorCode::NotFound.as_str().to_string(),
            ),
            AppError::BadRequest(_) => (
                StatusCode::BAD_REQUEST,
                ErrorCode::InvalidInput.as_str(),
                ErrorCode::InvalidInput.as_str().to_string(),
            ),
            AppError::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidInput.as_str(),
                ErrorCode::InvalidInput.as_str().to_string(),
            ),
            AppError::Forbidden(_) => (
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden.as_str(),
                ErrorCode::Forbidden.as_str().to_string(),
            ),
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorCode::InternalError.as_str(),
                    ErrorCode::InternalError.as_str().to_string(),
                )
            }
            AppError::Jwt(e) => {
                // Don't log JWT errors as ERROR - they're expected behavior
                tracing::debug!("JWT validation failed: {:?}", e);
                (
                    StatusCode::UNAUTHORIZED,
                    ErrorCode::InvalidToken.as_str(),
                    ErrorCode::InvalidToken.as_str().to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorCode::InternalError.as_str(),
                    ErrorCode::InternalError.as_str().to_string(),
                )
            }
            AppError::Coded(error_code, _) => {
                let status = match error_code {
                    // Authentication errors -> 401
                    ErrorCode::Unauthorized
                    | ErrorCode::InvalidCredentials
                    | ErrorCode::TokenExpired
                    | ErrorCode::TokenInvalidated
                    | ErrorCode::InvalidToken => StatusCode::UNAUTHORIZED,

                    // Account state errors -> 403
                    ErrorCode::AccountPending | ErrorCode::AccountRevoked => StatusCode::FORBIDDEN,

                    // Conflict errors -> 409
                    ErrorCode::UsernameExists
                    | ErrorCode::AlreadyMember
                    | ErrorCode::AlreadyTrustedUser
                    | ErrorCode::ParticipantAlreadyClaimed
                    | ErrorCode::ParticipantAlreadyLinked
                    | ErrorCode::AlreadyHasParticipant
                    | ErrorCode::MemberAlreadyActive => StatusCode::CONFLICT,

                    // Not found errors -> 404
                    ErrorCode::NotFound
                    | ErrorCode::UserNotFound
                    | ErrorCode::ProjectNotFound
                    | ErrorCode::ParticipantNotFound
                    | ErrorCode::PaymentNotFound
                    | ErrorCode::MemberNotFound
                    | ErrorCode::InviteNotFound
                    | ErrorCode::RecoveryNotFound
                    | ErrorCode::ApprovalNotFound => StatusCode::NOT_FOUND,

                    // Permission errors -> 403
                    ErrorCode::Forbidden
                    | ErrorCode::AdminRequired
                    | ErrorCode::EditorRequired
                    | ErrorCode::NotTrustedUser
                    | ErrorCode::InvitesDisabled
                    | ErrorCode::ProjectLimitReached => StatusCode::FORBIDDEN,

                    // Internal errors -> 500
                    ErrorCode::InternalError | ErrorCode::DatabaseError => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }

                    // All other errors -> 400 Bad Request
                    _ => StatusCode::BAD_REQUEST,
                };
                (status, error_code.as_str(), error_code.as_str().to_string())
            }
        };

        let body = Json(json!({ "error": message, "code": code }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
