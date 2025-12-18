<script lang="ts">
    import { goto } from '$app/navigation';
    import { changePassword, deleteAccount, type DeleteAccountResponse } from '$lib/api';
    import { auth } from '$lib/auth';

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
    <h1>Account Settings</h1>

    {#if $auth.user}
        <section class="section">
            <h2>Account Info</h2>
            <div class="info-grid">
                <div class="info-item">
                    <span class="label">Username</span>
                    <span class="value">{$auth.user?.username}</span>
                </div>
                {#if $auth.user?.display_name}
                    <div class="info-item">
                        <span class="label">Display Name</span>
                        <span class="value">{$auth.user?.display_name}</span>
                    </div>
                {/if}
            </div>
        </section>
    {/if}

    <section class="section">
        <h2>Change Password</h2>

        <form onsubmit={handleChangePassword}>
            {#if passwordError}
                <div class="error">{passwordError}</div>
            {/if}
            {#if passwordSuccess}
                <div class="success">{passwordSuccess}</div>
            {/if}

            <div class="field">
                <label for="current-password">Current Password</label>
                <input
                    id="current-password"
                    type="password"
                    bind:value={currentPassword}
                    required
                    disabled={changingPassword}
                />
            </div>

            <div class="field">
                <label for="new-password">New Password</label>
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
                <label for="confirm-password">Confirm New Password</label>
                <input
                    id="confirm-password"
                    type="password"
                    bind:value={confirmPassword}
                    required
                    disabled={changingPassword}
                />
            </div>

            <button type="submit" disabled={changingPassword}>
                {changingPassword ? 'Changing...' : 'Change Password'}
            </button>
        </form>
    </section>

    <section class="section danger-zone">
        <h2>Danger Zone</h2>
        <p class="warning-text">
            Deleting your account is permanent and cannot be undone.
            Projects you own will be transferred to another admin or deleted if no other members exist.
        </p>

        {#if !showDeleteConfirm}
            <button class="btn-danger" onclick={() => showDeleteConfirm = true}>
                Delete Account
            </button>
        {:else}
            <div class="delete-confirm">
                <h3>Confirm Account Deletion</h3>

                <form onsubmit={handleDeleteAccount}>
                    {#if deleteError}
                        <div class="error">{deleteError}</div>
                    {/if}

                    <div class="field">
                        <label for="delete-password">Enter your password to confirm</label>
                        <input
                            id="delete-password"
                            type="password"
                            bind:value={deletePassword}
                            required
                            disabled={deleting}
                            placeholder="Your password"
                        />
                    </div>

                    <div class="button-group">
                        <button
                            type="button"
                            class="btn-secondary"
                            onclick={() => { showDeleteConfirm = false; deletePassword = ''; deleteError = ''; }}
                            disabled={deleting}
                        >
                            Cancel
                        </button>
                        <button type="submit" class="btn-danger" disabled={deleting}>
                            {deleting ? 'Deleting...' : 'Delete My Account'}
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
