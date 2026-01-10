<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getMyPendingApprovals, type ProjectApproval } from '$lib/api';
  import { formatDateWithWeekday } from '$lib/format/date';
  import { _ } from 'svelte-i18n';

  let approvals = $state<ProjectApproval[]>([]);
  let loading = $state(true);
  let error = $state('');
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function loadApprovals() {
    try {
      loading = true;
      error = '';
      approvals = await getMyPendingApprovals();

      // If all approvals are resolved, redirect to home
      // But only if we have approvals - empty means new user pending system approval
      if (approvals.length === 0) {
        // Will be handled by displaying new user message
        loading = false;
        return;
      }
    } catch {
      // If we can't load approvals, user might be a new pending user
      // Don't treat this as an error, just show the new user message
      error = '';
      approvals = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadApprovals();

    // Auto-refresh every 10 seconds
    intervalId = setInterval(loadApprovals, 10000);
  });

  onDestroy(() => {
    if (intervalId) {
      clearInterval(intervalId);
    }
  });

  function getEventTypeLabel(eventType: string): string {
    switch (eventType) {
      case 'password_change':
        return $_('approvals.event.password_change', { default: 'Password Change' });
      case 'password_reset':
        return $_('approvals.event.password_reset', { default: 'Password Reset' });
      default:
        return eventType;
    }
  }
</script>

<div class="container">
  <div class="quarantine-card">
    <div class="icon-warning">⚠️</div>
    <h1>{$_('approvals.pending.title', { default: 'Account Pending Approval' })}</h1>

    <p class="description">
      {$_('approvals.pending.description', {
        default:
          'Your account requires approval from project members before you can continue. This is a security measure to ensure account integrity after sensitive changes.'
      })}
    </p>

    {#if loading}
      <div class="loading">{$_('common.loading', { default: 'Loading...' })}</div>
    {:else if error}
      <div class="error-message">{error}</div>
    {:else if approvals.length === 0}
      <div class="new-user-message">
        <p>
          {$_('approvals.new_user.description', {
            default:
              'Your account is awaiting approval from a system administrator. You will be able to create and join projects once your account is approved.'
          })}
        </p>
        <p class="contact-admin">
          {$_('approvals.new_user.contact', {
            default: 'Please contact the system administrator to request access.'
          })}
        </p>
      </div>
    {:else}
      <div class="approvals-list">
        {#each approvals as approval (approval.id)}
          <div class="approval-card">
            <div class="approval-header">
              <h3>{approval.project_name}</h3>
              <span class="event-badge">{getEventTypeLabel(approval.event_type)}</span>
            </div>

            <div class="approval-status">
              <div class="progress-bar">
                <div
                  class="progress-fill"
                  style="width: {Math.min(
                    100,
                    ((approval.vote_count || 0) / (approval.required_votes || 1)) * 100
                  )}%"
                ></div>
              </div>
              <div class="vote-count">
                {approval.vote_count || 0} / {approval.required_votes || 0}
                {$_('approvals.votes_received', { default: 'votes received' })}
              </div>
            </div>

            {#if approval.votes && approval.votes.length > 0}
              <div class="votes-list">
                <h4>{$_('approvals.votes', { default: 'Votes' })}</h4>
                {#each approval.votes as vote (vote.id)}
                  <div
                    class="vote-item"
                    class:approve={vote.vote === 'approve'}
                    class:reject={vote.vote === 'reject'}
                  >
                    <span class="vote-icon">{vote.vote === 'approve' ? '✓' : '✗'}</span>
                    <span class="voter-name">
                      {vote.voter_display_name || vote.voter_username}
                    </span>
                    <span class="vote-time">
                      {formatDateWithWeekday(vote.voted_at)}
                    </span>
                    {#if vote.reason}
                      <div class="vote-reason">{vote.reason}</div>
                    {/if}
                  </div>
                {/each}
              </div>
            {:else}
              <div class="no-votes">
                {$_('approvals.no_votes_yet', { default: 'No votes yet' })}
              </div>
            {/if}

            {#if !approval.can_self_approve}
              <div class="info-message">
                {$_('approvals.solo_project', {
                  default: 'This is a solo project. Only a system administrator can approve.'
                })}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      <div class="info-box">
        <p>
          {$_('approvals.pending.help', {
            default:
              'Contact your project administrators to request approval. The page will auto-refresh every 10 seconds.'
          })}
        </p>
      </div>
    {/if}
  </div>
</div>

<style>
  .container {
    max-width: 800px;
    margin: 2rem auto;
    padding: 0 1rem;
  }

  .quarantine-card {
    background: white;
    border-radius: 8px;
    padding: 2rem;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    text-align: center;
  }

  .icon-warning {
    font-size: 4rem;
    margin-bottom: 1rem;
  }

  h1 {
    color: #d97706;
    margin-bottom: 1rem;
  }

  .description {
    color: #6b7280;
    margin-bottom: 2rem;
    line-height: 1.6;
  }

  .approvals-list {
    margin: 2rem 0;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .approval-card {
    border: 1px solid #e5e7eb;
    border-radius: 6px;
    padding: 1.5rem;
    text-align: left;
    background: #f9fafb;
  }

  .approval-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .approval-header h3 {
    margin: 0;
    font-size: 1.25rem;
  }

  .event-badge {
    background: #dbeafe;
    color: #1e40af;
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .approval-status {
    margin-bottom: 1rem;
  }

  .progress-bar {
    width: 100%;
    height: 8px;
    background: #e5e7eb;
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.5rem;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #2563eb);
    transition: width 0.3s ease;
  }

  .vote-count {
    font-size: 0.875rem;
    color: #6b7280;
  }

  .votes-list h4 {
    font-size: 0.875rem;
    font-weight: 600;
    color: #374151;
    margin-bottom: 0.5rem;
  }

  .vote-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    background: white;
    border-radius: 4px;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }

  .vote-icon {
    font-weight: bold;
  }

  .vote-item.approve .vote-icon {
    color: #10b981;
  }

  .vote-item.reject .vote-icon {
    color: #ef4444;
  }

  .voter-name {
    font-weight: 500;
    color: #374151;
  }

  .vote-time {
    color: #9ca3af;
    font-size: 0.75rem;
  }

  .vote-reason {
    grid-column: 2 / -1;
    color: #6b7280;
    font-style: italic;
    font-size: 0.75rem;
    margin-top: 0.25rem;
  }

  .no-votes {
    color: #9ca3af;
    font-style: italic;
    font-size: 0.875rem;
    padding: 1rem;
  }

  .info-message {
    background: #fef3c7;
    color: #92400e;
    padding: 0.75rem;
    border-radius: 4px;
    font-size: 0.875rem;
    margin-top: 1rem;
  }

  .info-box {
    background: #eff6ff;
    border-left: 4px solid #3b82f6;
    padding: 1rem;
    border-radius: 4px;
    margin-top: 2rem;
    text-align: left;
  }

  .info-box p {
    margin: 0;
    color: #1e40af;
    font-size: 0.875rem;
  }

  .loading,
  .error-message {
    padding: 1rem;
    text-align: center;
  }

  .error-message {
    color: #dc2626;
    background: #fee2e2;
    border-radius: 4px;
  }

  .new-user-message {
    background: #eff6ff;
    border-left: 4px solid #3b82f6;
    padding: 1.5rem;
    border-radius: 4px;
    margin: 2rem 0;
    text-align: left;
  }

  .new-user-message p {
    margin: 0.75rem 0;
    color: #1e40af;
    line-height: 1.6;
  }

  .contact-admin {
    font-weight: 500;
    margin-top: 1rem;
  }
</style>
