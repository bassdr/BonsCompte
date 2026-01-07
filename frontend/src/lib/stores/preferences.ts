import { writable, get } from 'svelte/store';
import { browser } from '$app/environment';

export interface UserPreferences {
  date_format: string; // 'mdy' | 'dmy' | 'ymd' | 'iso'
  decimal_separator: string; // '.' | ','
  currency_symbol: string;
  currency_symbol_position: string; // 'before' | 'after'
}

const DEFAULTS: UserPreferences = {
  date_format: 'mdy',
  decimal_separator: '.',
  currency_symbol: '$',
  currency_symbol_position: 'before'
};

const STORAGE_KEY = 'bonscompte_preferences';

function loadFromStorage(): UserPreferences {
  if (!browser) return { ...DEFAULTS };

  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      return { ...DEFAULTS, ...JSON.parse(stored) };
    } catch {
      return { ...DEFAULTS };
    }
  }
  return { ...DEFAULTS };
}

function saveToStorage(prefs: UserPreferences) {
  if (browser) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  }
}

function createPreferencesStore() {
  const { subscribe, set, update } = writable<UserPreferences>(loadFromStorage());

  return {
    subscribe,

    /**
     * Initialize from backend user preferences (on login)
     * This takes precedence over localStorage
     */
    initFromUser(prefs: UserPreferences) {
      set(prefs);
      saveToStorage(prefs);
    },

    /**
     * Update a single preference
     */
    updatePreference<K extends keyof UserPreferences>(key: K, value: UserPreferences[K]) {
      update((current) => {
        const updated = { ...current, [key]: value };
        saveToStorage(updated);
        return updated;
      });
    },

    /**
     * Set all preferences at once
     */
    setAll(prefs: UserPreferences) {
      set(prefs);
      saveToStorage(prefs);
    },

    /**
     * Reset to defaults (on logout)
     */
    reset() {
      set({ ...DEFAULTS });
      if (browser) {
        localStorage.removeItem(STORAGE_KEY);
      }
    },

    /**
     * Get current preferences synchronously
     */
    get(): UserPreferences {
      return get({ subscribe });
    }
  };
}

export const preferences = createPreferencesStore();
