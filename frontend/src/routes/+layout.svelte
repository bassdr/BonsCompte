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

  let { children }: { children: Snippet } = $props();

  // Initialize i18n before anything else
  const initialLocale = getInitialLocale();
  setupI18n(initialLocale);

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
      const publicPaths = ['/login', '/register'];
      const isPublicPath = publicPaths.includes($page.url.pathname);

      if (!$isAuthenticated && !isPublicPath) {
        void goto('/login');
      } else if ($isAuthenticated && isPublicPath) {
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
    padding: 0.5rem 1rem;
    border-radius: 6px;
    transition: all 0.2s;
  }

  .settings-link:hover,
  .settings-link.active {
    color: var(--accent, #7b61ff);
    background: rgba(123, 97, 255, 0.1);
  }

  main {
    padding: 2rem 1rem;
    max-width: 1200px;
    margin: 0 auto;
  }

  /* Mobile responsive styles */
  @media (max-width: 768px) {
    .navbar {
      flex-wrap: wrap;
      gap: 1rem;
      padding: 0.75rem 1rem;
    }

    .nav-brand {
      font-size: 1.1rem;
      flex: 0 0 100%;
    }

    .nav-links {
      gap: 1rem;
      flex: 1;
    }

    .nav-links a {
      font-size: 0.9rem;
      padding: 0.5rem 0.75rem;
    }

    .nav-user {
      flex: 0 0 100%;
      justify-content: space-between;
      gap: 0.75rem;
    }

    .nav-user span {
      font-size: 0.9rem;
    }

    .nav-user button {
      padding: 0.4rem 0.8rem;
      font-size: 0.9rem;
    }

    main {
      padding: 1rem 0.75rem;
    }
  }

  @media (max-width: 480px) {
    .navbar {
      padding: 0.5rem 0.5rem;
    }

    .nav-brand {
      font-size: 1rem;
    }

    .nav-links {
      gap: 0.5rem;
    }

    .nav-links a {
      font-size: 0.8rem;
      padding: 0.4rem 0.6rem;
    }

    main {
      padding: 0.75rem 0.5rem;
    }
  }
</style>
