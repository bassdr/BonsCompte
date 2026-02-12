import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';

export interface UserPreferences {
  date_format: string;
  decimal_separator: string;
  currency_symbol: string;
  currency_symbol_position: string;
  // Budget preferences
  budget_pay_frequency: 'weekly' | 'biweekly' | 'semimonthly' | 'monthly';
  budget_hours_per_week: number;
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

    async init() {
      if (!browser) {
        update((s) => ({ ...s, loading: false }));
        return;
      }

      const token = localStorage.getItem(TOKEN_KEY);
      if (token) {
        try {
          // Decode JWT payload (base64)
          const payload = JSON.parse(atob(token.split('.')[1]));

          // Check if token is not expired
          if (payload.exp * 1000 > Date.now()) {
            // Token is valid - set it first to keep user logged in
            set({
              token,
              user: {
                id: payload.sub,
                username: payload.username,
                display_name: null,
                user_state: 'active',
                preferences: {
                  date_format: 'mdy',
                  decimal_separator: '.',
                  currency_symbol: '$',
                  currency_symbol_position: 'before',
                  budget_pay_frequency: 'biweekly',
                  budget_hours_per_week: 40
                }
              },
              loading: true // Still loading full user data
            });

            // Now try to load full user data from backend
            try {
              const { getCurrentUser } = await import('./api');
              const userData = await getCurrentUser();
              set({
                token,
                user: userData as User,
                loading: false
              });
              return;
            } catch {
              // If backend call fails, keep the user logged in with defaults
              set({
                token,
                user: {
                  id: payload.sub,
                  username: payload.username,
                  display_name: null,
                  user_state: 'active',
                  preferences: {
                    date_format: 'mdy',
                    decimal_separator: '.',
                    currency_symbol: '$',
                    currency_symbol_position: 'before',
                    budget_pay_frequency: 'biweekly',
                    budget_hours_per_week: 40
                  }
                },
                loading: false
              });
              return;
            }
          }
        } catch {
          // Invalid JWT format - token is corrupted
        }
        // Token is expired or invalid - remove it
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
