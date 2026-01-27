<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import {
    login,
    ApiRequestError,
    initiateRecovery,
    getRecoveryIntentStatus,
    resetPasswordWithToken
  } from '$lib/api';
  import { auth } from '$lib/auth';
  import { _ } from '$lib/i18n';
  import { preferences } from '$lib/stores/preferences';

  // Get return URL from query params
  const returnUrl = $derived($page.url.searchParams.get('returnUrl') || '/');

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let info = $state('');
  let loading = $state(false);
  let showForgotPassword = $state(false);

  // Recovery flow state
  let recoveryStep = $state<'init' | 'pending' | 'approved' | 'done'>('init');
  let recoveryUsername = $state('');
  let recoveryToken = $state('');
  let recoveryError = $state('');
  let recoveryInfo = $state('');
  let recoveryLoading = $state(false);
  let recoveryStatus = $state<{
    approvals_count: number;
    required_approvals: number;
    expires_at: string;
    is_expired: boolean;
  } | null>(null);
  let newPassword = $state('');
  let confirmNewPassword = $state('');

  // Map error codes to user-friendly translation keys
  function getErrorMessage(code: string): string {
    const errorMap: Record<string, string> = {
      INVALID_CREDENTIALS: $_('auth.errors.invalidCredentials'),
      ACCOUNT_PENDING: $_('auth.errors.accountPending'),
      ACCOUNT_REVOKED: $_('auth.errors.accountRevoked'),
      INTERNAL_ERROR: $_('auth.errors.internalError')
    };
    return errorMap[code] || $_('auth.loginFailed');
  }

  // Check for session expired or account revoked message on mount
  onMount(() => {
    if (browser) {
      const authMessage = sessionStorage.getItem('auth_message');
      if (authMessage === 'session_expired') {
        info = $_('auth.sessionExpired');
        sessionStorage.removeItem('auth_message');
      } else if (authMessage === 'account_revoked') {
        error = $_('auth.errors.accountRevoked', {
          default: 'Your account has been revoked. Please contact the administrator.'
        });
        sessionStorage.removeItem('auth_message');
      }
    }
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = '';
    info = '';
    loading = true;

    try {
      const response = await login(username, password);
      auth.setAuth(response.token, response.user);
      // Sync preferences from backend
      preferences.initFromUser(response.user.preferences);
      // Redirect to return URL or home
      await goto(returnUrl);
    } catch (err) {
      if (err instanceof ApiRequestError) {
        error = getErrorMessage(err.code);
      } else {
        error = err instanceof Error ? err.message : $_('auth.loginFailed');
      }
    } finally {
      loading = false;
    }
  }

  async function handleInitiateRecovery(e: Event) {
    e.preventDefault();
    recoveryError = '';
    recoveryInfo = '';
    recoveryLoading = true;

    try {
      const response = await initiateRecovery(recoveryUsername);
      recoveryToken = response.token;
      recoveryStep = 'pending';
      recoveryInfo = $_('recovery.requestCreated');
      await checkRecoveryStatus();
    } catch (err) {
      if (err instanceof ApiRequestError) {
        // Generic message to not reveal if user exists
        recoveryInfo = $_('recovery.requestSubmitted');
      } else if (err instanceof TypeError && err.message === 'Failed to fetch') {
        // Network error - backend is down or unreachable
        recoveryError = $_('errors.networkError');
      } else {
        recoveryError = err instanceof Error ? err.message : $_('recovery.initiateFailed');
      }
    } finally {
      recoveryLoading = false;
    }
  }

  async function checkRecoveryStatus() {
    if (!recoveryToken) return;

    try {
      const status = await getRecoveryIntentStatus(recoveryToken);
      recoveryStatus = status;

      if (status.status === 'approved') {
        recoveryStep = 'approved';
        recoveryInfo = $_('recovery.approved');
      } else if (status.status === 'rejected') {
        recoveryError = $_('recovery.rejected');
        recoveryStep = 'init';
      } else if (status.is_expired) {
        recoveryError = $_('recovery.expired');
        recoveryStep = 'init';
      }
    } catch {
      // Silently fail status check
    }
  }

  async function handleResetPassword(e: Event) {
    e.preventDefault();
    recoveryError = '';

    if (newPassword !== confirmNewPassword) {
      recoveryError = $_('auth.passwordMismatch');
      return;
    }

    if (newPassword.length < 6) {
      recoveryError = $_('auth.passwordTooShort');
      return;
    }

    recoveryLoading = true;

    try {
      await resetPasswordWithToken(recoveryToken, newPassword);
      recoveryStep = 'done';
      recoveryInfo = $_('recovery.passwordReset');
    } catch (err) {
      if (err instanceof ApiRequestError) {
        recoveryError = err.message;
      } else {
        recoveryError = err instanceof Error ? err.message : $_('recovery.resetFailed');
      }
    } finally {
      recoveryLoading = false;
    }
  }

  function cancelRecovery() {
    showForgotPassword = false;
    recoveryStep = 'init';
    recoveryToken = '';
    recoveryError = '';
    recoveryInfo = '';
    recoveryStatus = null;
    newPassword = '';
    confirmNewPassword = '';
  }
</script>

<div class="auth-container">
  <h1>{$_('auth.login')}</h1>

  <form onsubmit={handleSubmit}>
    {#if info}
      <div class="info">{info}</div>
    {/if}
    {#if error}
      <div class="error">{error}</div>
    {/if}

    <div class="field">
      <label for="username">{$_('auth.username')}</label>
      <input id="username" type="text" bind:value={username} required disabled={loading} />
    </div>

    <div class="field">
      <label for="password">{$_('auth.password')}</label>
      <input id="password" type="password" bind:value={password} required disabled={loading} />
    </div>

    <button type="submit" disabled={loading}>
      {loading ? $_('auth.loggingIn') : $_('auth.login')}
    </button>

    <button
      type="button"
      class="forgot-password-btn"
      onclick={() => (showForgotPassword = !showForgotPassword)}
    >
      {$_('auth.forgotPassword')}
    </button>
  </form>

  {#if showForgotPassword}
    <div class="forgot-password-panel">
      {#if recoveryStep === 'init'}
        <h3>{$_('recovery.title')}</h3>
        <p class="recovery-description">{$_('recovery.description')}</p>

        {#if recoveryError}
          <div class="error">{recoveryError}</div>
        {/if}
        {#if recoveryInfo}
          <div class="info">{recoveryInfo}</div>
        {/if}

        <form onsubmit={handleInitiateRecovery}>
          <div class="field">
            <label for="recoveryUsername">{$_('auth.username')}</label>
            <input
              id="recoveryUsername"
              type="text"
              bind:value={recoveryUsername}
              required
              disabled={recoveryLoading}
              placeholder={$_('recovery.usernamePlaceholder')}
            />
          </div>
          <button type="submit" disabled={recoveryLoading || !recoveryUsername.trim()}>
            {recoveryLoading ? $_('common.loading') : $_('recovery.initiateButton')}
          </button>
        </form>

        <p class="details-link">
          <a href="/help/password-recovery">{$_('auth.passwordRecoveryLink')}</a>
        </p>
      {:else if recoveryStep === 'pending'}
        <h3>{$_('recovery.pendingTitle')}</h3>

        {#if recoveryError}
          <div class="error">{recoveryError}</div>
        {/if}
        {#if recoveryInfo}
          <div class="info">{recoveryInfo}</div>
        {/if}

        <div class="recovery-status">
          <p>{$_('recovery.pendingDescription')}</p>

          {#if recoveryStatus}
            <div class="status-box">
              <div class="status-item">
                <span class="label">{$_('recovery.approvals')}:</span>
                <span class="value"
                  >{recoveryStatus.approvals_count}/{recoveryStatus.required_approvals}</span
                >
              </div>
              <div class="status-item">
                <span class="label">{$_('recovery.expiresAt')}:</span>
                <span class="value">{recoveryStatus.expires_at}</span>
              </div>
            </div>
          {/if}

          <button type="button" onclick={checkRecoveryStatus} disabled={recoveryLoading}>
            {$_('recovery.checkStatus')}
          </button>
        </div>
      {:else if recoveryStep === 'approved'}
        <h3>{$_('recovery.setNewPasswordTitle')}</h3>

        {#if recoveryError}
          <div class="error">{recoveryError}</div>
        {/if}
        {#if recoveryInfo}
          <div class="info">{recoveryInfo}</div>
        {/if}

        <form onsubmit={handleResetPassword}>
          <div class="field">
            <label for="newPassword">{$_('settings.newPassword')}</label>
            <input
              id="newPassword"
              type="password"
              bind:value={newPassword}
              required
              minlength="6"
              disabled={recoveryLoading}
            />
          </div>
          <div class="field">
            <label for="confirmNewPassword">{$_('settings.confirmNewPassword')}</label>
            <input
              id="confirmNewPassword"
              type="password"
              bind:value={confirmNewPassword}
              required
              disabled={recoveryLoading}
            />
          </div>
          <button type="submit" disabled={recoveryLoading}>
            {recoveryLoading ? $_('common.loading') : $_('recovery.resetButton')}
          </button>
        </form>
      {:else if recoveryStep === 'done'}
        <h3>{$_('recovery.successTitle')}</h3>
        <div class="info">{recoveryInfo}</div>
        <p>{$_('recovery.successDescription')}</p>
        <button type="button" onclick={cancelRecovery}>{$_('recovery.backToLogin')}</button>
      {/if}

      {#if recoveryStep !== 'done'}
        <button type="button" class="cancel-btn" onclick={cancelRecovery}>
          {$_('common.cancel')}
        </button>
      {/if}
    </div>
  {/if}

  <p class="link">
    {$_('auth.noAccount')}
    <a
      href={returnUrl !== '/'
        ? `${resolve('/register')}?returnUrl=${encodeURIComponent(returnUrl)}`
        : resolve('/register')}>{$_('auth.register')}</a
    >
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

  .info {
    background: #e3f2fd;
    color: #1565c0;
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

  .forgot-password-panel {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 8px;
    padding: 1.5rem;
    margin-top: 1rem;
  }

  .forgot-password-panel h3 {
    margin: 0 0 0.75rem 0;
    font-size: 1.1rem;
    color: #333;
  }

  .recovery-description {
    color: #666;
    font-size: 0.9rem;
    margin-bottom: 1rem;
  }

  .forgot-password-panel .field {
    margin-bottom: 1rem;
  }

  .forgot-password-panel button {
    margin-top: 0.5rem;
  }

  .forgot-password-panel .details-link {
    margin-top: 1rem;
    text-align: center;
  }

  .forgot-password-panel .details-link a {
    color: var(--accent, #7b61ff);
    font-size: 0.85rem;
  }

  .cancel-btn {
    background: #e0e0e0 !important;
    color: #333 !important;
    margin-top: 0.5rem !important;
  }

  .recovery-status {
    margin: 1rem 0;
  }

  .recovery-status p {
    color: #666;
    font-size: 0.9rem;
    margin-bottom: 1rem;
  }

  .status-box {
    background: white;
    border: 1px solid #ddd;
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .status-item {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }

  .status-item:last-child {
    margin-bottom: 0;
  }

  .status-item .label {
    color: #666;
  }

  .status-item .value {
    font-weight: 600;
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
