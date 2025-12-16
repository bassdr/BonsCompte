import { auth, type User } from './auth';
import { browser } from '$app/environment';

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

// Types
export interface Project {
    id: number;
    name: string;
    description: string | null;
    invite_code: string | null;
    created_by: number;
    created_at: string;
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
export const getUsers = () => authFetch("/api/users");

// Projects
export const getProjects = (): Promise<ProjectWithRole[]> =>
    authFetch("/api/projects");

export const getProject = (id: number): Promise<Project> =>
    authFetch(`/api/projects/${id}`);

export const createProject = (data: { name: string; description?: string }): Promise<Project> =>
    authFetch("/api/projects", {
        method: "POST",
        body: JSON.stringify(data)
    });

export const updateProject = (id: number, data: { name?: string; description?: string }): Promise<Project> =>
    authFetch(`/api/projects/${id}`, {
        method: "PUT",
        body: JSON.stringify(data)
    });

export const deleteProject = (id: number) =>
    authFetch(`/api/projects/${id}`, { method: "DELETE" });

export const regenerateInviteCode = (id: number): Promise<Project> =>
    authFetch(`/api/projects/${id}/regenerate-invite`, { method: "POST" });

export const joinProject = (inviteCode: string): Promise<Project> =>
    authFetch("/api/projects/join", {
        method: "POST",
        body: JSON.stringify({ invite_code: inviteCode })
    });

// Participants
export const getParticipants = (projectId: number): Promise<Participant[]> =>
    authFetch(`/api/projects/${projectId}/participants`);

export const getParticipant = (projectId: number, participantId: number): Promise<Participant> =>
    authFetch(`/api/projects/${projectId}/participants/${participantId}`);

export const createParticipant = (projectId: number, data: { name: string; default_weight?: number }): Promise<Participant> =>
    authFetch(`/api/projects/${projectId}/participants`, {
        method: "POST",
        body: JSON.stringify(data)
    });

export const updateParticipant = (projectId: number, participantId: number, data: { name?: string; default_weight?: number }): Promise<Participant> =>
    authFetch(`/api/projects/${projectId}/participants/${participantId}`, {
        method: "PUT",
        body: JSON.stringify(data)
    });

export const deleteParticipant = (projectId: number, participantId: number) =>
    authFetch(`/api/projects/${projectId}/participants/${participantId}`, { method: "DELETE" });

export const claimParticipant = (projectId: number, participantId: number): Promise<Participant> =>
    authFetch(`/api/projects/${projectId}/participants/${participantId}/claim`, { method: "POST" });

// Members
export const getMembers = (projectId: number): Promise<ProjectMember[]> =>
    authFetch(`/api/projects/${projectId}/members`);

export const getMember = (projectId: number, userId: number): Promise<ProjectMember> =>
    authFetch(`/api/projects/${projectId}/members/${userId}`);

export const updateMemberRole = (projectId: number, userId: number, role: string): Promise<ProjectMember> =>
    authFetch(`/api/projects/${projectId}/members/${userId}`, {
        method: "PUT",
        body: JSON.stringify({ role })
    });

export const removeMember = (projectId: number, userId: number) =>
    authFetch(`/api/projects/${projectId}/members/${userId}`, { method: "DELETE" });

export const setMemberParticipant = (projectId: number, userId: number, participantId: number | null): Promise<ProjectMember> =>
    authFetch(`/api/projects/${projectId}/members/${userId}/participant`, {
        method: "PUT",
        body: JSON.stringify({ participant_id: participantId })
    });

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
    authFetch(`/api/projects/${projectId}/payments`);

export const getPayment = (projectId: number, paymentId: number): Promise<PaymentWithContributions> =>
    authFetch(`/api/projects/${projectId}/payments/${paymentId}`);

export const createPayment = (projectId: number, payload: CreatePaymentInput): Promise<PaymentWithContributions> =>
    authFetch(`/api/projects/${projectId}/payments`, {
        method: "POST",
        body: JSON.stringify(payload)
    });

export const updatePayment = (projectId: number, paymentId: number, payload: CreatePaymentInput): Promise<PaymentWithContributions> =>
    authFetch(`/api/projects/${projectId}/payments/${paymentId}`, {
        method: "PUT",
        body: JSON.stringify(payload)
    });

export const deletePayment = (projectId: number, paymentId: number) =>
    authFetch(`/api/projects/${projectId}/payments/${paymentId}`, { method: "DELETE" });

// Debts
export const getDebts = (projectId: number, targetDate?: string): Promise<DebtSummary> => {
    const params = targetDate ? `?date=${targetDate}` : '';
    return authFetch(`/api/projects/${projectId}/debts${params}`);
};
