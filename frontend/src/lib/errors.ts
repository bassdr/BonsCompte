import { get } from 'svelte/store';
import { _ } from 'svelte-i18n';

/**
 * All backend error codes that can be translated on the frontend.
 * These match the ErrorCode enum in the Rust backend.
 */
export type BackendErrorCode =
  // Authentication
  | 'UNAUTHORIZED'
  | 'INVALID_CREDENTIALS'
  | 'ACCOUNT_PENDING'
  | 'ACCOUNT_REVOKED'
  | 'TOKEN_EXPIRED'
  | 'TOKEN_INVALIDATED'
  | 'INVALID_TOKEN'
  | 'USERNAME_EXISTS'
  | 'PASSWORD_TOO_WEAK'
  | 'USERNAME_REQUIRED'
  // Validation
  | 'INVALID_INPUT'
  | 'AMOUNT_MUST_BE_POSITIVE'
  | 'CONTRIBUTION_REQUIRED'
  | 'TOTAL_WEIGHT_MUST_BE_POSITIVE'
  | 'INVALID_PAYER'
  | 'INVALID_RECEIVER'
  | 'INVALID_PARTICIPANT'
  | 'INVALID_ROLE'
  | 'INVALID_DATE_FORMAT'
  | 'INVALID_DECIMAL_SEPARATOR'
  | 'INVALID_CURRENCY_POSITION'
  | 'INVALID_ACCOUNT_TYPE'
  | 'INVALID_VOTE'
  | 'INVALID_WARNING_HORIZON'
  | 'PASSWORD_MISMATCH'
  | 'NO_FIELDS_TO_UPDATE'
  // Not found
  | 'NOT_FOUND'
  | 'USER_NOT_FOUND'
  | 'PROJECT_NOT_FOUND'
  | 'PARTICIPANT_NOT_FOUND'
  | 'PAYMENT_NOT_FOUND'
  | 'MEMBER_NOT_FOUND'
  | 'INVITE_NOT_FOUND'
  | 'RECOVERY_NOT_FOUND'
  | 'APPROVAL_NOT_FOUND'
  // Permission
  | 'FORBIDDEN'
  | 'ADMIN_REQUIRED'
  | 'EDITOR_REQUIRED'
  | 'NOT_TRUSTED_USER'
  | 'CANNOT_MODIFY_SELF'
  | 'CANNOT_REMOVE_SELF'
  // Business logic
  | 'INVITES_DISABLED'
  | 'ALREADY_MEMBER'
  | 'INVITE_ALREADY_USED'
  | 'PARTICIPANT_ALREADY_CLAIMED'
  | 'PARTICIPANT_ALREADY_LINKED'
  | 'ALREADY_HAS_PARTICIPANT'
  | 'LINKED_USER_CANNOT_BE_POOL'
  | 'POOL_WARNING_ONLY_FOR_POOLS'
  | 'PROJECT_LIMIT_REACHED'
  | 'MEMBER_ALREADY_ACTIVE'
  | 'CANNOT_APPROVE_MEMBER'
  | 'CANNOT_REJECT_MEMBER'
  | 'CANNOT_REJECT_ACTIVE_MEMBER'
  | 'RECOVERY_EXPIRED'
  | 'RECOVERY_NOT_PENDING'
  | 'RECOVERY_NOT_APPROVED'
  | 'RECOVERY_INSUFFICIENT_TRUSTED_USERS'
  | 'CANNOT_TRUST_SELF'
  | 'ALREADY_TRUSTED_USER'
  | 'CANNOT_DELETE_ACCOUNT_NO_ADMIN'
  // Image
  | 'INVALID_BASE64_IMAGE'
  | 'IMAGE_TOO_LARGE'
  | 'IMAGE_EMPTY'
  | 'INVALID_IMAGE_FORMAT'
  // Internal
  | 'INTERNAL_ERROR'
  | 'DATABASE_ERROR'
  // Network/Unknown
  | 'NETWORK_ERROR'
  | 'UNKNOWN';

/**
 * Maps backend error codes to translation keys.
 * The translation key will be looked up as `errors.{translationKey}`.
 */
const errorCodeToTranslationKey: Record<BackendErrorCode, string> = {
  // Authentication
  UNAUTHORIZED: 'unauthorized',
  INVALID_CREDENTIALS: 'invalidCredentials',
  ACCOUNT_PENDING: 'accountPending',
  ACCOUNT_REVOKED: 'accountRevoked',
  TOKEN_EXPIRED: 'tokenExpired',
  TOKEN_INVALIDATED: 'tokenInvalidated',
  INVALID_TOKEN: 'invalidToken',
  USERNAME_EXISTS: 'usernameExists',
  PASSWORD_TOO_WEAK: 'passwordTooWeak',
  USERNAME_REQUIRED: 'usernameRequired',

  // Validation
  INVALID_INPUT: 'invalidInput',
  AMOUNT_MUST_BE_POSITIVE: 'amountMustBePositive',
  CONTRIBUTION_REQUIRED: 'contributionRequired',
  TOTAL_WEIGHT_MUST_BE_POSITIVE: 'totalWeightMustBePositive',
  INVALID_PAYER: 'invalidPayer',
  INVALID_RECEIVER: 'invalidReceiver',
  INVALID_PARTICIPANT: 'invalidParticipant',
  INVALID_ROLE: 'invalidRole',
  INVALID_DATE_FORMAT: 'invalidDateFormat',
  INVALID_DECIMAL_SEPARATOR: 'invalidDecimalSeparator',
  INVALID_CURRENCY_POSITION: 'invalidCurrencyPosition',
  INVALID_ACCOUNT_TYPE: 'invalidAccountType',
  INVALID_VOTE: 'invalidVote',
  INVALID_WARNING_HORIZON: 'invalidWarningHorizon',
  PASSWORD_MISMATCH: 'passwordMismatch',
  NO_FIELDS_TO_UPDATE: 'noFieldsToUpdate',

  // Not found
  NOT_FOUND: 'notFound',
  USER_NOT_FOUND: 'userNotFound',
  PROJECT_NOT_FOUND: 'projectNotFound',
  PARTICIPANT_NOT_FOUND: 'participantNotFound',
  PAYMENT_NOT_FOUND: 'paymentNotFound',
  MEMBER_NOT_FOUND: 'memberNotFound',
  INVITE_NOT_FOUND: 'inviteNotFound',
  RECOVERY_NOT_FOUND: 'recoveryNotFound',
  APPROVAL_NOT_FOUND: 'approvalNotFound',

  // Permission
  FORBIDDEN: 'forbidden',
  ADMIN_REQUIRED: 'adminRequired',
  EDITOR_REQUIRED: 'editorRequired',
  NOT_TRUSTED_USER: 'notTrustedUser',
  CANNOT_MODIFY_SELF: 'cannotModifySelf',
  CANNOT_REMOVE_SELF: 'cannotRemoveSelf',

  // Business logic
  INVITES_DISABLED: 'invitesDisabled',
  ALREADY_MEMBER: 'alreadyMember',
  INVITE_ALREADY_USED: 'inviteAlreadyUsed',
  PARTICIPANT_ALREADY_CLAIMED: 'participantAlreadyClaimed',
  PARTICIPANT_ALREADY_LINKED: 'participantAlreadyLinked',
  ALREADY_HAS_PARTICIPANT: 'alreadyHasParticipant',
  LINKED_USER_CANNOT_BE_POOL: 'linkedUserCannotBePool',
  POOL_WARNING_ONLY_FOR_POOLS: 'poolWarningOnlyForPools',
  PROJECT_LIMIT_REACHED: 'projectLimitReached',
  MEMBER_ALREADY_ACTIVE: 'memberAlreadyActive',
  CANNOT_APPROVE_MEMBER: 'cannotApproveMember',
  CANNOT_REJECT_MEMBER: 'cannotRejectMember',
  CANNOT_REJECT_ACTIVE_MEMBER: 'cannotRejectActiveMember',
  RECOVERY_EXPIRED: 'recoveryExpired',
  RECOVERY_NOT_PENDING: 'recoveryNotPending',
  RECOVERY_NOT_APPROVED: 'recoveryNotApproved',
  RECOVERY_INSUFFICIENT_TRUSTED_USERS: 'recoveryInsufficientTrustedUsers',
  CANNOT_TRUST_SELF: 'cannotTrustSelf',
  ALREADY_TRUSTED_USER: 'alreadyTrustedUser',
  CANNOT_DELETE_ACCOUNT_NO_ADMIN: 'cannotDeleteAccountNoAdmin',

  // Image
  INVALID_BASE64_IMAGE: 'invalidBase64Image',
  IMAGE_TOO_LARGE: 'imageTooLarge',
  IMAGE_EMPTY: 'imageEmpty',
  INVALID_IMAGE_FORMAT: 'invalidImageFormat',

  // Internal
  INTERNAL_ERROR: 'internalError',
  DATABASE_ERROR: 'databaseError',

  // Network/Unknown
  NETWORK_ERROR: 'networkError',
  UNKNOWN: 'unknown'
};

/**
 * Translates a backend error code to a localized message.
 * Falls back to the error code itself if no translation is found.
 *
 * @param code - The backend error code
 * @returns The translated error message
 */
export function translateErrorCode(code: string): string {
  const $_ = get(_);
  const translationKey = errorCodeToTranslationKey[code as BackendErrorCode];

  if (translationKey) {
    const translated = $_(`errors.${translationKey}`);
    // If translation returns the key itself, the translation is missing
    if (translated !== `errors.${translationKey}`) {
      return translated;
    }
  }

  // Fallback: return a generic error message
  return $_('errors.unknown');
}

/**
 * Checks if an error is a network error (backend unreachable).
 *
 * @param error - The error to check
 * @returns True if this is a network error
 */
export function isNetworkError(error: unknown): boolean {
  if (error instanceof TypeError && error.message === 'Failed to fetch') {
    return true;
  }
  return false;
}

/**
 * Gets the appropriate error message from an error object.
 * Handles network errors, ApiRequestError, and plain Error objects.
 *
 * @param error - The error to get a message from
 * @param fallbackKey - Optional fallback translation key (default: 'errors.unknown')
 * @returns The translated error message
 */
export function getErrorMessage(error: unknown, fallbackKey = 'errors.unknown'): string {
  const $_ = get(_);

  // Network error
  if (isNetworkError(error)) {
    return $_('errors.networkError');
  }

  // ApiRequestError with code
  if (error && typeof error === 'object' && 'code' in error) {
    const code = (error as { code: string }).code;
    return translateErrorCode(code);
  }

  // Regular Error
  if (error instanceof Error) {
    // Don't expose raw error messages - use fallback
    return $_(fallbackKey);
  }

  return $_(fallbackKey);
}
