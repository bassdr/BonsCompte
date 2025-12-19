import { writable } from 'svelte/store';

export interface UserPreferences {
	language: string;
	dateFormat: string;
	currencyPosition: 'before' | 'after';
	decimalSeparator: '.' | ',';
}

const defaultPreferences: UserPreferences = {
	language: 'en',
	dateFormat: 'YYYY-MM-DD',
	currencyPosition: 'before',
	decimalSeparator: '.',
};

export const preferences = writable<UserPreferences>(defaultPreferences);

/**
 * Load user preferences from a user object (API response uses snake_case)
 * Called when user logs in or preferences are updated
 */
export function loadPreferencesFromUser(user: {
	language?: string;
	date_format?: string;
	currency_position?: string;
	decimal_separator?: string;
}) {
	preferences.set({
		language: user.language || 'en',
		dateFormat: user.date_format || 'YYYY-MM-DD',
		currencyPosition: (user.currency_position as 'before' | 'after') || 'before',
		decimalSeparator: (user.decimal_separator as '.' | ',') || '.',
	});
}

/**
 * Reset preferences to defaults
 */
export function resetPreferences() {
	preferences.set(defaultPreferences);
}
