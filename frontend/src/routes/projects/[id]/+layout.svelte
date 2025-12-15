<script lang="ts">
    import { page } from '$app/stores';
    import { loadProject, clearProject, currentProject, isAdmin } from '$lib/stores/project';
    import type { Snippet } from 'svelte';

    let { children }: { children: Snippet } = $props();

    let loading = $state(true);
    let error = $state('');

    $effect(() => {
        const projectId = parseInt($page.params.id ?? '');
        if (!isNaN(projectId)) {
            loadProjectData(projectId);
        }

        return () => {
            clearProject();
        };
    });

    async function loadProjectData(projectId: number) {
        loading = true;
        error = '';
        try {
            await loadProject(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load project';
        } finally {
            loading = false;
        }
    }
</script>

{#if loading}
    <div class="loading">Loading project...</div>
{:else if error}
    <div class="error">
        <h2>Error</h2>
        <p>{error}</p>
        <a href="/">Back to projects</a>
    </div>
{:else if $currentProject}
    <div class="project-layout">
        <header class="project-header">
            <div class="project-title">
                <a href="/" class="back-link">&larr;</a>
                <h1>{$currentProject.name}</h1>
            </div>
            <nav class="project-nav">
                <a href="/projects/{$page.params.id}" class:active={$page.url.pathname === `/projects/${$page.params.id}`}>
                    Payments
                </a>
                <a href="/projects/{$page.params.id}/debts" class:active={$page.url.pathname.includes('/debts')}>
                    Debts
                </a>
                <a href="/projects/{$page.params.id}/participants" class:active={$page.url.pathname.includes('/participants')}>
                    Participants
                </a>
                <a href="/projects/{$page.params.id}/members" class:active={$page.url.pathname.includes('/members')}>
                    Members
                </a>
                {#if $isAdmin}
                    <a href="/projects/{$page.params.id}/settings" class:active={$page.url.pathname.includes('/settings')}>
                        Settings
                    </a>
                {/if}
            </nav>
        </header>

        <div class="project-content">
            {@render children()}
        </div>
    </div>
{/if}

<style>
    .loading {
        display: flex;
        justify-content: center;
        align-items: center;
        min-height: 200px;
        color: #666;
    }

    .error {
        text-align: center;
        padding: 2rem;
    }

    .error h2 {
        color: #c00;
    }

    .error a {
        color: var(--accent, #7b61ff);
    }

    .project-layout {
        min-height: 100%;
    }

    .project-header {
        margin-bottom: 2rem;
    }

    .project-title {
        display: flex;
        align-items: center;
        gap: 1rem;
        margin-bottom: 1rem;
    }

    .back-link {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 36px;
        height: 36px;
        background: rgba(255, 255, 255, 0.8);
        border-radius: 8px;
        text-decoration: none;
        color: #666;
        font-size: 1.2rem;
    }

    .back-link:hover {
        background: rgba(255, 255, 255, 1);
        color: var(--accent, #7b61ff);
    }

    h1 {
        margin: 0;
        color: var(--accent, #7b61ff);
    }

    .project-nav {
        display: flex;
        gap: 0.5rem;
        background: rgba(255, 255, 255, 0.8);
        backdrop-filter: blur(10px);
        border-radius: 12px;
        padding: 0.5rem;
    }

    .project-nav a {
        padding: 0.75rem 1.25rem;
        text-decoration: none;
        color: #666;
        font-weight: 600;
        border-radius: 8px;
        transition: all 0.2s;
    }

    .project-nav a:hover {
        background: rgba(123, 97, 255, 0.1);
        color: var(--accent, #7b61ff);
    }

    .project-nav a.active {
        background: var(--accent, #7b61ff);
        color: white;
    }

    .project-content {
        /* Content area */
    }
</style>
