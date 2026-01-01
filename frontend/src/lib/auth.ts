import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';

export interface UserPreferences {
  date_format: string;
  decimal_separator: string;
  currency_symbol: string;
  currency_symbol_position: string;
}

export interface User {
  id: number;
  username: string;
  display_name: string | null;
  user_state: string;
  preferences: UserPreferences;
}

interface AuthState {
  token: string | null;
  user: User | null;
  loading: boolean;
}

const TOKEN_KEY = 'bonscompte_token';

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthState>({
    token: null,
    user: null,
    loading: true
  });

  return {
    subscribe,

    init() {
      if (!browser) {
        update((s) => ({ ...s, loading: false }));
        return;
      }

      const token = localStorage.getItem(TOKEN_KEY);
      if (token) {
        try {
          // Decode JWT payload (base64)
          const payload = JSON.parse(atob(token.split('.')[1]));

          // Check if expired
          if (payload.exp * 1000 > Date.now()) {
            // Note: When loading from JWT, we don't have full user data
            // Preferences will be loaded from localStorage by the preferences store
            set({
              token,
              user: {
                id: payload.sub,
                username: payload.username,
                display_name: null,
                user_state: 'active', // Default, will be updated when fetching user data
                preferences: {
                  date_format: 'mdy',
                  decimal_separator: '.',
                  currency_symbol: '$',
                  currency_symbol_position: 'before'
                }
              },
              loading: false
            });
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
    },

    updateUser(userData: Partial<User>) {
      update((s) => ({
        ...s,
        user: s.user ? { ...s.user, ...userData } : null
      }));
    }
  };
}

export const auth = createAuthStore();
export const isAuthenticated = derived(auth, ($auth) => !!$auth.token && !$auth.loading);
export const isLoading = derived(auth, ($auth) => $auth.loading);
