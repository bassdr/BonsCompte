<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolveRoute } from '$app/paths';
  import { register, ApiRequestError } from '$lib/api';
  import { auth } from '$lib/auth';
  import { _ } from '$lib/i18n';
  import { preferences } from '$lib/stores/preferences';

  let username = $state('');
  let displayName = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let loading = $state(false);

  // Map error codes to user-friendly translation keys
  function getErrorMessage(code: string): string {
    const errorMap: Record<string, string> = {
      PASSWORD_TOO_WEAK: $_('auth.errors.passwordTooWeak'),
      USERNAME_EXISTS: $_('auth.errors.usernameExists'),
      INVALID_INPUT: $_('auth.errors.invalidInput'),
      INTERNAL_ERROR: $_('auth.errors.internalError')
    };
    return errorMap[code] || $_('auth.registrationFailed');
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = '';

    if (password !== confirmPassword) {
      error = $_('auth.passwordMismatch');
      return;
    }

    if (password.length < 6) {
      error = $_('auth.passwordTooShort');
      return;
    }

    loading = true;

    try {
      const response = await register(username, password, displayName || undefined);
      auth.setAuth(response.token, response.user);
      // Sync preferences from backend
      preferences.initFromUser(response.user.preferences);
      await goto('/');
    } catch (err) {
      if (err instanceof ApiRequestError) {
        error = getErrorMessage(err.code);
      } else {
        error = err instanceof Error ? err.message : $_('auth.registrationFailed');
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="auth-container">
  <h1>{$_('auth.register')}</h1>

  <form onsubmit={handleSubmit}>
    {#if error}
      <div class="error">{error}</div>
    {/if}

    <div class="field">
      <label for="username">{$_('auth.username')}</label>
      <input id="username" type="text" bind:value={username} required disabled={loading} />
    </div>

    <div class="field">
      <label for="displayName">{$_('auth.displayNameOptional')}</label>
      <input id="displayName" type="text" bind:value={displayName} disabled={loading} />
    </div>

    <div class="field">
      <label for="password">{$_('auth.password')}</label>
      <input
        id="password"
        type="password"
        bind:value={password}
        required
        minlength="6"
        disabled={loading}
      />
    </div>

    <div class="field">
      <label for="confirmPassword">{$_('auth.confirmPassword')}</label>
      <input
        id="confirmPassword"
        type="password"
        bind:value={confirmPassword}
        required
        disabled={loading}
      />
    </div>

    <button type="submit" disabled={loading}>
      {loading ? $_('auth.creatingAccount') : $_('auth.register')}
    </button>
  </form>

  <p class="link">
    {$_('auth.hasAccount')} <a href={resolveRoute('/login')}>{$_('auth.login')}</a>
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
</style>
