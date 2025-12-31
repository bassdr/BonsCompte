<script lang="ts">
  import { auth, isAuthenticated, isLoading } from '$lib/auth';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { browser } from '$app/environment';
  import type { Snippet } from 'svelte';
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
  import { getActionableApprovals } from '$lib/api';
  import { onMount, onDestroy } from 'svelte';

  let { children }: { children: Snippet } = $props();

  // Initialize i18n before anything else
  const initialLocale = getInitialLocale();
  setupI18n(initialLocale);

  let actionableApprovalsCount = $state(0);
  let approvalCheckInterval: ReturnType<typeof setInterval> | null = null;

  async function checkActionableApprovals() {
    if (!$isAuthenticated) return;

    try {
      const approvals = await getActionableApprovals();
      actionableApprovalsCount = approvals.length;
    } catch {
      // Silently fail - approvals might not be accessible
      actionableApprovalsCount = 0;
    }
  }

  onMount(() => {
    // Check immediately on mount
    checkActionableApprovals();

    // Check every 30 seconds
    approvalCheckInterval = setInterval(checkActionableApprovals, 30000);
  });

  onDestroy(() => {
    if (approvalCheckInterval) {
      clearInterval(approvalCheckInterval);
    }
  });

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
        void goto('/login');
      } else if ($isAuthenticated && isPublicPath && !$page.url.pathname.startsWith('/help')) {
        void goto('/');
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
  <div class="loading">&#x27F3;</div>
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
          {#if actionableApprovalsCount > 0}
            <span class="badge">{actionableApprovalsCount}</span>
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

  <main>
    {@render children()}
  </main>
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
</style>
