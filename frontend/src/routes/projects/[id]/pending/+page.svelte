<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { getProject, leaveProject } from '$lib/api';
  import { _ } from 'svelte-i18n';

  let projectId = $derived(Number($page.params.id));
  let loading = $state(true);
  let leaving = $state(false);
  let errorKey = $state('');
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function checkAccess() {
    try {
      // Try to access the project - if successful, user has been approved
      await getProject(projectId);
      // User has access now, redirect to project
      if (browser) {
        goto(`/projects/${projectId}/overview`);
      }
    } catch {
      // If we get MEMBERSHIP_PENDING, we're still pending - that's expected
      // The redirect in api.ts is disabled here because we're already on the pending page
      // For other errors, stay on this page
      loading = false;
    }
  }

  async function handleCancelRequest() {
    if (leaving) return;

    leaving = true;
    errorKey = '';

    try {
      await leaveProject(projectId);
      // Successfully cancelled, redirect to projects list
      if (browser) {
        goto('/');
      }
    } catch {
      errorKey = 'projectPending.cancelFailed';
      leaving = false;
    }
  }

  onMount(() => {
    checkAccess();

    // Auto-refresh every 5 seconds to check if approved
    intervalId = setInterval(checkAccess, 5000);
  });

  onDestroy(() => {
    if (intervalId) {
      clearInterval(intervalId);
    }
  });
</script>

<div class="container">
  <div class="pending-card">
    <div class="icon-clock">&#128337;</div>
    <h1>{$_('projectPending.title', { default: 'Membership Pending' })}</h1>

    {#if loading}
      <div class="loading">{$_('common.loading', { default: 'Loading...' })}</div>
    {:else}
      <p class="description">
        {$_('projectPending.description', {
          default:
            'Your request to join this project is pending approval from an administrator. You will be redirected automatically once approved.'
        })}
      </p>

      <div class="info-box">
        <p>
          {$_('projectPending.autoRefresh', {
            default: 'This page will automatically check for approval every 5 seconds.'
          })}
        </p>
      </div>

      {#if errorKey}
        <div class="error-message">{$_(errorKey)}</div>
      {/if}

      <div class="actions">
        <button class="btn-secondary" onclick={handleCancelRequest} disabled={leaving}>
          {leaving
            ? $_('projectPending.cancelling', { default: 'Cancelling...' })
            : $_('projectPending.cancelRequest', { default: 'Cancel Request' })}
        </button>

        <a href="/" class="btn-link">
          {$_('projectPending.backToProjects', { default: 'Back to Projects' })}
        </a>
      </div>
    {/if}
  </div>
</div>

<style>
  .container {
    max-width: 600px;
    margin: 2rem auto;
    padding: 0 1rem;
  }

  .pending-card {
    background: white;
    border-radius: 8px;
    padding: 2rem;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    text-align: center;
  }

  .icon-clock {
    font-size: 4rem;
    margin-bottom: 1rem;
  }

  h1 {
    color: #3b82f6;
    margin-bottom: 1rem;
  }

  .description {
    color: #6b7280;
    margin-bottom: 1.5rem;
    line-height: 1.6;
  }

  .info-box {
    background: #eff6ff;
    border-left: 4px solid #3b82f6;
    padding: 1rem;
    border-radius: 4px;
    margin-bottom: 1.5rem;
    text-align: left;
  }

  .info-box p {
    margin: 0;
    color: #1e40af;
    font-size: 0.875rem;
  }

  .error-message {
    color: #dc2626;
    background: #fee2e2;
    padding: 0.75rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    align-items: center;
  }

  .btn-secondary {
    padding: 0.75rem 1.5rem;
    background: #f3f4f6;
    color: #374151;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 1rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .btn-secondary:hover:not(:disabled) {
    background: #e5e7eb;
  }

  .btn-secondary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-link {
    color: #3b82f6;
    text-decoration: none;
    font-size: 0.9rem;
  }

  .btn-link:hover {
    text-decoration: underline;
  }

  .loading {
    padding: 1rem;
    color: #6b7280;
  }
</style>
