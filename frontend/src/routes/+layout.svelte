<svelte:head>
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous">
  <link href="https://fonts.googleapis.com/css2?family=Nunito:wght@300;400;700;800&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="/src/styles/moit-doll.css">
</svelte:head>

<script lang="ts">
    import { auth, isAuthenticated, isLoading } from '$lib/auth';
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { browser } from '$app/environment';
    import type { Snippet } from 'svelte';

    let { children }: { children: Snippet } = $props();

    // Initialize auth on mount
    $effect(() => {
        auth.init();
    });

    // Redirect to login if not authenticated (except on auth pages)
    $effect(() => {
        if (browser && !$isLoading) {
            const publicPaths = ['/login', '/register'];
            const isPublicPath = publicPaths.includes($page.url.pathname);

            if (!$isAuthenticated && !isPublicPath) {
                goto('/login');
            } else if ($isAuthenticated && isPublicPath) {
                goto('/');
            }
        }
    });

    function handleLogout() {
        auth.logout();
        goto('/login');
    }
</script>

{#if $isLoading}
    <div class="loading">Loading...</div>
{:else}
    {#if $isAuthenticated}
        <nav class="navbar">
            <div class="nav-brand">BonsCompte</div>
            <div class="nav-links">
                <a href="/" class:active={$page.url.pathname === '/'}>Projects</a>
            </div>
            <div class="nav-user">
                <span>{$auth.user?.username}</span>
                <button onclick={handleLogout}>Logout</button>
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

    main {
        padding: 2rem;
        max-width: 1200px;
        margin: 0 auto;
    }
</style>
