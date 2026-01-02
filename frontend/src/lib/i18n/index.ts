import {
	register,
	init,
	getLocaleFromNavigator,
	locale,
	_,
	isLoading as i18nIsLoading
} from 'svelte-i18n';
import { derived } from 'svelte/store';
import { browser } from '$app/environment';

const STORAGE_KEY = 'bonscompte_language';
const SUPPORTED_LANGUAGES = ['en', 'fr'];
const DEFAULT_LANGUAGE = 'en';

// Register translations (lazy loaded)
register('en', () => import('./translations/en.json'));
register('fr', () => import('./translations/fr.json'));

// Initialize immediately with default locale to prevent "locale not set" errors during SSR/hydration
// The actual locale will be set properly when setupI18n() is called
init({
	fallbackLocale: DEFAULT_LANGUAGE,
	initialLocale: DEFAULT_LANGUAGE
});

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
 * Set the locale after initialization (init is called at module load time)
 */
export function setupI18n(initialLocale: string = DEFAULT_LANGUAGE) {
	// init() was already called at module load time with DEFAULT_LANGUAGE
	// Just update the locale if different
	if (initialLocale !== DEFAULT_LANGUAGE) {
		locale.set(initialLocale);
	}
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

// Derived store to check if i18n is ready (locale is set AND translations are loaded)
export const isLocaleLoaded = derived(
	[locale, i18nIsLoading],
	([$locale, $isLoading]) => typeof $locale === 'string' && !$isLoading
);
