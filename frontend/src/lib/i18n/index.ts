import { register, init, getLocaleFromNavigator, locale as localeStore } from 'svelte-i18n';

// Register locale loaders
register('en', () => import('./locales/en.json'));
register('fr', () => import('./locales/fr.json'));

export function initI18n(userLanguage?: string) {
	const locale = userLanguage || getLocaleFromNavigator() || 'en';

	return init({
		fallbackLocale: 'en',
		initialLocale: locale,
	});
}

/**
 * Change the current locale at runtime
 * This updates the svelte-i18n locale store, triggering reactive updates
 */
export function setLocale(newLocale: string) {
	localeStore.set(newLocale);
}

// Re-export commonly used functions
export { _, locale, locales } from 'svelte-i18n';
