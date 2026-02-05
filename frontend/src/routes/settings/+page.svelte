<script lang="ts">
  import { goto } from '$app/navigation';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import {
    changePassword,
    deleteAccount,
    updatePreferences,
    updateProfile,
    getTrustedUsers,
    addTrustedUser,
    removeTrustedUser,
    getRecoveryStatus,
    type TrustedUser,
    type RecoveryStatus
  } from '$lib/api';
  import { auth } from '$lib/auth';
  import { _ } from '$lib/i18n';
  import { preferences } from '$lib/stores/preferences';
  import { getErrorKey } from '$lib/errors';

  // Profile state
  let editingProfile = $state(false);
  let profileDisplayName = $state($auth.user?.display_name ?? '');
  let profileSaving = $state(false);
  let profileErrorKey = $state('');
  let profileSuccess = $state('');

  // Password change state
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let passwordErrorKey = $state('');
  let passwordSuccess = $state('');
  let changingPassword = $state(false);

  // Delete account state
  let deletePassword = $state('');
  let deleteErrorKey = $state('');
  let deleting = $state(false);
  let showDeleteConfirm = $state(false);

  // Preferences state
  let prefsDateFormat = $state($preferences.date_format);
  let prefsDecimalSeparator = $state($preferences.decimal_separator);
  let prefsCurrencySymbol = $state($preferences.currency_symbol);
  let prefsCurrencyPosition = $state($preferences.currency_symbol_position);
  let prefsSaving = $state(false);
  let prefsSuccess = $state('');
  let prefsErrorKey = $state('');

  // Trusted users state
  let trustedUsers = $state<TrustedUser[]>([]);
  let recoveryStatus = $state<RecoveryStatus | null>(null);
  let trustedUsersLoading = $state(true);
  let trustedUsersErrorKey = $state('');
  let newTrustedUsername = $state('');
  let addingTrustedUser = $state(false);
  let removingTrustedUserId = $state<number | null>(null);

  // Sync preferences state when store changes
  $effect(() => {
    prefsDateFormat = $preferences.date_format;
    prefsDecimalSeparator = $preferences.decimal_separator;
    prefsCurrencySymbol = $preferences.currency_symbol;
    prefsCurrencyPosition = $preferences.currency_symbol_position;
  });

  // Load trusted users on mount
  $effect(() => {
    loadTrustedUsers();
  });

  async function loadTrustedUsers() {
    trustedUsersLoading = true;
    trustedUsersErrorKey = '';
    try {
      const [users, status] = await Promise.all([getTrustedUsers(), getRecoveryStatus()]);
      trustedUsers = users;
      recoveryStatus = status;
    } catch (err) {
      trustedUsersErrorKey = getErrorKey(err, 'settings.trustedUsers.failedToLoad');
    } finally {
      trustedUsersLoading = false;
    }
  }

  async function handleAddTrustedUser(e: Event) {
    e.preventDefault();
    if (!newTrustedUsername.trim()) return;

    addingTrustedUser = true;
    trustedUsersErrorKey = '';

    try {
      const newUser = await addTrustedUser(newTrustedUsername.trim());
      trustedUsers = [newUser, ...trustedUsers];
      newTrustedUsername = '';
      // Refresh recovery status
      recoveryStatus = await getRecoveryStatus();
    } catch (err) {
      trustedUsersErrorKey = getErrorKey(err, 'settings.trustedUsers.failedToAdd');
    } finally {
      addingTrustedUser = false;
    }
  }

  async function handleRemoveTrustedUser(id: number) {
    removingTrustedUserId = id;
    trustedUsersErrorKey = '';

    try {
      await removeTrustedUser(id);
      trustedUsers = trustedUsers.filter((u) => u.id !== id);
      // Refresh recovery status
      recoveryStatus = await getRecoveryStatus();
    } catch (err) {
      trustedUsersErrorKey = getErrorKey(err, 'settings.trustedUsers.failedToRemove');
    } finally {
      removingTrustedUserId = null;
    }
  }

  function startEditProfile() {
    profileDisplayName = $auth.user?.display_name ?? '';
    editingProfile = true;
    profileErrorKey = '';
    profileSuccess = '';
  }

  function cancelEditProfile() {
    editingProfile = false;
    profileDisplayName = $auth.user?.display_name ?? '';
    profileErrorKey = '';
  }

  async function handleProfileSave() {
    profileSaving = true;
    profileErrorKey = '';
    profileSuccess = '';

    try {
      const updatedUser = await updateProfile({
        display_name: profileDisplayName.trim() || null
      });
      auth.updateUser({ display_name: updatedUser.display_name });
      profileSuccess = $_('settings.profileSaved');
      editingProfile = false;
    } catch (err) {
      profileErrorKey = getErrorKey(err, 'settings.failedToSaveProfile');
    } finally {
      profileSaving = false;
    }
  }

  async function handlePreferenceSave() {
    prefsSaving = true;
    prefsSuccess = '';
    prefsErrorKey = '';

    try {
      const updatedPrefs = await updatePreferences({
        date_format: prefsDateFormat,
        decimal_separator: prefsDecimalSeparator,
        currency_symbol: prefsCurrencySymbol,
        currency_symbol_position: prefsCurrencyPosition
      });

      preferences.setAll(updatedPrefs);
      prefsSuccess = $_('settings.preferencesSaved');
    } catch (err) {
      prefsErrorKey = getErrorKey(err, 'settings.failedToSavePreferences');
    } finally {
      prefsSaving = false;
    }
  }

  async function handleChangePassword(e: Event) {
    e.preventDefault();
    passwordErrorKey = '';
    passwordSuccess = '';

    if (newPassword !== confirmPassword) {
      passwordErrorKey = 'settings.passwordsDoNotMatch';
      return;
    }

    if (newPassword.length < 6) {
      passwordErrorKey = 'settings.passwordTooShort';
      return;
    }

    changingPassword = true;

    try {
      await changePassword(currentPassword, newPassword);
      passwordSuccess = $_('settings.passwordChanged');
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      passwordErrorKey = getErrorKey(err, 'settings.failedToChangePassword');
    } finally {
      changingPassword = false;
    }
  }

  async function handleDeleteAccount(e: Event) {
    e.preventDefault();
    deleteErrorKey = '';

    if (!deletePassword) {
      deleteErrorKey = 'settings.passwordRequired';
      return;
    }

    deleting = true;

    try {
      await deleteAccount(deletePassword);
      // Log out and redirect
      auth.logout();
      await goto('/login');
    } catch (err) {
      deleteErrorKey = getErrorKey(err, 'settings.failedToDelete');
    } finally {
      deleting = false;
    }
  }
</script>

<div class="settings-container">
  <button type="button" class="back-link" onclick={() => history.back()}>{$_('common.back')}</button
  >
  <h1>{$_('settings.title')}</h1>

  {#if $auth.user}
    <section class="section">
      <h2>{$_('settings.accountInfo')}</h2>

      {#if profileErrorKey}
        <div class="error">{$_(profileErrorKey)}</div>
      {/if}
      {#if profileSuccess}
        <div class="success">{profileSuccess}</div>
      {/if}

      <div class="info-grid">
        <div class="info-item">
          <span class="label">{$_('auth.username')}</span>
          <span class="value">{$auth.user?.username}</span>
        </div>
        <div class="info-item">
          <span class="label">{$_('auth.displayName')}</span>
          {#if editingProfile}
            <div class="edit-inline">
              <input
                type="text"
                bind:value={profileDisplayName}
                placeholder={$_('settings.displayNamePlaceholder')}
                disabled={profileSaving}
              />
              <div class="edit-buttons">
                <button type="button" onclick={handleProfileSave} disabled={profileSaving}>
                  {profileSaving ? $_('common.saving') : $_('common.save')}
                </button>
                <button
                  type="button"
                  class="btn-secondary"
                  onclick={cancelEditProfile}
                  disabled={profileSaving}
                >
                  {$_('common.cancel')}
                </button>
              </div>
            </div>
          {:else}
            <button type="button" class="value-button" onclick={startEditProfile}>
              <span class="value-text">{$auth.user?.display_name || $_('settings.notSet')}</span>
              <span class="edit-hint">{$_('common.edit')}</span>
            </button>
          {/if}
        </div>
      </div>
    </section>
  {/if}

  <section class="section">
    <h2>{$_('settings.preferences')}</h2>

    <div class="info-notice">
      {$_('settings.preferencesNotice')}
    </div>

    {#if prefsErrorKey}
      <div class="error">{$_(prefsErrorKey)}</div>
    {/if}
    {#if prefsSuccess}
      <div class="success">{prefsSuccess}</div>
    {/if}

    <div class="prefs-grid">
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

  <section class="section trusted-users-section">
    <h2>{$_('settings.trustedUsers.title')}</h2>

    <div class="info-notice">
      {$_('settings.trustedUsers.description')}
    </div>

    <div class="security-warning">
      <strong>{$_('settings.trustedUsers.securityWarningTitle')}</strong>
      {$_('settings.trustedUsers.securityWarning')}
    </div>

    {#if trustedUsersErrorKey}
      <div class="error">{$_(trustedUsersErrorKey)}</div>
    {/if}

    <form class="add-trusted-form" onsubmit={handleAddTrustedUser}>
      <input
        type="text"
        bind:value={newTrustedUsername}
        placeholder={$_('settings.trustedUsers.usernamePlaceholder')}
        disabled={addingTrustedUser}
      />
      <button type="submit" disabled={addingTrustedUser || !newTrustedUsername.trim()}>
        {addingTrustedUser ? $_('common.adding') : $_('common.add')}
      </button>
    </form>

    {#if trustedUsersLoading}
      <div class="loading">{$_('common.loading')}</div>
    {:else if trustedUsers.length === 0}
      <div class="empty-state">
        {$_('settings.trustedUsers.noTrustedUsers')}
      </div>
    {:else}
      <ul class="trusted-users-list">
        {#each trustedUsers as user (user.id)}
          <li class="trusted-user-item">
            <div class="user-info">
              <span class="username">{user.username}</span>
              {#if user.display_name}
                <span class="display-name">({user.display_name})</span>
              {/if}
            </div>
            <button
              type="button"
              class="btn-remove"
              onclick={() => handleRemoveTrustedUser(user.id)}
              disabled={removingTrustedUserId === user.id}
            >
              {removingTrustedUserId === user.id ? $_('common.removing') : $_('common.remove')}
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if recoveryStatus}
      <div class="recovery-status" class:recoverable={recoveryStatus.recoverable}>
        {#if recoveryStatus.recoverable}<CircleCheck
            size={24}
            color="currentColor"
          />{:else}<CircleAlert size={24} color="currentColor" />{/if}
        <span>
          {recoveryStatus.recoverable
            ? $_('settings.trustedUsers.statusRecoverable')
            : $_('settings.trustedUsers.statusNotRecoverable', {
                values: {
                  current: recoveryStatus.trusted_user_count,
                  required: recoveryStatus.required_count
                }
              })}
        </span>
      </div>
    {/if}
  </section>

  <section class="section">
    <h2>{$_('settings.changePassword')}</h2>

    <form onsubmit={handleChangePassword}>
      {#if passwordErrorKey}
        <div class="error">{$_(passwordErrorKey)}</div>
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
      <button class="btn-danger" onclick={() => (showDeleteConfirm = true)}>
        {$_('settings.deleteAccount')}
      </button>
    {:else}
      <div class="delete-confirm">
        <h3>{$_('settings.confirmDeletion')}</h3>

        <form onsubmit={handleDeleteAccount}>
          {#if deleteErrorKey}
            <div class="error">{$_(deleteErrorKey)}</div>
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
              onclick={() => {
                showDeleteConfirm = false;
                deletePassword = '';
                deleteErrorKey = '';
              }}
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

  .back-link {
    display: block;
    background: none;
    border: none;
    padding: 0;
    color: var(--accent, #7b61ff);
    text-decoration: none;
    font-size: 0.95rem;
    font-weight: 500;
    margin-bottom: 1rem;
    cursor: pointer;
    text-align: left;
  }

  .back-link:hover {
    text-decoration: underline;
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

  .value-button {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    cursor: pointer;
    text-align: left;
    color: inherit;
  }

  .value-button .value-text {
    font-weight: 500;
    color: #333;
  }

  .value-button:hover .value-text {
    color: var(--accent, #7b61ff);
  }

  .edit-hint {
    font-size: 0.75rem;
    color: #999;
    font-weight: 400;
  }

  .value-button:hover .edit-hint {
    color: var(--accent, #7b61ff);
  }

  .edit-inline {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .edit-inline input {
    padding: 0.5rem;
    font-size: 0.95rem;
  }

  .edit-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .edit-buttons button {
    padding: 0.5rem 1rem;
    font-size: 0.85rem;
  }

  .info-notice {
    background: #e7f3ff;
    color: #004085;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
    border-left: 4px solid #0066cc;
  }

  .prefs-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  @media (min-width: 600px) {
    .prefs-grid {
      grid-template-columns: 1fr 1fr;
    }
  }

  .prefs-grid select,
  .prefs-grid input {
    width: 100%;
    box-sizing: border-box;
  }

  .field {
    margin-bottom: 0;
    min-width: 0;
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
    box-sizing: border-box;
  }

  select {
    box-sizing: border-box;
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

  /* Trusted Users Section */
  .trusted-users-section .security-warning {
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

  .add-trusted-form button {
    width: auto;
    white-space: nowrap;
  }

  .trusted-users-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1rem 0;
  }

  .trusted-user-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: #f8f9fa;
    border-radius: 8px;
    margin-bottom: 0.5rem;
  }

  .user-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .user-info .username {
    font-weight: 500;
  }

  .user-info .display-name {
    color: #666;
    font-size: 0.9rem;
  }

  .btn-remove {
    background: #dc3545;
    padding: 0.4rem 0.75rem;
    font-size: 0.85rem;
  }

  .empty-state {
    color: #666;
    text-align: center;
    padding: 1rem;
    font-style: italic;
  }

  .loading {
    color: #666;
    text-align: center;
    padding: 1rem;
  }

  .recovery-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem;
    border-radius: 8px;
    background: #fff3cd;
    color: #856404;
    font-size: 0.9rem;
  }

  .recovery-status.recoverable {
    background: #d4edda;
    color: #155724;
  }

  @media (max-width: 480px) {
    .add-trusted-form {
      flex-direction: column;
    }

    .add-trusted-form button {
      width: 100%;
    }

    .trusted-user-item {
      flex-direction: column;
      gap: 0.5rem;
      align-items: flex-start;
    }

    .btn-remove {
      width: 100%;
    }
  }
</style>
