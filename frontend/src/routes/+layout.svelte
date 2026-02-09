<script lang="ts">
  import { auth, isAuthenticated, isLoading } from '$lib/auth';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { browser } from '$app/environment';
  import type { Snippet } from 'svelte';
  import Loader from '@lucide/svelte/icons/loader';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import {
    setupI18n,
    getInitialLocale,
    _,
    locale,
    setLocale,
    supportedLanguages,
    isLocaleLoaded
  } from '$lib/i18n';
  import { preferences } from '$lib/stores/preferences';
  // Use our forked SVAR components with separate fonts/icons props
  import { Locale, Material } from '$lib/components/svar';
  // @ts-expect-error - @svar-ui/core-locales doesn't have types
  import { en, fr } from '@svar-ui/core-locales';
  import {
    recoveryStatus,
    refreshRecoveryStatus,
    resetRecoveryStatus,
    isDontShowAgain,
    setDontShowAgain
  } from '$lib/stores/recoveryStatus';
  import { pendingApprovals } from '$lib/stores/pendingApprovals';
  import { onMount, onDestroy } from 'svelte';

  let { children }: { children: Snippet } = $props();

  // Initialize i18n before anything else
  const initialLocale = getInitialLocale();
  setupI18n(initialLocale);

  let approvalCheckInterval: ReturnType<typeof setInterval> | null = null;
  let recoveryCheckInterval: ReturnType<typeof setInterval> | null = null;
  let dismissedForSession = $state(false);
  let showDismissDialog = $state(false);
  let dontShowAgainPermanent = $state(false);

  // Total count of items needing attention from store
  let totalPendingCount = $derived(
    $pendingApprovals.actionableCount +
      $pendingApprovals.pendingMembersCount +
      $pendingApprovals.pendingRecoveriesCount
  );

  // Check if permanently dismissed on mount
  $effect(() => {
    if (browser) {
      dontShowAgainPermanent = isDontShowAgain();
    }
  });

  // Check recovery status when authentication state changes
  $effect(() => {
    if ($isAuthenticated) {
      refreshRecoveryStatus();
      dismissedForSession = false; // Reset session dismiss on login
      // Reset and fetch fresh approval data for this user
      pendingApprovals.resetForNewUser();
      pendingApprovals.refresh();
    } else {
      resetRecoveryStatus();
      // Reset all approval state on logout
      pendingApprovals.reset();
    }
  });

  onMount(() => {
    // Check immediately on mount
    if ($isAuthenticated) {
      pendingApprovals.refresh();
    }

    // Check every 30 seconds
    approvalCheckInterval = setInterval(() => {
      if ($isAuthenticated) pendingApprovals.refresh();
    }, 30000);
    // Check recovery status every 10 seconds (catches changes from settings page)
    recoveryCheckInterval = setInterval(() => {
      if ($isAuthenticated) refreshRecoveryStatus();
    }, 10000);
  });

  onDestroy(() => {
    if (approvalCheckInterval) {
      clearInterval(approvalCheckInterval);
    }
    if (recoveryCheckInterval) {
      clearInterval(recoveryCheckInterval);
    }
  });

  // Get SVAR locale based on current language
  let svarLocale = $derived.by(() => {
    const currentLang = $locale;
    if (currentLang === 'fr') {
      return fr;
    }
    return en; // Default to English for other languages
  });

  function handleDismissClick() {
    showDismissDialog = true;
  }

  function dismissForSession() {
    dismissedForSession = true;
    showDismissDialog = false;
  }

  function dismissPermanently() {
    setDontShowAgain(true);
    dontShowAgainPermanent = true;
    showDismissDialog = false;
  }

  function cancelDismiss() {
    showDismissDialog = false;
  }

  // Initialize auth on mount
  $effect(() => {
    auth.init();
  });

  // When user logs in, sync preferences from backend
  $effect(() => {
    if ($auth.user?.preferences) {
      preferences.initFromUser($auth.user.preferences);
    }
  });

  // When user logs out, reset preferences
  $effect(() => {
    if (!$isAuthenticated && !$isLoading) {
      // Don't reset on initial load, only on logout
    }
  });

  function handleLanguageChange(e: Event) {
    const select = e.target as HTMLSelectElement;
    setLocale(select.value);
  }

  // Redirect to login if not authenticated (except on auth pages)
  $effect(() => {
    if (browser && !$isLoading) {
      const publicPaths = ['/login', '/register', '/help/password-recovery'];
      const isPublicPath = publicPaths.some((p) => $page.url.pathname.startsWith(p));

      if (!$isAuthenticated && !isPublicPath) {
        // Save current URL as return destination after login
        const returnUrl = $page.url.pathname + $page.url.search;
        void goto(`/login?returnUrl=${encodeURIComponent(returnUrl)}`);
      } else if ($isAuthenticated && isPublicPath && !$page.url.pathname.startsWith('/help')) {
        // Check for returnUrl parameter when redirecting authenticated users away from auth pages
        const returnUrl = $page.url.searchParams.get('returnUrl');
        void goto(returnUrl || '/');
      }
    }
  });

  async function handleLogout() {
    auth.logout();
    await goto('/login');
  }
</script>

<svelte:head>
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link
    href="https://fonts.googleapis.com/css2?family=Nunito:wght@300;400;700;800&display=swap"
    rel="stylesheet"
  />
</svelte:head>

{#if $isLoading || !$isLocaleLoaded}
  <div class="loading"><span class="loading-spinner"><Loader size={24} /></span></div>
{:else}
  {#if $isAuthenticated}
    <nav class="navbar">
      <div class="nav-brand">BonsCompte</div>
      <div class="nav-links">
        <a href={resolve('/')} class:active={$page.url.pathname === '/'}>{$_('nav.projects')}</a>
        <a
          href={resolve('/approvals')}
          class="approvals-link"
          class:active={$page.url.pathname.startsWith('/approvals')}
        >
          {$_('nav.approvals', { default: 'Approvals' })}
          {#if totalPendingCount > 0}
            <span class="badge">{totalPendingCount}</span>
          {/if}
        </a>
      </div>
      <div class="nav-user">
        <select class="language-select" value={$locale} onchange={handleLanguageChange}>
          {#each supportedLanguages as lang (lang.code)}
            <option value={lang.code}>{lang.name}</option>
          {/each}
        </select>
        <span>{$auth.user?.username}</span>
        <a
          href={resolve('/settings')}
          class="settings-link"
          class:active={$page.url.pathname === '/settings'}>{$_('nav.settings')}</a
        >
        <button onclick={handleLogout}>{$_('nav.logout')}</button>
      </div>
    </nav>

    {#if $recoveryStatus && !$recoveryStatus.recoverable && !dismissedForSession && !dontShowAgainPermanent}
      <div class="recovery-warning-banner">
        <div class="warning-content">
          <span class="warning-icon"><TriangleAlert size={20} color="currentColor" /></span>
          <span>
            {$_('layout.recoveryWarning', {
              values: {
                current: $recoveryStatus.trusted_user_count,
                required: $recoveryStatus.required_count
              }
            })}
          </span>
          <a href={resolve('/settings')}>{$_('layout.setupTrustedUsers')}</a>
        </div>
        <button class="dismiss-btn" onclick={handleDismissClick}>×</button>
      </div>
    {/if}

    {#if $pendingApprovals.showBanner && totalPendingCount > 0}
      <div class="pending-approvals-banner">
        <div class="banner-content">
          <span class="banner-icon">!</span>
          <span>
            {$_('layout.pendingApprovalsNotice', {
              default: 'You have {count} pending approval(s) that need your attention.',
              values: { count: totalPendingCount }
            })}
          </span>
          <a href={resolve('/approvals')}
            >{$_('layout.viewApprovals', { default: 'View Approvals' })}</a
          >
        </div>
        <button class="dismiss-btn" onclick={() => pendingApprovals.dismissBanner()}>×</button>
      </div>
    {/if}

    {#if showDismissDialog}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="dismiss-dialog-overlay"
        onclick={cancelDismiss}
        onkeydown={(e) => e.key === 'Escape' && cancelDismiss()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="dismiss-dialog-title"
        tabindex="-1"
      >
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="dismiss-dialog" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
          <h3 id="dismiss-dialog-title">{$_('layout.dismissWarningTitle')}</h3>
          <p>{$_('layout.dismissWarningDescription')}</p>
          <div class="dismiss-dialog-buttons">
            <button class="btn-secondary" onclick={cancelDismiss}>
              {$_('common.cancel')}
            </button>
            <button class="btn-secondary" onclick={dismissForSession}>
              {$_('layout.dismissForNow')}
            </button>
            <button class="btn-danger" onclick={dismissPermanently}>
              {$_('layout.dontShowAgain')}
            </button>
          </div>
        </div>
      </div>
    {/if}
  {:else}
    <!-- Language selector for unauthenticated pages -->
    <nav class="navbar navbar-minimal">
      <div class="nav-brand">BonsCompte</div>
      <div class="nav-user">
        <select class="language-select" value={$locale} onchange={handleLanguageChange}>
          {#each supportedLanguages as lang (lang.code)}
            <option value={lang.code}>{lang.name}</option>
          {/each}
        </select>
      </div>
    </nav>
  {/if}

  <Material fonts={false}>
    {#key $locale}
      <Locale words={svarLocale}>
        <main>
          {@render children()}
        </main>
      </Locale>
    {/key}
  </Material>
{/if}

<style>
  .loading {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100vh;
    font-size: 1.2rem;
    color: #666;
  }

  .loading-spinner {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .navbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 2rem;
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-bottom: 1px solid #eee;
  }

  .nav-brand {
    font-weight: 800;
    font-size: 1.4rem;
    color: var(--accent, #7b61ff);
  }

  .nav-links {
    display: flex;
    gap: 1.5rem;
  }

  .nav-links a {
    text-decoration: none;
    color: #666;
    font-weight: 600;
    transition: color 0.2s;
  }

  .nav-links a:hover,
  .nav-links a.active {
    color: var(--accent, #7b61ff);
  }

  .nav-user {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .nav-user span {
    color: #666;
  }

  .nav-user button {
    padding: 0.5rem 1rem;
    background: transparent;
    border: 1px solid #ddd;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .nav-user button:hover {
    background: #f5f5f5;
  }

  .language-select {
    padding: 0.4rem 0.6rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 0.9rem;
    color: #333;
  }

  .language-select:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .navbar-minimal {
    justify-content: space-between;
  }

  .settings-link {
    text-decoration: none;
    color: #666;
    font-weight: 500;
    transition: color 0.2s;
  }

  .settings-link:hover,
  .settings-link.active {
    color: var(--accent, #7b61ff);
  }

  .approvals-link {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .badge {
    background: var(--accent, #7b61ff);
    color: white;
    padding: 0.15rem 0.5rem;
    border-radius: 10px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  main {
    padding: 2rem;
    max-width: 1200px;
    margin: 0 auto;
  }

  .recovery-warning-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 2rem;
    background: #fff3cd;
    border-bottom: 1px solid #ffc107;
    color: #856404;
    font-size: 0.9rem;
  }

  .pending-approvals-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 2rem;
    background: #cfe2ff;
    border-bottom: 1px solid #3b82f6;
    color: #1e40af;
    font-size: 0.9rem;
  }

  .banner-content {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .banner-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    background: #3b82f6;
    color: white;
    border-radius: 50%;
    font-size: 0.8rem;
    font-weight: 700;
  }

  .banner-content a {
    color: #1e40af;
    font-weight: 600;
    text-decoration: underline;
  }

  .banner-content a:hover {
    color: #1e3a8a;
  }

  .warning-content {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .warning-icon {
    flex-shrink: 0;
  }

  .warning-content a {
    color: #856404;
    font-weight: 600;
    text-decoration: underline;
  }

  .warning-content a:hover {
    color: #533f03;
  }

  .dismiss-btn {
    background: transparent;
    border: none;
    font-size: 1.2rem;
    color: #856404;
    cursor: pointer;
    padding: 0 0.5rem;
    line-height: 1;
  }

  .dismiss-btn:hover {
    color: #533f03;
  }

  .dismiss-dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .dismiss-dialog {
    background: white;
    padding: 1.5rem;
    border-radius: 8px;
    max-width: 400px;
    width: 90%;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  }

  .dismiss-dialog h3 {
    margin: 0 0 0.75rem 0;
    font-size: 1.1rem;
    color: #333;
  }

  .dismiss-dialog p {
    margin: 0 0 1.25rem 0;
    color: #666;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .dismiss-dialog-buttons {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .dismiss-dialog-buttons button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-secondary {
    background: #f5f5f5;
    border: 1px solid #ddd;
    color: #333;
  }

  .btn-secondary:hover {
    background: #eee;
  }

  .btn-danger {
    background: #dc3545;
    border: 1px solid #dc3545;
    color: white;
  }

  .btn-danger:hover {
    background: #c82333;
  }

  /* Mobile responsive styles */
  @media (max-width: 768px) {
    .navbar {
      padding: 0.75rem 1rem;
      flex-wrap: wrap;
      gap: 0.5rem;
    }

    .nav-brand {
      font-size: 1.2rem;
    }

    .nav-links {
      order: 3;
      width: 100%;
      justify-content: center;
      gap: 1rem;
      padding-top: 0.5rem;
      border-top: 1px solid #eee;
    }

    .nav-links a {
      font-size: 0.9rem;
    }

    .nav-user {
      gap: 0.5rem;
    }

    .nav-user span {
      display: none;
    }

    .language-select {
      padding: 0.3rem 0.4rem;
      font-size: 0.85rem;
    }

    .nav-user button {
      padding: 0.4rem 0.8rem;
      font-size: 0.9rem;
    }

    main {
      padding: 1rem;
    }
  }

  @media (max-width: 480px) {
    .navbar {
      padding: 0.5rem 0.75rem;
    }

    .nav-brand {
      font-size: 1.1rem;
    }

    .nav-links {
      gap: 0.75rem;
    }

    .nav-links a {
      font-size: 0.85rem;
    }

    .settings-link {
      font-size: 0.85rem;
    }

    main {
      padding: 0.75rem;
    }
  }

  /* SVAR Theme Customizations */
  /* Override Material theme variables to match app design */
  :global(.wx-material-theme) {
    /* Use app's font instead of Roboto */
    --wx-font-family:
      'Nunito', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial,
      sans-serif;

    /* Use app's accent color */
    --wx-color-primary: var(--accent, #7b61ff);
    --wx-color-primary-selected: rgba(123, 97, 255, 0.15);

    /* Better input styling */
    --wx-input-border: 1px solid #ddd;
    --wx-input-border-focus: 1px solid var(--accent, #7b61ff);
    --wx-input-border-radius: 6px;
    --wx-input-padding: 8px 12px;
    --wx-input-height: 40px;

    /* Popup/dropdown styling */
    --wx-popup-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    --wx-popup-border: 1px solid #ddd;
    --wx-popup-border-radius: 8px;

    /* Calendar styling */
    --wx-calendar-padding: 12px;
    --wx-calendar-cell-size: 32px;
    --wx-calendar-gap: 2px;

    /* Button styling */
    --wx-button-border-radius: 4px;
    --wx-button-padding: 4px 12px;
    --wx-button-height: 28px;
    --wx-button-font-size: 13px;

    /* Reset Material's full height */
    height: auto !important;
  }

  /* Fix focus ring for inputs */
  :global(.wx-material-theme input:focus) {
    box-shadow: 0 0 0 3px rgba(123, 97, 255, 0.1);
    outline: none;
  }

  /* Fix SVAR dropdown z-index and overflow */
  :global(.wx-dropdown) {
    z-index: 1000 !important;
    overflow: visible !important;
  }

  /* Ensure calendar inside dropdown can overflow */
  :global(.wx-dropdown .wx-calendar) {
    overflow: visible !important;
  }
</style>
