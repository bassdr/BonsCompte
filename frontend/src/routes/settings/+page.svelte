<script lang="ts">
    import { goto } from '$app/navigation';
    import { changePassword, deleteAccount, updateUserPreferences, type DeleteAccountResponse, type UpdatePreferencesRequest } from '$lib/api';
    import { auth } from '$lib/auth';
    import { preferences, loadPreferencesFromUser } from '$lib/stores/preferences';
    import { setLocale, _ } from '$lib/i18n';
    import { get } from 'svelte/store';

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

    // Initialize form state from preferences
    const initialPrefs = get(preferences);
    let formLanguage = $state(initialPrefs.language);
    let formDateFormat = $state(initialPrefs.dateFormat);
    let formCurrencyPosition = $state(initialPrefs.currencyPosition);
    let formDecimalSeparator = $state(initialPrefs.decimalSeparator);

    let preferencesError = $state('');
    let preferencesSuccess = $state('');
    let savingPreferences = $state(false);

    // Update form when preferences store changes
    $effect(() => {
        const prefs = $preferences;
        formLanguage = prefs.language;
        formDateFormat = prefs.dateFormat;
        formCurrencyPosition = prefs.currencyPosition;
        formDecimalSeparator = prefs.decimalSeparator;
    });

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

    async function handleSavePreferences(e: Event) {
        e.preventDefault();
        preferencesError = '';
        preferencesSuccess = '';
        savingPreferences = true;

        try {
            const updates: UpdatePreferencesRequest = {
                language: formLanguage,
                date_format: formDateFormat,
                currency_position: formCurrencyPosition,
                decimal_separator: formDecimalSeparator
            };

            const updatedUser = await updateUserPreferences(updates);

            // Reload preferences store and update i18n locale
            loadPreferencesFromUser(updatedUser);
            setLocale(updatedUser.language);

            preferencesSuccess = 'Preferences updated successfully!';
            setTimeout(() => {
                preferencesSuccess = '';
            }, 3000);
        } catch (err) {
            preferencesError = err instanceof Error ? err.message : 'Failed to update preferences';
            console.error('Error updating preferences:', err);
        } finally {
            savingPreferences = false;
        }
    }

    function resetPreferencesForm() {
        const prefs = get(preferences);
        formLanguage = prefs.language;
        formDateFormat = prefs.dateFormat;
        formCurrencyPosition = prefs.currencyPosition;
        formDecimalSeparator = prefs.decimalSeparator;
        preferencesError = '';
        preferencesSuccess = '';
    }
</script>

<div class="settings-container">
    <h1>{$_('settings.title')}</h1>

    {#if $auth.user}
        <section class="section">
            <h2>{$_('settings.account')}</h2>
            <dl class="info-list">
                <div class="info-item">
                    <dt>{$_('auth.username')}</dt>
                    <dd>{$auth.user?.username}</dd>
                </div>
                {#if $auth.user?.display_name}
                    <div class="info-item">
                        <dt>{$_('auth.displayName')}</dt>
                        <dd>{$auth.user?.display_name}</dd>
                    </div>
                {/if}
            </dl>
        </section>
    {/if}

    <section class="section">
        <h2>{$_('settings.preferences')}</h2>

        <form onsubmit={handleSavePreferences}>
            {#if preferencesError}
                <div class="error">{preferencesError}</div>
            {/if}
            {#if preferencesSuccess}
                <div class="success">{preferencesSuccess}</div>
            {/if}

            <div class="field">
                <label for="language">{$_('settings.language')}</label>
                <select id="language" bind:value={formLanguage} disabled={savingPreferences}>
                    <option value="en">English</option>
                    <option value="fr">Français</option>
                </select>
            </div>

            <div class="field">
                <label for="dateFormat">{$_('settings.dateFormat')}</label>
                <select id="dateFormat" bind:value={formDateFormat} disabled={savingPreferences}>
                    <option value="YYYY-MM-DD">YYYY-MM-DD (2025-12-18)</option>
                    <option value="DD/MM/YYYY">DD/MM/YYYY (18/12/2025)</option>
                    <option value="MM/DD/YYYY">MM/DD/YYYY (12/18/2025)</option>
                </select>
            </div>

            <div class="field">
                <label>{$_('settings.currencyPosition')}</label>
                <div class="radio-group">
                    <label class="radio-label">
                        <input type="radio" name="currencyPosition" value="before" bind:group={formCurrencyPosition} disabled={savingPreferences}>
                        {$_('settings.before')}
                    </label>
                    <label class="radio-label">
                        <input type="radio" name="currencyPosition" value="after" bind:group={formCurrencyPosition} disabled={savingPreferences}>
                        {$_('settings.after')}
                    </label>
                </div>
            </div>

            <div class="field">
                <label>{$_('settings.decimalSeparator')}</label>
                <div class="radio-group">
                    <label class="radio-label">
                        <input type="radio" name="decimalSeparator" value="." bind:group={formDecimalSeparator} disabled={savingPreferences}>
                        {$_('settings.period')}
                    </label>
                    <label class="radio-label">
                        <input type="radio" name="decimalSeparator" value="," bind:group={formDecimalSeparator} disabled={savingPreferences}>
                        {$_('settings.comma')}
                    </label>
                </div>
            </div>

            <div class="warning-box">
                {$_('settings.currencyWarning')}
            </div>

            <div class="button-group">
                <button type="submit" disabled={savingPreferences}>
                    {savingPreferences ? $_('common.loading') : $_('common.save')}
                </button>
                <button type="button" class="btn-secondary" onclick={resetPreferencesForm} disabled={savingPreferences}>
                    {$_('common.cancel')}
                </button>
            </div>
        </form>
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
                {changingPassword ? $_('common.loading') : $_('settings.changePassword')}
            </button>
        </form>
    </section>

    <section class="section danger-zone">
        <h2>{$_('settings.deleteAccount')}</h2>
        <p class="warning-text">
            {$_('settings.deleteAccountWarning')}
        </p>

        {#if !showDeleteConfirm}
            <button class="btn-danger" onclick={() => showDeleteConfirm = true}>
                {$_('settings.deleteAccount')}
            </button>
        {:else}
            <div class="delete-confirm">
                <h3>{$_('settings.deleteAccount')}</h3>

                <form onsubmit={handleDeleteAccount}>
                    {#if deleteError}
                        <div class="error">{deleteError}</div>
                    {/if}

                    <div class="field">
                        <label for="delete-password">{$_('auth.password')}</label>
                        <input
                            id="delete-password"
                            type="password"
                            bind:value={deletePassword}
                            required
                            disabled={deleting}
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
                            {deleting ? $_('common.loading') : $_('settings.deleteAccount')}
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
        margin: 0 auto;
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

    .info-list {
        margin: 0;
    }

    .info-item {
        display: flex;
        justify-content: space-between;
        padding: 0.75rem 0;
        border-bottom: 1px solid #eee;
    }

    .info-item:last-child {
        border-bottom: none;
    }

    .info-item dt {
        font-size: 0.9rem;
        color: #666;
    }

    .info-item dd {
        font-weight: 500;
        margin: 0;
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

    input,
    select {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
    }

    input:focus,
    select:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    select {
        cursor: pointer;
    }

    .radio-group {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .radio-label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
        padding: 0.5rem;
        border-radius: 6px;
        font-weight: normal;
        margin-bottom: 0;
    }

    .radio-label:hover {
        background-color: rgba(123, 97, 255, 0.05);
    }

    .radio-label input[type="radio"] {
        width: auto;
        cursor: pointer;
    }

    .warning-box {
        background-color: #fff3cd;
        border: 1px solid #ffc107;
        border-radius: 6px;
        padding: 1rem;
        color: #856404;
        font-size: 0.9rem;
        margin-bottom: 1rem;
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
