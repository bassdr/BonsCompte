<script lang="ts">
	import { page } from '$app/stores';
	import { resolveRoute } from '$app/paths';
	import { loadProject, clearProject, currentProject, isAdmin } from '$lib/stores/project';
	import type { Snippet } from 'svelte';
	import { _ } from '$lib/i18n';

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
			error = e instanceof Error ? e.message : $_('projects.failedToLoad');
		} finally {
			loading = false;
		}
	}
</script>

{#if loading}
	<div class="loading">{$_('common.loading')}</div>
{:else if error}
	<div class="error">
		<h2>{$_('common.error')}</h2>
		<p>{error}</p>
		<a href={resolveRoute('/')}>{$_('nav.projects')}</a>
	</div>
{:else if $currentProject}
	<div class="project-layout">
		<header class="project-header">
			<div class="project-title">
				<a href={resolveRoute('/')} class="back-link">&larr;</a>
				<h1>{$currentProject.name}</h1>
			</div>
			<nav class="project-nav">
				<a
					href={resolveRoute(`/projects/${$page.params.id}/overview`)}
					class:active={$page.url.pathname.includes('/overview')}
				>
					{$_('overview.title')}
				</a>
				<!-- Disabled as not yet functionnal
                                <a
					href={resolveRoute(`/projects/${$page.params.id}/cashflow`)}
					class:active={$page.url.pathname.includes('/cashflow')}
				>
					{$_('cashflow.title')}
				</a>-->
				<a
					href={resolveRoute(`/projects/${$page.params.id}/payments`)}
					class:active={$page.url.pathname.includes('/payments')}
				>
					{$_('payments.title')}
				</a>
				<a
					href={resolveRoute(`/projects/${$page.params.id}/history`)}
					class:active={$page.url.pathname.includes('/history')}
				>
					{$_('history.title')}
				</a>
				{#if $isAdmin}
					<a
						href={resolveRoute(`/projects/${$page.params.id}/settings`)}
						class:active={$page.url.pathname.includes('/settings')}
					>
						{$_('nav.settings')}
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
		font-size: 1.8rem;
	}

	.project-nav {
		display: flex;
		gap: 0.5rem;
		background: rgba(255, 255, 255, 0.8);
		backdrop-filter: blur(10px);
		border-radius: 12px;
		padding: 0.5rem;
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
	}

	.project-nav a {
		padding: 0.75rem 1.25rem;
		text-decoration: none;
		color: #666;
		font-weight: 600;
		border-radius: 8px;
		transition: all 0.2s;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.project-nav a:hover {
		background: rgba(123, 97, 255, 0.1);
		color: var(--accent, #7b61ff);
	}

	.project-nav a.active {
		background: var(--accent, #7b61ff);
		color: white;
	}

	/* Mobile responsive styles */
	@media (max-width: 768px) {
		.project-header {
			margin-bottom: 1.5rem;
		}

		.project-title {
			gap: 0.75rem;
			margin-bottom: 1rem;
		}

		.back-link {
			width: 32px;
			height: 32px;
			font-size: 1.1rem;
		}

		h1 {
			font-size: 1.3rem;
		}

		.project-nav {
			padding: 0.4rem;
			border-radius: 10px;
		}

		.project-nav a {
			padding: 0.6rem 1rem;
			font-size: 0.9rem;
		}
	}

	@media (max-width: 480px) {
		.project-title {
			gap: 0.5rem;
			margin-bottom: 0.75rem;
		}

		.back-link {
			width: 28px;
			height: 28px;
			font-size: 1rem;
		}

		h1 {
			font-size: 1.1rem;
		}

		.project-nav {
			padding: 0.35rem;
			gap: 0.3rem;
		}

		.project-nav a {
			padding: 0.5rem 0.8rem;
			font-size: 0.8rem;
		}
	}
</style>
