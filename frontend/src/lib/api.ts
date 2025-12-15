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

// Protected endpoints
export const getUsers = () => authFetch("/api/users");

export const getPayments = () => authFetch("/api/payments");

export const getPayment = (id: number) => authFetch(`/api/payments/${id}`);

export interface CreatePaymentInput {
    payer_id: number | null;
    receiver_id?: number | null;
    amount: number;
    description: string;
    payment_date?: string;
    contributions: Array<{ user_id: number; weight: number }>;
}

export const createPayment = (payload: CreatePaymentInput) =>
    authFetch("/api/payments", {
        method: "POST",
        body: JSON.stringify(payload)
    });

export const deletePayment = (id: number) =>
    authFetch(`/api/payments/${id}`, { method: "DELETE" });

export const getDebts = () => authFetch("/api/debts");

// Legacy compatibility (will be removed)
export const getMembers = getUsers;
export const getExpenses = getPayments;
export const postExpense = createPayment;

export async function uploadReceipt(_file: File) {
    // TODO: implement file upload endpoint
    return { url: "" };
}
