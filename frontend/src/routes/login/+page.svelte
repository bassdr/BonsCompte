<script lang="ts">
    import { goto } from '$app/navigation';
    import { login } from '$lib/api';
    import { auth } from '$lib/auth';
    import { _ } from '$lib/i18n';
    import { preferences } from '$lib/stores/preferences';

    let username = $state('');
    let password = $state('');
    let error = $state('');
    let loading = $state(false);
    let showForgotPassword = $state(false);

    async function handleSubmit(e: Event) {
        e.preventDefault();
        error = '';
        loading = true;

        try {
            const response = await login(username, password);
            auth.setAuth(response.token, response.user);
            // Sync preferences from backend
            preferences.initFromUser(response.user.preferences);
            goto('/');
        } catch (err) {
            error = err instanceof Error ? err.message : $_('auth.loginFailed');
        } finally {
            loading = false;
        }
    }
</script>

<div class="auth-container">
    <h1>{$_('auth.login')}</h1>

    <form onsubmit={handleSubmit}>
        {#if error}
            <div class="error">{error}</div>
        {/if}

        <div class="field">
            <label for="username">{$_('auth.username')}</label>
            <input
                id="username"
                type="text"
                bind:value={username}
                required
                disabled={loading}
            />
        </div>

        <div class="field">
            <label for="password">{$_('auth.password')}</label>
            <input
                id="password"
                type="password"
                bind:value={password}
                required
                disabled={loading}
            />
        </div>

        <button type="submit" disabled={loading}>
            {loading ? $_('auth.loggingIn') : $_('auth.login')}
        </button>

        <button
            type="button"
            class="forgot-password-btn"
            onclick={() => showForgotPassword = !showForgotPassword}
        >
            {$_('auth.forgotPassword')}
        </button>
    </form>

    {#if showForgotPassword}
        <div class="forgot-password-info">
            <p>
                {$_('auth.passwordRecoveryInfo')}
            </p>
            <p class="details-link">
                <a href="https://github.com/bassdr/BonsCompte/blob/main/docs/PASSWORD_RECOVERY.md" target="_blank" rel="noopener noreferrer">
                    {$_('auth.passwordRecoveryLink')}
                </a>
            </p>
        </div>
    {/if}

    <p class="link">
        {$_('auth.noAccount')} <a href="/register">{$_('auth.register')}</a>
    </p>
</div>

<style>
    .auth-container {
        max-width: 400px;
        margin: 2rem auto;
        padding: 2rem;
    }

    h1 {
        text-align: center;
        margin-bottom: 2rem;
    }

    .field {
        margin-bottom: 1rem;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
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
        width: 100%;
        padding: 0.75rem;
        margin-top: 1rem;
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

    .error {
        background: #fee;
        color: #c00;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .link {
        text-align: center;
        margin-top: 1.5rem;
    }

    .link a {
        color: var(--accent, #7b61ff);
    }

    .forgot-password-btn {
        width: 100%;
        padding: 0.5rem;
        margin-top: 0.75rem;
        background: transparent;
        color: #666;
        border: none;
        font-size: 0.9rem;
        cursor: pointer;
        text-decoration: underline;
    }

    .forgot-password-btn:hover {
        color: var(--accent, #7b61ff);
    }

    .forgot-password-info {
        background: #f8f9fa;
        border: 1px solid #e9ecef;
        border-radius: 8px;
        padding: 1rem;
        margin-top: 1rem;
        font-size: 0.9rem;
        color: #555;
    }

    .forgot-password-info p {
        margin: 0 0 0.75rem 0;
    }

    .forgot-password-info p:last-child {
        margin-bottom: 0;
    }

    .forgot-password-info .details-link a {
        color: var(--accent, #7b61ff);
        font-size: 0.85rem;
    }

    /* Mobile responsive styles */
    @media (max-width: 768px) {
        .auth-container {
            margin: 1rem auto;
            padding: 1.5rem 1rem;
        }

        h1 {
            font-size: 1.5rem;
            margin-bottom: 1.5rem;
        }

        input {
            padding: 0.875rem;
            font-size: 16px;
        }

        button {
            padding: 0.875rem;
            font-size: 1rem;
        }
    }

    @media (max-width: 480px) {
        .auth-container {
            margin: 1rem 0;
            padding: 1rem;
            max-width: 100%;
        }

        h1 {
            font-size: 1.3rem;
            margin-bottom: 1.25rem;
        }

        .field {
            margin-bottom: 1.25rem;
        }

        input {
            padding: 1rem;
            font-size: 16px;
            border-radius: 6px;
        }

        button {
            padding: 1rem;
            margin-top: 1.5rem;
            font-size: 1rem;
            border-radius: 6px;
        }

        .link {
            margin-top: 1.25rem;
            font-size: 0.95rem;
        }
    }
</style>
