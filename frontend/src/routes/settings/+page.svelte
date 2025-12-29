<script lang="ts">
    import { goto } from '$app/navigation';
    import { changePassword, deleteAccount, updatePreferences, type DeleteAccountResponse } from '$lib/api';
    import { auth, type UserPreferences } from '$lib/auth';
    import { _ } from '$lib/i18n';
    import { preferences } from '$lib/stores/preferences';
    import { supportedLanguages, setLocale } from '$lib/i18n';

    // Password change state
    let currentPassword = $state('');
    let newPassword = $state('');
    let confirmPassword = $state('');
    let passwordError = $state('');
    let passwordSuccess = $state('');
    let changingPassword = $state(false);

    // Delete account state
    let deletePassword = $state('');
    let deleteError = $state('');
    let deleting = $state(false);
    let showDeleteConfirm = $state(false);
    let deleteResult = $state<DeleteAccountResponse | null>(null);

    // Preferences state
    let prefsLanguage = $state($preferences.language);
    let prefsDateFormat = $state($preferences.date_format);
    let prefsDecimalSeparator = $state($preferences.decimal_separator);
    let prefsCurrencySymbol = $state($preferences.currency_symbol);
    let prefsCurrencyPosition = $state($preferences.currency_symbol_position);
    let prefsSaving = $state(false);
    let prefsSuccess = $state('');
    let prefsError = $state('');

    // Sync preferences state when store changes
    $effect(() => {
        prefsLanguage = $preferences.language;
        prefsDateFormat = $preferences.date_format;
        prefsDecimalSeparator = $preferences.decimal_separator;
        prefsCurrencySymbol = $preferences.currency_symbol;
        prefsCurrencyPosition = $preferences.currency_symbol_position;
    });

    async function handlePreferenceSave() {
        prefsSaving = true;
        prefsSuccess = '';
        prefsError = '';

        try {
            const updatedPrefs = await updatePreferences({
                language: prefsLanguage,
                date_format: prefsDateFormat,
                decimal_separator: prefsDecimalSeparator,
                currency_symbol: prefsCurrencySymbol,
                currency_symbol_position: prefsCurrencyPosition
            });

            preferences.setAll(updatedPrefs);
            prefsSuccess = $_('settings.preferencesSaved');
        } catch (err) {
            prefsError = err instanceof Error ? err.message : 'Failed to save preferences';
        } finally {
            prefsSaving = false;
        }
    }

    function handleLanguageChange(e: Event) {
        const value = (e.target as HTMLSelectElement).value;
        prefsLanguage = value;
        // Immediately update locale for preview
        setLocale(value);
    }

    async function handleChangePassword(e: Event) {
        e.preventDefault();
        passwordError = '';
        passwordSuccess = '';

        if (newPassword !== confirmPassword) {
            passwordError = 'Passwords do not match';
            return;
        }

        if (newPassword.length < 6) {
            passwordError = 'New password must be at least 6 characters';
            return;
        }

        changingPassword = true;

        try {
            await changePassword(currentPassword, newPassword);
            passwordSuccess = 'Password changed successfully';
            currentPassword = '';
            newPassword = '';
            confirmPassword = '';
        } catch (err) {
            passwordError = err instanceof Error ? err.message : 'Failed to change password';
        } finally {
            changingPassword = false;
        }
    }

    async function handleDeleteAccount(e: Event) {
        e.preventDefault();
        deleteError = '';

        if (!deletePassword) {
            deleteError = 'Password is required';
            return;
        }

        deleting = true;

        try {
            deleteResult = await deleteAccount(deletePassword);
            // Log out and redirect
            auth.logout();
            goto('/login');
        } catch (err) {
            deleteError = err instanceof Error ? err.message : 'Failed to delete account';
        } finally {
            deleting = false;
        }
    }
</script>

<div class="settings-container">
    <h1>{$_('settings.title')}</h1>

    {#if $auth.user}
        <section class="section">
            <h2>{$_('settings.accountInfo')}</h2>
            <div class="info-grid">
                <div class="info-item">
                    <span class="label">{$_('auth.username')}</span>
                    <span class="value">{$auth.user?.username}</span>
                </div>
                {#if $auth.user?.display_name}
                    <div class="info-item">
                        <span class="label">{$_('auth.displayName')}</span>
                        <span class="value">{$auth.user?.display_name}</span>
                    </div>
                {/if}
            </div>
        </section>
    {/if}

    <section class="section">
        <h2>{$_('settings.preferences')}</h2>

        {#if prefsError}
            <div class="error">{prefsError}</div>
        {/if}
        {#if prefsSuccess}
            <div class="success">{prefsSuccess}</div>
        {/if}

        <div class="prefs-grid">
            <div class="field">
                <label for="pref-language">{$_('settings.language')}</label>
                <select id="pref-language" value={prefsLanguage} onchange={handleLanguageChange}>
                    {#each supportedLanguages as lang}
                        <option value={lang.code}>{lang.name}</option>
                    {/each}
                </select>
            </div>

            <div class="field">
                <label for="pref-dateformat">{$_('settings.dateFormat')}</label>
                <select id="pref-dateformat" bind:value={prefsDateFormat}>
                    <option value="mdy">{$_('settings.dateFormatMDY')}</option>
                    <option value="dmy">{$_('settings.dateFormatDMY')}</option>
                    <option value="ymd">{$_('settings.dateFormatYMD')}</option>
                    <option value="iso">{$_('settings.dateFormatISO')}</option>
                </select>
            </div>

            <div class="field">
                <label for="pref-decimal">{$_('settings.decimalSeparator')}</label>
                <select id="pref-decimal" bind:value={prefsDecimalSeparator}>
                    <option value=".">{$_('settings.decimalPeriod')}</option>
                    <option value=",">{$_('settings.decimalComma')}</option>
                </select>
            </div>

            <div class="field">
                <label for="pref-currency">{$_('settings.currencySymbol')}</label>
                <input
                    id="pref-currency"
                    type="text"
                    bind:value={prefsCurrencySymbol}
                    maxlength="5"
                    placeholder="$"
                />
            </div>

            <div class="field">
                <label for="pref-currpos">{$_('settings.currencyPosition')}</label>
                <select id="pref-currpos" bind:value={prefsCurrencyPosition}>
                    <option value="before">{$_('settings.currencyBefore')}</option>
                    <option value="after">{$_('settings.currencyAfter')}</option>
                </select>
            </div>
        </div>

        <button type="button" onclick={handlePreferenceSave} disabled={prefsSaving}>
            {prefsSaving ? $_('settings.savingPreferences') : $_('common.save')}
        </button>
    </section>

    <section class="section">
        <h2>{$_('settings.changePassword')}</h2>

        <form onsubmit={handleChangePassword}>
            {#if passwordError}
                <div class="error">{passwordError}</div>
            {/if}
            {#if passwordSuccess}
                <div class="success">{passwordSuccess}</div>
            {/if}

            <div class="field">
                <label for="current-password">{$_('settings.currentPassword')}</label>
                <input
                    id="current-password"
                    type="password"
                    bind:value={currentPassword}
                    required
                    disabled={changingPassword}
                />
            </div>

            <div class="field">
                <label for="new-password">{$_('settings.newPassword')}</label>
                <input
                    id="new-password"
                    type="password"
                    bind:value={newPassword}
                    required
                    disabled={changingPassword}
                    minlength="6"
                />
            </div>

            <div class="field">
                <label for="confirm-password">{$_('settings.confirmNewPassword')}</label>
                <input
                    id="confirm-password"
                    type="password"
                    bind:value={confirmPassword}
                    required
                    disabled={changingPassword}
                />
            </div>

            <button type="submit" disabled={changingPassword}>
                {changingPassword ? $_('settings.changingPassword') : $_('settings.changePassword')}
            </button>
        </form>
    </section>

    <section class="section danger-zone">
        <h2>{$_('settings.dangerZone')}</h2>
        <p class="warning-text">
            {$_('settings.deleteAccountWarning')}
        </p>

        {#if !showDeleteConfirm}
            <button class="btn-danger" onclick={() => showDeleteConfirm = true}>
                {$_('settings.deleteAccount')}
            </button>
        {:else}
            <div class="delete-confirm">
                <h3>{$_('settings.confirmDeletion')}</h3>

                <form onsubmit={handleDeleteAccount}>
                    {#if deleteError}
                        <div class="error">{deleteError}</div>
                    {/if}

                    <div class="field">
                        <label for="delete-password">{$_('settings.enterPasswordToConfirm')}</label>
                        <input
                            id="delete-password"
                            type="password"
                            bind:value={deletePassword}
                            required
                            disabled={deleting}
                            placeholder={$_('settings.yourPassword')}
                        />
                    </div>

                    <div class="button-group">
                        <button
                            type="button"
                            class="btn-secondary"
                            onclick={() => { showDeleteConfirm = false; deletePassword = ''; deleteError = ''; }}
                            disabled={deleting}
                        >
                            {$_('common.cancel')}
                        </button>
                        <button type="submit" class="btn-danger" disabled={deleting}>
                            {deleting ? $_('settings.deleting') : $_('settings.deleteMyAccount')}
                        </button>
                    </div>
                </form>
            </div>
        {/if}
    </section>
</div>

<style>
    .settings-container {
        max-width: 600px;
        margin: 2rem auto;
        padding: 2rem;
    }

    h1 {
        margin-bottom: 2rem;
    }

    h2 {
        font-size: 1.25rem;
        margin-bottom: 1rem;
        color: #333;
    }

    .section {
        background: white;
        border-radius: 12px;
        padding: 1.5rem;
        margin-bottom: 1.5rem;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    }

    .info-grid {
        display: grid;
        gap: 1rem;
    }

    .info-item {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .info-item .label {
        font-size: 0.85rem;
        color: #666;
    }

    .info-item .value {
        font-weight: 500;
    }

    .prefs-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 1rem;
        margin-bottom: 1rem;
    }

    .prefs-grid select,
    .prefs-grid input {
        width: 100%;
    }

    .field {
        margin-bottom: 1rem;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
        font-size: 0.9rem;
    }

    input {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
    }

    input:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    button {
        padding: 0.75rem 1.5rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    button:hover:not(:disabled) {
        opacity: 0.9;
    }

    button:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .btn-secondary {
        background: #e0e0e0;
        color: #333;
    }

    .btn-danger {
        background: #dc3545;
    }

    .error {
        background: #fee;
        color: #c00;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .success {
        background: #efe;
        color: #080;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .danger-zone {
        border: 1px solid #dc3545;
    }

    .danger-zone h2 {
        color: #dc3545;
    }

    .warning-text {
        color: #666;
        margin-bottom: 1rem;
        font-size: 0.9rem;
    }

    .delete-confirm {
        background: #fff5f5;
        padding: 1rem;
        border-radius: 8px;
        margin-top: 1rem;
    }

    .delete-confirm h3 {
        font-size: 1rem;
        margin-bottom: 1rem;
        color: #dc3545;
    }

    .button-group {
        display: flex;
        gap: 1rem;
        margin-top: 1rem;
    }

    /* Mobile responsive */
    @media (max-width: 768px) {
        .settings-container {
            margin: 1rem auto;
            padding: 1rem;
        }

        h1 {
            font-size: 1.5rem;
            margin-bottom: 1.5rem;
        }

        .section {
            padding: 1rem;
            border-radius: 8px;
        }

        input {
            padding: 0.875rem;
            font-size: 16px;
        }

        button {
            width: 100%;
            padding: 0.875rem;
        }

        .button-group {
            flex-direction: column;
        }
    }

    @media (max-width: 480px) {
        .settings-container {
            margin: 0;
            padding: 1rem;
            max-width: 100%;
        }

        h1 {
            font-size: 1.3rem;
        }

        h2 {
            font-size: 1.1rem;
        }

        .section {
            margin-bottom: 1rem;
        }

        input {
            padding: 1rem;
            font-size: 16px;
        }

        button {
            padding: 1rem;
        }
    }
</style>
