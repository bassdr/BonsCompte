import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';
import { loadPreferencesFromUser } from '$lib/stores/preferences';
import { setLocale } from '$lib/i18n';

export interface User {
    id: number;
    username: string;
    display_name: string | null;
    language: string;
    date_format: string;
    currency_position: string;
    decimal_separator: string;
}

interface AuthState {
    token: string | null;
    user: User | null;
    loading: boolean;
}

const TOKEN_KEY = 'bonscompte_token';
const BASE = import.meta.env.VITE_API_BASE ?? "http://localhost:8000";

function createAuthStore() {
    const { subscribe, set, update } = writable<AuthState>({
        token: null,
        user: null,
        loading: true
    });

    return {
        subscribe,

        async init() {
            if (!browser) {
                update(s => ({ ...s, loading: false }));
                return;
            }

            const token = localStorage.getItem(TOKEN_KEY);
            if (token) {
                try {
                    // Decode JWT payload (base64)
                    const payload = JSON.parse(atob(token.split('.')[1]));

                    // Check if expired
                    if (payload.exp * 1000 > Date.now()) {
                        // Fetch full user data from API
                        try {
                            const res = await fetch(`${BASE}/users/me`, {
                                headers: { "Authorization": `Bearer ${token}` }
                            });
                            if (res.ok) {
                                const user: User = await res.json();
                                loadPreferencesFromUser(user);
                                setLocale(user.language);
                                set({ token, user, loading: false });
                                return;
                            }
                        } catch {
                            // Fall back to token data if fetch fails
                        }

                        // Fallback: use basic info from token with default preferences
                        const user: User = {
                            id: payload.sub,
                            username: payload.username,
                            display_name: null,
                            language: 'en',
                            date_format: 'YYYY-MM-DD',
                            currency_position: 'before',
                            decimal_separator: '.'
                        };
                        loadPreferencesFromUser(user);
                        setLocale(user.language);
                        set({ token, user, loading: false });
                        return;
                    }
                } catch {
                    // Invalid token
                }
                localStorage.removeItem(TOKEN_KEY);
            }

            set({ token: null, user: null, loading: false });
        },

        setAuth(token: string, user: User) {
            if (browser) {
                localStorage.setItem(TOKEN_KEY, token);
            }
            // Load user preferences and update i18n locale
            loadPreferencesFromUser(user);
            setLocale(user.language);
            set({ token, user, loading: false });
        },

        logout() {
            if (browser) {
                localStorage.removeItem(TOKEN_KEY);
            }
            set({ token: null, user: null, loading: false });
        },

        getToken(): string | null {
            return get({ subscribe }).token;
        }
    };
}

export const auth = createAuthStore();
export const isAuthenticated = derived(auth, $auth => !!$auth.token && !$auth.loading);
export const isLoading = derived(auth, $auth => $auth.loading);
