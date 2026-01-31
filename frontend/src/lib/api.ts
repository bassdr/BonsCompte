import { auth, type User, type UserPreferences } from './auth';
import { browser } from '$app/environment';

// Function to get API base URL - exported for testing
export function getApiBaseUrl(): string {
  // 1. Environment variable takes precedence (for custom deployments)
  if (import.meta.env.VITE_API_BASE) {
    return import.meta.env.VITE_API_BASE;
  }

  // 2. In browser and HTTPS (production), use /api (NGINX reverse proxy)
  if (browser && window.location.protocol === 'https:') {
    return '/api';
  }

  // 3. Development default
  return 'http://localhost:8000';
}

// Default to localhost:8000 for development
// In production (served over HTTPS), use /api which NGINX proxies to backend
// Override with VITE_API_BASE in docker builds or custom deployments
const BASE = getApiBaseUrl();

// Error codes returned by the backend for structured error handling
export type AuthErrorCode =
  | 'TOKEN_EXPIRED'
  | 'TOKEN_INVALIDATED'
  | 'INVALID_TOKEN'
  | 'UNAUTHORIZED'
  | 'PASSWORD_TOO_WEAK'
  | 'USERNAME_EXISTS'
  | 'INVALID_CREDENTIALS'
  | 'INVALID_INPUT'
  | 'ACCOUNT_PENDING'
  | 'PENDING_APPROVAL'
  | 'ACCOUNT_REVOKED'
  | 'INTERNAL_ERROR';

export interface ApiError {
  code: string;
  error: string;
}

export class ApiRequestError extends Error {
  code: string;
  status: number;

  constructor(code: string, message: string, status: number) {
    super(message);
    this.code = code;
    this.status = status;
    this.name = 'ApiRequestError';
  }
}

interface AuthFetchOptions extends RequestInit {
  // If true, 404 responses return null instead of throwing
  nullOn404?: boolean;
}

async function authFetch(path: string, opts: AuthFetchOptions = {}) {
  const { nullOn404, ...fetchOpts } = opts;
  const token = auth.getToken();

  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(token && { Authorization: `Bearer ${token}` }),
    ...(fetchOpts.headers as Record<string, string>)
  };

  let res: Response;
  try {
    res = await fetch(`${BASE}${path}`, { ...fetchOpts, headers });
  } catch (e) {
    // Network error - backend is down or unreachable
    if (e instanceof TypeError && e.message === 'Failed to fetch') {
      throw new ApiRequestError('NETWORK_ERROR', 'NETWORK_ERROR', 0);
    }
    throw e;
  }

  // Handle 404 silently if nullOn404 is set
  if (res.status === 404 && nullOn404) {
    return null;
  }

  if (res.status === 401) {
    // Try to get the error code to distinguish expired vs invalid
    const text = await res.text();
    try {
      const data: ApiError = JSON.parse(text);
      if (data.code === 'TOKEN_EXPIRED' || data.code === 'TOKEN_INVALIDATED') {
        auth.logout();
        if (browser) {
          // Store a message for the login page
          sessionStorage.setItem('auth_message', 'session_expired');
          window.location.href = '/login';
        }
        throw new ApiRequestError(data.code, data.error, res.status);
      }
    } catch (e) {
      if (e instanceof ApiRequestError) throw e;
      // If we can't parse the response, treat as generic 401
    }
    auth.logout();
    if (browser) {
      window.location.href = '/login';
    }
    throw new Error('Session expired');
  }

  // Handle 403 - check for pending approval or revoked account
  if (res.status === 403) {
    const text = await res.text();
    try {
      const data: ApiError = JSON.parse(text);
      if (data.code === 'ACCOUNT_PENDING' || data.code === 'PENDING_APPROVAL') {
        // Redirect to quarantine page, don't clear token
        if (browser) {
          window.location.href = '/account/pending';
        }
        throw new ApiRequestError(data.code, data.error, res.status);
      }
      if (data.code === 'ACCOUNT_REVOKED') {
        // Logout revoked users
        auth.logout();
        if (browser) {
          sessionStorage.setItem('auth_message', 'account_revoked');
          window.location.href = '/login';
        }
        throw new ApiRequestError(data.code, data.error, res.status);
      }
      // Other 403 errors
      throw new ApiRequestError(data.code || 'FORBIDDEN', data.error, res.status);
    } catch (e) {
      if (e instanceof ApiRequestError) throw e;
      throw new Error(`${res.status}: ${text}`);
    }
  }

  if (!res.ok) {
    // Read response body once as text, then try to parse as JSON
    const text = await res.text();
    try {
      const data: ApiError = JSON.parse(text);
      throw new ApiRequestError(data.code || 'UNKNOWN', data.error, res.status);
    } catch (e) {
      if (e instanceof ApiRequestError) throw e;
      // If not valid JSON, throw with the raw text
      throw new Error(`${res.status}: ${text}`);
    }
  }

  return res.json();
}

// Auth endpoints (public)
export interface AuthResponse {
  token: string;
  user: User;
}

export async function login(username: string, password: string): Promise<AuthResponse> {
  let res: Response;
  try {
    res = await fetch(`${BASE}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password })
    });
  } catch (e) {
    // Network error - backend is down or unreachable
    if (e instanceof TypeError && e.message === 'Failed to fetch') {
      throw new ApiRequestError('NETWORK_ERROR', 'NETWORK_ERROR', 0);
    }
    throw e;
  }

  if (!res.ok) {
    const data: ApiError = await res
      .json()
      .catch(() => ({ error: 'Login failed', code: 'UNKNOWN' }));
    throw new ApiRequestError(data.code || 'UNKNOWN', data.error || 'Login failed', res.status);
  }

  return res.json();
}

export async function register(
  username: string,
  password: string,
  display_name?: string
): Promise<AuthResponse> {
  let res: Response;
  try {
    res = await fetch(`${BASE}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password, display_name })
    });
  } catch (e) {
    // Network error - backend is down or unreachable
    if (e instanceof TypeError && e.message === 'Failed to fetch') {
      throw new ApiRequestError('NETWORK_ERROR', 'NETWORK_ERROR', 0);
    }
    throw e;
  }

  if (!res.ok) {
    const data: ApiError = await res
      .json()
      .catch(() => ({ error: 'Registration failed', code: 'UNKNOWN' }));
    throw new ApiRequestError(
      data.code || 'UNKNOWN',
      data.error || 'Registration failed',
      res.status
    );
  }

  return res.json();
}

// User management
export const changePassword = (
  currentPassword: string,
  newPassword: string
): Promise<{ message: string }> =>
  authFetch('/users/me/password', {
    method: 'PUT',
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword })
  });

export const deleteAccount = (password: string): Promise<DeleteAccountResponse> =>
  authFetch('/users/me', {
    method: 'DELETE',
    body: JSON.stringify({ password })
  });

// User profile
export interface UpdateProfileRequest {
  display_name?: string | null;
}

export const updateProfile = (data: UpdateProfileRequest): Promise<User> =>
  authFetch('/users/me/profile', {
    method: 'PUT',
    body: JSON.stringify(data)
  });

// User preferences
export const getPreferences = (): Promise<UserPreferences> => authFetch('/users/me/preferences');

export const updatePreferences = (prefs: Partial<UserPreferences>): Promise<UserPreferences> =>
  authFetch('/users/me/preferences', {
    method: 'PUT',
    body: JSON.stringify(prefs)
  });

// Types
export interface Project {
  id: number;
  name: string;
  description: string | null;
  invite_code: string | null;
  created_by: number;
  created_at: string;
  invites_enabled: boolean;
  require_approval: boolean;
  pool_warning_horizon: string; // Deprecated: now per-pool in participants table
  pending_member_access: 'none' | 'read_only' | 'auto_approve';
}

export interface PoolSummary {
  pool_name: string;
  ownership: number;
}

export interface ProjectWithRole extends Project {
  role: string;
  owner_name: string;
  user_balance: number | null;
  user_pools: PoolSummary[];
  member_status: 'active' | 'pending' | 'recovered';
}

export interface Participant {
  id: number;
  project_id: number;
  name: string;
  user_id: number | null;
  default_weight: number;
  created_at: string;
  account_type: 'user' | 'pool';
  warning_horizon_account: string | null; // Pool warning horizon for account total (null = disabled)
  warning_horizon_users: string | null; // Pool warning horizon for user ownership (null = disabled)
}

export interface ProjectMember {
  id: number;
  project_id: number;
  user_id: number;
  username: string;
  display_name: string | null;
  role: string;
  participant_id: number | null;
  participant_name: string | null;
  joined_at: string;
  status: 'pending' | 'active' | 'rejected' | 'recovered';
}

export interface ParticipantInvite {
  id: number;
  participant_id: number;
  invite_token: string;
  created_at: string;
  expires_at: string | null;
  used_by: number | null;
  used_at: string | null;
}

export interface JoinProjectResponse {
  project: Project;
  status: string;
  participant_id: number | null;
}

export interface DeleteAccountResponse {
  message: string;
  affected_projects: Array<{
    project_id: number;
    project_name: string;
    outcome: 'transferred' | 'deleted';
    transferred_to: string | null;
  }>;
}

export interface Payment {
  id: number;
  project_id: number | null;
  payer_id: number | null;
  amount: number;
  description: string;
  payment_date: string;
  created_at: string;
  // Receipt image (Base64 encoded)
  receipt_image: string | null;
  // Recurrence fields
  is_recurring: boolean;
  recurrence_type: string | null; // 'daily' | 'weekly' | 'monthly' | 'yearly'
  recurrence_interval: number | null;
  recurrence_times_per: number | null;
  recurrence_end_date: string | null;
  // Enhanced recurrence patterns (JSON strings)
  recurrence_weekdays: string | null; // e.g., "[[1,3],[0,5]]" - weekdays per week in cycle
  recurrence_monthdays: string | null; // e.g., "[1, 15, 28]" - days of month
  recurrence_months: string | null; // e.g., "[1, 6, 12]" - months (1=Jan, 12=Dec)
  // Internal transfer support
  // null = external expense (money leaves system, affects settlements)
  // number = internal transfer to this account (only affects pool ownership)
  receiver_account_id: number | null;
  // Payment finalization status: true (default) = final, false = draft
  is_final: boolean;
  // Dual ledger flags for pool expected minimum
  // affects_balance: Transaction affects actual pool balance (default: true)
  affects_balance: boolean;
  // affects_payer_expectation: When payer is a pool and true, reduces payer's expected minimum
  affects_payer_expectation: boolean;
  // affects_receiver_expectation: When receiver is a pool and true, increases receiver's expected minimum
  affects_receiver_expectation: boolean;
}

export interface Contribution {
  id: number;
  participant_id: number;
  participant_name: string;
  payment_id: number;
  amount: number;
  weight: number;
}

export interface PaymentWithContributions extends Payment {
  payer_name: string | null;
  contributions: Contribution[];
}

export interface ParticipantBalance {
  participant_id: number;
  participant_name: string;
  total_paid: number;
  total_owed: number;
  net_balance: number;
}

export interface Debt {
  from_participant_id: number;
  from_participant_name: string;
  to_participant_id: number;
  to_participant_name: string;
  amount: number;
}

export interface PaymentOccurrence {
  payment_id: number;
  description: string;
  amount: number;
  occurrence_date: string;
  payer_id: number | null;
  is_recurring: boolean;
  // Internal transfer support
  receiver_account_id: number | null;
  // Payment finalization status: true = final, false = draft
  is_final: boolean;
  // Dual ledger flags for pool expected minimum
  affects_balance: boolean;
  affects_payer_expectation: boolean;
  affects_receiver_expectation: boolean;
}

export interface PairwisePaymentBreakdown {
  payment_id: number;
  description: string;
  occurrence_date: string;
  amount: number;
}

export interface PairwiseBalance {
  participant_id: number;
  participant_name: string;
  other_participant_id: number;
  other_participant_name: string;
  amount_paid_for: number; // Amount this participant paid for other
  amount_owed_by: number; // Amount other paid for this participant
  net: number; // paid_for - owed_by (positive = they owe you)
  paid_for_breakdown: PairwisePaymentBreakdown[]; // Details of what we paid for them
  owed_by_breakdown: PairwisePaymentBreakdown[]; // Details of what they paid for us
}

export interface PoolOwnershipEntry {
  participant_id: number;
  participant_name: string;
  contributed: number;
  consumed: number;
  ownership: number;
  contributed_breakdown: PairwisePaymentBreakdown[];
  consumed_breakdown: PairwisePaymentBreakdown[];
}

export interface PoolOwnership {
  pool_id: number;
  pool_name: string;
  entries: PoolOwnershipEntry[];
  total_balance: number;
  // Dual ledger: expected minimum from affects_expectation=true transactions
  expected_minimum: number;
  is_below_expected: boolean; // total_balance < expected_minimum
  shortfall: number | null; // expected_minimum - total_balance (if positive)
}

export interface DebtSummary {
  balances: ParticipantBalance[];
  settlements: Debt[];
  direct_settlements: Debt[];
  target_date: string;
  occurrences: PaymentOccurrence[];
  pairwise_balances: PairwiseBalance[];
  pool_ownerships: PoolOwnership[];
}

// Cashflow Planning
export interface MonthlyBalance {
  month: string;
  participant_id: number;
  participant_name: string;
  net_balance: number;
}

export interface RecurringContributionRecommendation {
  participant_id: number;
  participant_name: string;
  recommended_amount: number;
  frequency_type: string;
  frequency_interval: number;
  current_trend: number;
  calculation_method: string;
}

export interface PoolOwnershipSnapshot {
  participant_id: number;
  participant_name: string;
  ownership: number;
}

export interface PoolMonthlyBalance {
  month: string;
  total_balance: number;
  participant_ownerships: PoolOwnershipSnapshot[];
}

export interface PoolEvolution {
  pool_id: number;
  pool_name: string;
  monthly_balances: PoolMonthlyBalance[];
}

export interface RecurringPaymentToConsolidate {
  payment_id: number;
  description: string;
  amount: number;
  payer_id: number;
  payer_name: string;
  recurrence_type: string;
  recurrence_interval: number;
}

export interface BalanceEvent {
  date: string;
  participant_id: number;
  participant_name: string;
  net_balance: number;
  is_synthetic: boolean;
}

export interface ComputedRecommendation {
  payer_id: number;
  payer_name: string;
  receiver_id: number;
  receiver_name: string;
  recommended_amount: number;
  frequency_type: string;
  frequency_interval: number;
  current_trend: number;
  calculation_method: string;
  start_date: string;
  end_date: string;
}

export interface CashflowProjection {
  start_date: string;
  end_date: string;
  months: string[];
  monthly_balances: MonthlyBalance[];
  recommendations: RecurringContributionRecommendation[];
  pool_evolutions: PoolEvolution[];
  consolidate_mode: boolean;
  payments_to_consolidate: RecurringPaymentToConsolidate[];
  // New fields for recommendation-driven view
  balance_events: BalanceEvent[];
  computed_recommendation: ComputedRecommendation | null;
  include_recommendation: boolean;
}

// Users
export const getUsers = () => authFetch('/users');

// Projects
export const getProjects = (): Promise<ProjectWithRole[]> => authFetch('/projects');

export const getProject = (id: number): Promise<Project> => authFetch(`/projects/${id}`);

export const createProject = (data: { name: string; description?: string }): Promise<Project> =>
  authFetch('/projects', {
    method: 'POST',
    body: JSON.stringify(data)
  });

export const updateProject = (
  id: number,
  data: { name?: string; description?: string }
): Promise<Project> =>
  authFetch(`/projects/${id}`, {
    method: 'PUT',
    body: JSON.stringify(data)
  });

export const deleteProject = (id: number) => authFetch(`/projects/${id}`, { method: 'DELETE' });

export const regenerateInviteCode = (id: number): Promise<Project> =>
  authFetch(`/projects/${id}/regenerate-invite`, { method: 'POST' });

export const joinProject = (
  inviteCode: string,
  participantToken?: string
): Promise<JoinProjectResponse> =>
  authFetch('/projects/join', {
    method: 'POST',
    body: JSON.stringify({ invite_code: inviteCode, participant_token: participantToken })
  });

export const updateProjectSettings = (
  id: number,
  settings: {
    invites_enabled?: boolean;
    require_approval?: boolean;
    pending_member_access?: 'none' | 'read_only' | 'auto_approve';
  }
): Promise<Project> =>
  authFetch(`/projects/${id}/settings`, {
    method: 'PUT',
    body: JSON.stringify(settings)
  });

// Participants
export const getParticipants = (projectId: number): Promise<Participant[]> =>
  authFetch(`/projects/${projectId}/participants`);

export const getParticipant = (projectId: number, participantId: number): Promise<Participant> =>
  authFetch(`/projects/${projectId}/participants/${participantId}`);

export const createParticipant = (
  projectId: number,
  data: { name: string; default_weight?: number; account_type?: 'user' | 'pool' }
): Promise<Participant> =>
  authFetch(`/projects/${projectId}/participants`, {
    method: 'POST',
    body: JSON.stringify(data)
  });

export const updateParticipant = (
  projectId: number,
  participantId: number,
  data: { name?: string; default_weight?: number; account_type?: 'user' | 'pool' }
): Promise<Participant> =>
  authFetch(`/projects/${projectId}/participants/${participantId}`, {
    method: 'PUT',
    body: JSON.stringify(data)
  });

export const deleteParticipant = (projectId: number, participantId: number) =>
  authFetch(`/projects/${projectId}/participants/${participantId}`, { method: 'DELETE' });

export const claimParticipant = (projectId: number, participantId: number): Promise<Participant> =>
  authFetch(`/projects/${projectId}/participants/${participantId}/claim`, { method: 'POST' });

export const updatePoolWarningSettings = (
  projectId: number,
  participantId: number,
  data: {
    warning_horizon_account?: string; // empty string = disable
    warning_horizon_users?: string; // empty string = disable
  }
): Promise<Participant> =>
  authFetch(`/projects/${projectId}/participants/${participantId}/warning-settings`, {
    method: 'PATCH',
    body: JSON.stringify(data)
  });

// Participant Invites
export const createParticipantInvite = (
  projectId: number,
  participantId: number
): Promise<ParticipantInvite> =>
  authFetch(`/projects/${projectId}/participants/${participantId}/invite`, { method: 'POST' });

export const getParticipantInvite = (
  projectId: number,
  participantId: number
): Promise<ParticipantInvite | null> =>
  authFetch(`/projects/${projectId}/participants/${participantId}/invite`, { nullOn404: true });

export const revokeParticipantInvite = (
  projectId: number,
  participantId: number
): Promise<{ revoked: boolean }> =>
  authFetch(`/projects/${projectId}/participants/${participantId}/invite`, { method: 'DELETE' });

// Members
export const getMembers = (projectId: number): Promise<ProjectMember[]> =>
  authFetch(`/projects/${projectId}/members`);

export const getMember = (projectId: number, userId: number): Promise<ProjectMember> =>
  authFetch(`/projects/${projectId}/members/${userId}`);

export const updateMemberRole = (
  projectId: number,
  userId: number,
  role: string
): Promise<ProjectMember> =>
  authFetch(`/projects/${projectId}/members/${userId}`, {
    method: 'PUT',
    body: JSON.stringify({ role })
  });

export const removeMember = (projectId: number, userId: number) =>
  authFetch(`/projects/${projectId}/members/${userId}`, { method: 'DELETE' });

export const setMemberParticipant = (
  projectId: number,
  userId: number,
  participantId: number | null
): Promise<ProjectMember> =>
  authFetch(`/projects/${projectId}/members/${userId}/participant`, {
    method: 'PUT',
    body: JSON.stringify({ participant_id: participantId })
  });

// Member Approval
export const getPendingMembers = (projectId: number): Promise<ProjectMember[]> =>
  authFetch(`/projects/${projectId}/members/pending`);

export const approveMember = (projectId: number, userId: number): Promise<ProjectMember> =>
  authFetch(`/projects/${projectId}/members/${userId}/approve`, { method: 'PUT' });

export const rejectMember = (projectId: number, userId: number): Promise<{ rejected: boolean }> =>
  authFetch(`/projects/${projectId}/members/${userId}/reject`, { method: 'PUT' });

// Payments
export interface CreatePaymentInput {
  payer_id: number | null;
  amount: number;
  description: string;
  payment_date?: string;
  contributions: Array<{ participant_id: number; weight: number }>;
  // Receipt image (Base64 encoded)
  receipt_image?: string;
  // Recurrence fields
  is_recurring?: boolean;
  recurrence_type?: string;
  recurrence_interval?: number;
  recurrence_times_per?: number;
  recurrence_end_date?: string;
  // Enhanced recurrence patterns (JSON strings)
  recurrence_weekdays?: string;
  recurrence_monthdays?: string;
  recurrence_months?: string;
  // Internal transfer: recipient account (null = external expense)
  receiver_account_id?: number | null;
  // Payment finalization status: true (default) = final, false = draft
  is_final?: boolean;
  // Dual ledger flags for pool expected minimum
  // affects_balance: Transaction affects actual pool balance (default: true)
  affects_balance?: boolean;
  // affects_payer_expectation: When payer is a pool and true, reduces payer's expected minimum
  affects_payer_expectation?: boolean;
  // affects_receiver_expectation: When receiver is a pool and true, increases receiver's expected minimum
  affects_receiver_expectation?: boolean;
}

export const getPayments = (projectId: number): Promise<PaymentWithContributions[]> =>
  authFetch(`/projects/${projectId}/payments`);

export const getPayment = (
  projectId: number,
  paymentId: number
): Promise<PaymentWithContributions> => authFetch(`/projects/${projectId}/payments/${paymentId}`);

export const createPayment = (
  projectId: number,
  payload: CreatePaymentInput
): Promise<PaymentWithContributions> =>
  authFetch(`/projects/${projectId}/payments`, {
    method: 'POST',
    body: JSON.stringify(payload)
  });

export const updatePayment = (
  projectId: number,
  paymentId: number,
  payload: CreatePaymentInput
): Promise<PaymentWithContributions> =>
  authFetch(`/projects/${projectId}/payments/${paymentId}`, {
    method: 'PUT',
    body: JSON.stringify(payload)
  });

export const deletePayment = (projectId: number, paymentId: number) =>
  authFetch(`/projects/${projectId}/payments/${paymentId}`, { method: 'DELETE' });

// Debts
export const getDebts = (
  projectId: number,
  targetDate?: string,
  includeDrafts?: boolean
): Promise<DebtSummary> => {
  const params = new URLSearchParams();
  if (targetDate) params.set('date', targetDate);
  if (includeDrafts !== undefined) params.set('include_drafts', includeDrafts.toString());
  const queryString = params.toString();
  return authFetch(`/projects/${projectId}/debts${queryString ? '?' + queryString : ''}`);
};

export interface CashflowProjectionParams {
  horizonMonths?: number;
  recommendationMode?: string;
  frequencyType?: string;
  frequencyInterval?: number;
  consolidateMode?: boolean;
  // Recommendation configuration
  recPayerId?: number;
  recReceiverId?: number;
  recStartDate?: string;
  recEndDate?: string;
  includeRecommendation?: boolean;
}

export const getCashflowProjection = (
  projectId: number,
  opts: CashflowProjectionParams = {}
): Promise<CashflowProjection> => {
  const params = new URLSearchParams();
  if (opts.horizonMonths) params.set('horizon_months', opts.horizonMonths.toString());
  if (opts.recommendationMode) params.set('recommendation_mode', opts.recommendationMode);
  if (opts.frequencyType) params.set('frequency_type', opts.frequencyType);
  if (opts.frequencyInterval) params.set('frequency_interval', opts.frequencyInterval.toString());
  if (opts.consolidateMode) params.set('consolidate_mode', 'true');
  if (opts.recPayerId) params.set('rec_payer_id', opts.recPayerId.toString());
  if (opts.recReceiverId) params.set('rec_receiver_id', opts.recReceiverId.toString());
  if (opts.recStartDate) params.set('rec_start_date', opts.recStartDate);
  if (opts.recEndDate) params.set('rec_end_date', opts.recEndDate);
  if (opts.includeRecommendation !== undefined)
    params.set('include_recommendation', opts.includeRecommendation.toString());
  const queryString = params.toString();
  return authFetch(`/projects/${projectId}/cashflow${queryString ? '?' + queryString : ''}`);
};

export interface ConsolidatePaymentsRequest {
  payer_id: number;
  pool_id: number;
  amount: number;
  description: string;
  payment_date: string;
  recurrence_type: string;
  recurrence_interval: number;
  payment_ids_to_delete: number[];
}

export interface ConsolidatePaymentsResponse {
  deleted_count: number;
  new_payment_id: number;
}

export const consolidatePayments = (
  projectId: number,
  request: ConsolidatePaymentsRequest
): Promise<ConsolidatePaymentsResponse> =>
  authFetch(`/projects/${projectId}/cashflow/consolidate`, {
    method: 'POST',
    body: JSON.stringify(request)
  });

// History
export interface HistoryEntry {
  id: number;
  created_at: string;
  correlation_id: string;
  actor_user_id: number | null;
  actor_name: string | null;
  project_id: number | null;
  entity_type: string;
  entity_id: number | null;
  action: 'CREATE' | 'UPDATE' | 'DELETE' | 'UNDO';
  payload_before: unknown | null;
  payload_after: unknown | null;
  reason: string | null;
  undoes_history_id: number | null;
  is_undone: boolean;
}

export interface HistoryQuery {
  limit?: number;
  offset?: number;
  entity_type?: string;
}

export interface ChainVerification {
  is_valid: boolean;
  total_entries: number;
  first_broken_id: number | null;
  message: string;
}

export const getProjectHistory = (
  projectId: number,
  query?: HistoryQuery
): Promise<HistoryEntry[]> => {
  const params = new URLSearchParams();
  if (query?.limit) params.set('limit', query.limit.toString());
  if (query?.offset) params.set('offset', query.offset.toString());
  if (query?.entity_type) params.set('entity_type', query.entity_type);
  const queryString = params.toString();
  return authFetch(`/projects/${projectId}/history${queryString ? '?' + queryString : ''}`);
};

export const getEntityHistory = (
  projectId: number,
  entityType: string,
  entityId: number
): Promise<HistoryEntry[]> => authFetch(`/projects/${projectId}/history/${entityType}/${entityId}`);

export const undoHistoryEntry = (
  projectId: number,
  historyId: number,
  reason?: string
): Promise<HistoryEntry> =>
  authFetch(`/projects/${projectId}/history/${historyId}/undo`, {
    method: 'POST',
    body: JSON.stringify({ reason })
  });

export const verifyHistoryChain = (projectId: number): Promise<ChainVerification> =>
  authFetch(`/projects/${projectId}/history/verify`);

// ========================================
// Approval endpoints
// ========================================

export interface ApprovalVote {
  id: number;
  approval_id: number;
  voter_id: number;
  voter_username?: string;
  voter_display_name?: string;
  vote: string; // 'approve' or 'reject'
  reason?: string;
  voted_at: string;
}

export interface ProjectApproval {
  id: number;
  user_id: number;
  project_id: number;
  event_type: string;
  event_metadata?: string;
  status: string; // 'pending', 'approved', 'rejected'
  created_at: string;
  resolved_at?: string;
  // Additional fields from ApprovalWithDetails
  project_name?: string;
  username?: string;
  display_name?: string;
  votes?: ApprovalVote[];
  vote_count?: number;
  required_votes?: number;
  can_self_approve?: boolean;
}

export interface CastVoteRequest {
  vote: 'approve' | 'reject';
  reason?: string;
}

export const getMyPendingApprovals = (): Promise<ProjectApproval[]> =>
  authFetch('/approvals/my-pending');

export const getActionableApprovals = (): Promise<ProjectApproval[]> =>
  authFetch('/approvals/actionable');

export const getApproval = (id: number): Promise<ProjectApproval> => authFetch(`/approvals/${id}`);

export const castVote = (
  approvalId: number,
  vote: 'approve' | 'reject',
  reason?: string
): Promise<ProjectApproval> =>
  authFetch(`/approvals/${approvalId}/vote`, {
    method: 'POST',
    body: JSON.stringify({ vote, reason } as CastVoteRequest)
  });

// ========================================
// Trusted Users (Password Recovery)
// ========================================

export interface TrustedUser {
  id: number;
  trusted_user_id: number;
  username: string;
  display_name: string | null;
  created_at: string;
}

export interface RecoveryStatus {
  recoverable: boolean;
  trusted_user_count: number;
  required_count: number;
}

export const getTrustedUsers = (): Promise<TrustedUser[]> => authFetch('/users/me/trusted-users');

export const addTrustedUser = (username: string): Promise<TrustedUser> =>
  authFetch('/users/me/trusted-users', {
    method: 'POST',
    body: JSON.stringify({ username })
  });

export const removeTrustedUser = (id: number): Promise<{ message: string }> =>
  authFetch(`/users/me/trusted-users/${id}`, { method: 'DELETE' });

export const getRecoveryStatus = (): Promise<RecoveryStatus> =>
  authFetch('/users/me/recovery-status');

// ========================================
// Recovery Intent API (Password Recovery)
// ========================================

export interface InitiateRecoveryResponse {
  message: string;
  token: string;
  expires_at: string;
}

export interface RecoveryIntentStatus {
  status: string;
  approvals_count: number;
  required_approvals: number;
  expires_at: string;
  is_expired: boolean;
}

export interface PendingRecoveryIntent {
  id: number;
  user_id: number;
  username: string;
  display_name: string | null;
  token: string;
  status: string;
  created_at: string;
  expires_at: string;
  approvals_count: number;
  rejections_count: number;
  required_approvals: number;
  is_blocked: boolean;
  user_has_voted: boolean;
  user_vote: string | null;
}

export interface RecoveryVoteResponse {
  message: string;
  vote: string;
  approvals_count: number;
  rejections_count: number;
  required_approvals: number;
  is_blocked: boolean;
  status: string;
}

// Public API for recovery (no auth required)
async function publicFetch<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(opts.headers as Record<string, string>)
  };

  let res: Response;
  try {
    res = await fetch(`${BASE}${path}`, { ...opts, headers });
  } catch (e) {
    // Network error - backend is down or unreachable
    if (e instanceof TypeError && e.message === 'Failed to fetch') {
      throw new ApiRequestError('NETWORK_ERROR', 'NETWORK_ERROR', 0);
    }
    throw e;
  }

  if (!res.ok) {
    const text = await res.text();
    try {
      const data: ApiError = JSON.parse(text);
      throw new ApiRequestError(data.code || 'UNKNOWN', data.error || text, res.status);
    } catch (e) {
      if (e instanceof ApiRequestError) throw e;
      throw new ApiRequestError('UNKNOWN', text || res.statusText, res.status);
    }
  }

  return res.json();
}

// Initiate password recovery (no auth)
export const initiateRecovery = (username: string): Promise<InitiateRecoveryResponse> =>
  publicFetch('/recovery/initiate', {
    method: 'POST',
    body: JSON.stringify({ username })
  });

// Get recovery status by token (no auth)
export const getRecoveryIntentStatus = (token: string): Promise<RecoveryIntentStatus> =>
  publicFetch(`/recovery/${token}/status`);

// Reset password after recovery approved (no auth)
export const resetPasswordWithToken = (
  token: string,
  newPassword: string
): Promise<{ message: string }> =>
  publicFetch(`/recovery/${token}/reset`, {
    method: 'POST',
    body: JSON.stringify({ new_password: newPassword })
  });

// Get pending recovery requests for trusted users to approve (requires auth)
export const getPendingRecoveries = (): Promise<PendingRecoveryIntent[]> =>
  authFetch('/recovery/pending');

// Vote on a recovery intent (requires auth)
export const voteOnRecovery = (
  token: string,
  vote: 'approve' | 'reject'
): Promise<RecoveryVoteResponse> =>
  authFetch(`/recovery/${token}/vote`, {
    method: 'POST',
    body: JSON.stringify({ vote })
  });
