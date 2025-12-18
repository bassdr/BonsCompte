import { auth, type User } from './auth';
import { browser } from '$app/environment';

// Default to localhost:8000 for development
// Override with VITE_API_BASE in docker builds or production
const BASE = import.meta.env.VITE_API_BASE ?? "http://localhost:8000";

async function authFetch(path: string, opts: RequestInit = {}) {
    const token = auth.getToken();

    const headers: HeadersInit = {
        "Content-Type": "application/json",
        ...(token && { "Authorization": `Bearer ${token}` }),
        ...opts.headers as Record<string, string>
    };

    const res = await fetch(`${BASE}${path}`, { ...opts, headers });

    if (res.status === 401) {
        auth.logout();
        if (browser) {
            window.location.href = '/login';
        }
        throw new Error('Session expired');
    }

    if (!res.ok) {
        const text = await res.text();
        throw new Error(`${res.status}: ${text}`);
    }

    return res.json();
}

// Auth endpoints (public)
export interface AuthResponse {
    token: string;
    user: User;
}

export async function login(username: string, password: string): Promise<AuthResponse> {
    const res = await fetch(`${BASE}/auth/login`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password })
    });

    if (!res.ok) {
        const data = await res.json().catch(() => ({ error: 'Login failed' }));
        throw new Error(data.error || 'Login failed');
    }

    return res.json();
}

export async function register(
    username: string,
    password: string,
    display_name?: string
): Promise<AuthResponse> {
    const res = await fetch(`${BASE}/auth/register`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password, display_name })
    });

    if (!res.ok) {
        const data = await res.json().catch(() => ({ error: 'Registration failed' }));
        throw new Error(data.error || 'Registration failed');
    }

    return res.json();
}

// User management
export const changePassword = (currentPassword: string, newPassword: string): Promise<{ message: string }> =>
    authFetch("/users/me/password", {
        method: "PUT",
        body: JSON.stringify({ current_password: currentPassword, new_password: newPassword })
    });

export const deleteAccount = (password: string): Promise<DeleteAccountResponse> =>
    authFetch("/users/me", {
        method: "DELETE",
        body: JSON.stringify({ password })
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
}

export interface ProjectWithRole extends Project {
    role: string;
}

export interface Participant {
    id: number;
    project_id: number;
    name: string;
    user_id: number | null;
    default_weight: number;
    created_at: string;
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
    status: 'pending' | 'active' | 'rejected';
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
    recurrence_type: string | null;  // 'daily' | 'weekly' | 'monthly' | 'yearly'
    recurrence_interval: number | null;
    recurrence_times_per: number | null;
    recurrence_end_date: string | null;
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
}

export interface DebtSummary {
    balances: ParticipantBalance[];
    settlements: Debt[];
    target_date: string;
    occurrences: PaymentOccurrence[];
}

// Users
export const getUsers = () => authFetch("/users");

// Projects
export const getProjects = (): Promise<ProjectWithRole[]> =>
    authFetch("/projects");

export const getProject = (id: number): Promise<Project> =>
    authFetch(`/projects/${id}`);

export const createProject = (data: { name: string; description?: string }): Promise<Project> =>
    authFetch("/projects", {
        method: "POST",
        body: JSON.stringify(data)
    });

export const updateProject = (id: number, data: { name?: string; description?: string }): Promise<Project> =>
    authFetch(`/projects/${id}`, {
        method: "PUT",
        body: JSON.stringify(data)
    });

export const deleteProject = (id: number) =>
    authFetch(`/projects/${id}`, { method: "DELETE" });

export const regenerateInviteCode = (id: number): Promise<Project> =>
    authFetch(`/projects/${id}/regenerate-invite`, { method: "POST" });

export const joinProject = (inviteCode: string, participantToken?: string): Promise<JoinProjectResponse> =>
    authFetch("/projects/join", {
        method: "POST",
        body: JSON.stringify({ invite_code: inviteCode, participant_token: participantToken })
    });

export const updateProjectSettings = (id: number, settings: { invites_enabled?: boolean; require_approval?: boolean }): Promise<Project> =>
    authFetch(`/projects/${id}/settings`, {
        method: "PUT",
        body: JSON.stringify(settings)
    });

// Participants
export const getParticipants = (projectId: number): Promise<Participant[]> =>
    authFetch(`/projects/${projectId}/participants`);

export const getParticipant = (projectId: number, participantId: number): Promise<Participant> =>
    authFetch(`/projects/${projectId}/participants/${participantId}`);

export const createParticipant = (projectId: number, data: { name: string; default_weight?: number }): Promise<Participant> =>
    authFetch(`/projects/${projectId}/participants`, {
        method: "POST",
        body: JSON.stringify(data)
    });

export const updateParticipant = (projectId: number, participantId: number, data: { name?: string; default_weight?: number }): Promise<Participant> =>
    authFetch(`/projects/${projectId}/participants/${participantId}`, {
        method: "PUT",
        body: JSON.stringify(data)
    });

export const deleteParticipant = (projectId: number, participantId: number) =>
    authFetch(`/projects/${projectId}/participants/${participantId}`, { method: "DELETE" });

export const claimParticipant = (projectId: number, participantId: number): Promise<Participant> =>
    authFetch(`/projects/${projectId}/participants/${participantId}/claim`, { method: "POST" });

// Participant Invites
export const createParticipantInvite = (projectId: number, participantId: number): Promise<ParticipantInvite> =>
    authFetch(`/projects/${projectId}/participants/${participantId}/invite`, { method: "POST" });

export const getParticipantInvite = (projectId: number, participantId: number): Promise<ParticipantInvite> =>
    authFetch(`/projects/${projectId}/participants/${participantId}/invite`);

export const revokeParticipantInvite = (projectId: number, participantId: number): Promise<{ revoked: boolean }> =>
    authFetch(`/projects/${projectId}/participants/${participantId}/invite`, { method: "DELETE" });

// Members
export const getMembers = (projectId: number): Promise<ProjectMember[]> =>
    authFetch(`/projects/${projectId}/members`);

export const getMember = (projectId: number, userId: number): Promise<ProjectMember> =>
    authFetch(`/projects/${projectId}/members/${userId}`);

export const updateMemberRole = (projectId: number, userId: number, role: string): Promise<ProjectMember> =>
    authFetch(`/projects/${projectId}/members/${userId}`, {
        method: "PUT",
        body: JSON.stringify({ role })
    });

export const removeMember = (projectId: number, userId: number) =>
    authFetch(`/projects/${projectId}/members/${userId}`, { method: "DELETE" });

export const setMemberParticipant = (projectId: number, userId: number, participantId: number | null): Promise<ProjectMember> =>
    authFetch(`/projects/${projectId}/members/${userId}/participant`, {
        method: "PUT",
        body: JSON.stringify({ participant_id: participantId })
    });

// Member Approval
export const getPendingMembers = (projectId: number): Promise<ProjectMember[]> =>
    authFetch(`/projects/${projectId}/members/pending`);

export const approveMember = (projectId: number, userId: number): Promise<ProjectMember> =>
    authFetch(`/projects/${projectId}/members/${userId}/approve`, { method: "PUT" });

export const rejectMember = (projectId: number, userId: number): Promise<{ rejected: boolean }> =>
    authFetch(`/projects/${projectId}/members/${userId}/reject`, { method: "PUT" });

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
}

export const getPayments = (projectId: number): Promise<PaymentWithContributions[]> =>
    authFetch(`/projects/${projectId}/payments`);

export const getPayment = (projectId: number, paymentId: number): Promise<PaymentWithContributions> =>
    authFetch(`/projects/${projectId}/payments/${paymentId}`);

export const createPayment = (projectId: number, payload: CreatePaymentInput): Promise<PaymentWithContributions> =>
    authFetch(`/projects/${projectId}/payments`, {
        method: "POST",
        body: JSON.stringify(payload)
    });

export const updatePayment = (projectId: number, paymentId: number, payload: CreatePaymentInput): Promise<PaymentWithContributions> =>
    authFetch(`/projects/${projectId}/payments/${paymentId}`, {
        method: "PUT",
        body: JSON.stringify(payload)
    });

export const deletePayment = (projectId: number, paymentId: number) =>
    authFetch(`/projects/${projectId}/payments/${paymentId}`, { method: "DELETE" });

// Debts
export const getDebts = (projectId: number, targetDate?: string): Promise<DebtSummary> => {
    const params = targetDate ? `?date=${targetDate}` : '';
    return authFetch(`/projects/${projectId}/debts${params}`);
};
