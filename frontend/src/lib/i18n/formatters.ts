import { get } from 'svelte/store';
import { preferences } from '$lib/stores/preferences';

/**
 * Format currency amount with user's preferred symbol position and decimal separator
 * Always uses $ symbol. Multi-currency not supported yet.
 * NOTE: In Svelte templates, this creates a reactive dependency on the preferences store
 * @example formatCurrency(123.45) => "$123.45" or "123.45$" or "123,45$"
 */
export function formatCurrency(amount: number): string {
	const prefs = get(preferences);
	let formatted = amount.toFixed(2);

	// Replace decimal separator if needed
	if (prefs.decimalSeparator === ',') {
		formatted = formatted.replace('.', ',');
	}

	// Place currency symbol based on position preference
	if (prefs.currencyPosition === 'before') {
		return `$${formatted}`;
	} else {
		return `${formatted}$`;
	}
}

/**
 * Format date string (YYYY-MM-DD) with user's preferred locale
 * Uses browser's Intl API for locale-aware formatting
 * @example formatDate("2025-12-18") => "12/18/2025" (en-US) or "18/12/2025" (fr-FR)
 */
export function formatDate(dateStr: string): string {
	try {
		const prefs = get(preferences);
		// Extract date part (handle ISO datetime strings)
		const datePart = dateStr.split('T')[0];
		const [year, month, day] = datePart.split('-').map(Number);

		// Create date in UTC to avoid timezone issues
		const date = new Date(year, month - 1, day);

		// Map language codes to locale strings
		const localeMap: Record<string, string> = {
			en: 'en-US',
			fr: 'fr-FR',
		};

		const locale = localeMap[prefs.language] || 'en-US';

		return new Intl.DateTimeFormat(locale, {
			year: 'numeric',
			month: '2-digit',
			day: '2-digit',
		}).format(date);
	} catch (e) {
		console.error('Error formatting date:', e);
		return dateStr;
	}
}

/**
 * Format date with long form (e.g., "Mon, Dec 18, 2025")
 * @example formatDateLong("2025-12-18") => "Thu, Dec 18, 2025" or "jeu. 18 déc. 2025"
 */
export function formatDateLong(dateStr: string): string {
	try {
		const prefs = get(preferences);
		// Extract date part (handle ISO datetime strings)
		const datePart = dateStr.split('T')[0];
		const [year, month, day] = datePart.split('-').map(Number);

		// Create date in UTC
		const date = new Date(year, month - 1, day);

		const localeMap: Record<string, string> = {
			en: 'en-US',
			fr: 'fr-FR',
		};

		const locale = localeMap[prefs.language] || 'en-US';

		return new Intl.DateTimeFormat(locale, {
			weekday: 'short',
			year: 'numeric',
			month: 'short',
			day: 'numeric',
		}).format(date);
	} catch (e) {
		console.error('Error formatting date long:', e);
		return dateStr;
	}
}

/**
 * Format month and year (e.g., "December 2025" or "décembre 2025")
 * @example formatMonthYear("2025-12") => "December 2025" or "décembre 2025"
 */
export function formatMonthYear(dateStr: string): string {
	try {
		const prefs = get(preferences);
		// Extract year-month part
		const parts = dateStr.split('-');
		const year = Number(parts[0]);
		const month = Number(parts[1]);

		// Create date in UTC
		const date = new Date(year, month - 1, 1);

		const localeMap: Record<string, string> = {
			en: 'en-US',
			fr: 'fr-FR',
		};

		const locale = localeMap[prefs.language] || 'en-US';

		return new Intl.DateTimeFormat(locale, {
			month: 'long',
			year: 'numeric',
		}).format(date);
	} catch (e) {
		console.error('Error formatting month year:', e);
		return dateStr;
	}
}
