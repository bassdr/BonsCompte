<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/stores';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import { register, addTrustedUser, type TrustedUser } from '$lib/api';
  import { auth } from '$lib/auth';
  import { _ } from '$lib/i18n';
  import { preferences } from '$lib/stores/preferences';
  import { getErrorKey } from '$lib/errors';

  // Get return URL from query params
  const returnUrl = $derived($page.url.searchParams.get('returnUrl') || '/');

  // Step tracking: 'register' or 'trusted-users'
  let step = $state<'register' | 'trusted-users'>('register');

  // Registration form state
  let username = $state('');
  let displayName = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let errorKey = $state('');
  let loading = $state(false);

  // Trusted users form state
  let trustedUsers = $state<TrustedUser[]>([]);
  let newTrustedUsername = $state('');
  let addingTrustedUser = $state(false);
  let trustedUserErrorKey = $state('');

  async function handleSubmit(e: Event) {
    e.preventDefault();
    errorKey = '';

    if (password !== confirmPassword) {
      errorKey = 'auth.passwordMismatch';
      return;
    }

    if (password.length < 6) {
      errorKey = 'auth.passwordTooShort';
      return;
    }

    loading = true;

    try {
      const response = await register(username, password, displayName || undefined);
      auth.setAuth(response.token, response.user);
      // Sync preferences from backend
      preferences.initFromUser(response.user.preferences);
      // Move to trusted users step
      step = 'trusted-users';
    } catch (err) {
      errorKey = getErrorKey(err, 'auth.registrationFailed');
    } finally {
      loading = false;
    }
  }

  async function handleAddTrustedUser(e: Event) {
    e.preventDefault();
    if (!newTrustedUsername.trim()) return;

    addingTrustedUser = true;
    trustedUserErrorKey = '';

    try {
      const newUser = await addTrustedUser(newTrustedUsername.trim());
      trustedUsers = [...trustedUsers, newUser];
      newTrustedUsername = '';
    } catch (err) {
      trustedUserErrorKey = getErrorKey(err, 'settings.trustedUsers.failedToAdd');
    } finally {
      addingTrustedUser = false;
    }
  }

  async function finishRegistration() {
    await goto(returnUrl);
  }
</script>

<div class="auth-container">
  {#if step === 'register'}
    <h1>{$_('auth.register')}</h1>

    <form onsubmit={handleSubmit}>
      {#if errorKey}
        <div class="error">{$_(errorKey)}</div>
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
      {$_('auth.hasAccount')}
      <a
        href={returnUrl !== '/'
          ? `${resolve('/login')}?returnUrl=${encodeURIComponent(returnUrl)}`
          : resolve('/login')}>{$_('auth.login')}</a
      >
    </p>
  {:else}
    <h1>{$_('register.trustedUsersTitle')}</h1>

    <div class="info-notice">
      {$_('register.trustedUsersDescription')}
    </div>

    <div class="security-warning">
      <strong>{$_('settings.trustedUsers.securityWarningTitle')}</strong>
      {$_('settings.trustedUsers.securityWarning')}
    </div>

    {#if trustedUserErrorKey}
      <div class="error">{$_(trustedUserErrorKey)}</div>
    {/if}

    <form class="add-trusted-form" onsubmit={handleAddTrustedUser}>
      <input
        type="text"
        bind:value={newTrustedUsername}
        placeholder={$_('settings.trustedUsers.usernamePlaceholder')}
        disabled={addingTrustedUser}
      />
      <button
        type="submit"
        class="btn-add"
        disabled={addingTrustedUser || !newTrustedUsername.trim()}
      >
        {addingTrustedUser ? $_('common.adding') : $_('common.add')}
      </button>
    </form>

    {#if trustedUsers.length > 0}
      <ul class="trusted-users-list">
        {#each trustedUsers as user (user.id)}
          <li class="trusted-user-item">
            <span class="username">{user.username}</span>
            {#if user.display_name}
              <span class="display-name">({user.display_name})</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    <div class="trusted-status">
      {#if trustedUsers.length >= 2}
        <span class="status-good"
          ><CircleCheck size={18} /> {$_('register.accountRecoverable')}</span
        >
      {:else}
        <span class="status-warning"
          ><CircleAlert size={18} />
          {$_('register.accountNotRecoverable', {
            values: { current: trustedUsers.length, required: 2 }
          })}</span
        >
      {/if}
    </div>

    <div class="button-group">
      <button type="button" class="btn-secondary" onclick={finishRegistration}>
        {$_('register.skipForNow')}
      </button>
      <button type="button" onclick={finishRegistration} disabled={trustedUsers.length < 2}>
        {$_('register.continue')}
      </button>
    </div>

    <p class="info-text">
      {$_('register.canAddLater')}
    </p>
  {/if}
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

  /* Trusted Users Step Styles */
  .info-notice {
    background: #e7f3ff;
    color: #004085;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
    border-left: 4px solid #0066cc;
  }

  .security-warning {
    background: #f8d7da;
    color: #721c24;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.85rem;
    border-left: 4px solid #dc3545;
  }

  .add-trusted-form {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .add-trusted-form input {
    flex: 1;
    margin-bottom: 0;
  }

  .btn-add {
    width: auto;
    margin-top: 0;
    white-space: nowrap;
  }

  .trusted-users-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1rem 0;
  }

  .trusted-user-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem;
    background: #f8f9fa;
    border-radius: 8px;
    margin-bottom: 0.5rem;
  }

  .trusted-user-item .username {
    font-weight: 500;
  }

  .trusted-user-item .display-name {
    color: #666;
    font-size: 0.9rem;
  }

  .trusted-status {
    text-align: center;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }

  .status-good {
    color: #155724;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .status-warning {
    color: #856404;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .button-group {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .button-group button {
    flex: 1;
    margin-top: 0;
  }

  .btn-secondary {
    background: #e0e0e0;
    color: #333;
  }

  .info-text {
    text-align: center;
    color: #666;
    font-size: 0.85rem;
  }
</style>
