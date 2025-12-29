import { register, init, getLocaleFromNavigator, locale, _ } from 'svelte-i18n';
import { derived } from 'svelte/store';
import { browser } from '$app/environment';

const STORAGE_KEY = 'bonscompte_language';
const SUPPORTED_LANGUAGES = ['en', 'fr'];
const DEFAULT_LANGUAGE = 'en';

// Register translations (lazy loaded)
register('en', () => import('./translations/en.json'));
register('fr', () => import('./translations/fr.json'));

/**
 * Determine initial locale based on precedence:
 * 1. User preference from backend (passed as parameter)
 * 2. localStorage
 * 3. Browser language
 * 4. Default: 'en'
 */
export function getInitialLocale(userLanguage?: string | null): string {
	// 1. User preference from backend (highest priority)
	if (userLanguage && SUPPORTED_LANGUAGES.includes(userLanguage)) {
		return userLanguage;
	}

	if (browser) {
		// 2. localStorage
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored && SUPPORTED_LANGUAGES.includes(stored)) {
			return stored;
		}

		// 3. Browser language
		const browserLang = getLocaleFromNavigator()?.split('-')[0];
		if (browserLang && SUPPORTED_LANGUAGES.includes(browserLang)) {
			return browserLang;
		}
	}

	// 4. Default
	return DEFAULT_LANGUAGE;
}

/**
 * Initialize svelte-i18n with the determined locale
 */
export function setupI18n(initialLocale: string = DEFAULT_LANGUAGE) {
	init({
		fallbackLocale: DEFAULT_LANGUAGE,
		initialLocale
	});
}

/**
 * Set locale and persist to localStorage
 */
export function setLocale(lang: string) {
	if (!SUPPORTED_LANGUAGES.includes(lang)) {
		console.warn(`Unsupported language: ${lang}`);
		return;
	}

	locale.set(lang);

	if (browser) {
		localStorage.setItem(STORAGE_KEY, lang);
	}
}

/**
 * Get current locale value synchronously
 */
export function getCurrentLocale(): string {
	let current = DEFAULT_LANGUAGE;
	const unsubscribe = locale.subscribe((value) => {
		current = value || DEFAULT_LANGUAGE;
	});
	unsubscribe();
	return current;
}

// Export for components
export { _, locale };

// Supported languages for UI dropdowns
export const supportedLanguages = [
	{ code: 'en', name: 'English' },
	{ code: 'fr', name: 'Français' }
];

// Derived store to check if i18n is ready
export const isLocaleLoaded = derived(locale, ($locale) => typeof $locale === 'string');
