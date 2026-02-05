<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import Check from '@lucide/svelte/icons/check';
  import X from '@lucide/svelte/icons/x';
  import { getApproval, castVote, type ProjectApproval } from '$lib/api';
  import { formatDateWithWeekday } from '$lib/format/date';
  import { _ } from 'svelte-i18n';
  import { getErrorKey } from '$lib/errors';

  const approvalId = Number($page.params.id);

  let approval = $state<ProjectApproval | null>(null);
  let loading = $state(true);
  let errorKey = $state('');
  let voting = $state(false);
  let voteReason = $state('');

  async function loadApproval() {
    try {
      loading = true;
      errorKey = '';
      approval = await getApproval(approvalId);
    } catch (e) {
      errorKey = getErrorKey(e, 'approvals.failedToLoad');
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadApproval();
  });

  async function handleVote(vote: 'approve' | 'reject') {
    if (!approval || voting) return;

    voting = true;
    try {
      await castVote(approvalId, vote, voteReason || undefined);
      // Refresh approval data
      await loadApproval();
      voteReason = '';
    } catch (e) {
      errorKey = getErrorKey(e, 'approvals.failedToVote');
    } finally {
      voting = false;
    }
  }

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

  function getStatusBadgeClass(status: string): string {
    switch (status) {
      case 'approved':
        return 'status-approved';
      case 'rejected':
        return 'status-rejected';
      default:
        return 'status-pending';
    }
  }
</script>

<div class="container">
  <button class="btn-back" onclick={() => goto('/approvals')}>
    ← {$_('common.back', { default: 'Back' })}
  </button>

  {#if loading}
    <div class="loading">{$_('common.loading', { default: 'Loading...' })}</div>
  {:else if errorKey}
    <div class="error-message">{$_(errorKey)}</div>
  {:else if approval}
    <div class="approval-detail">
      <div class="header">
        <div>
          <h1>{approval.username || 'User'}</h1>
          <p class="project-name">{approval.project_name}</p>
        </div>
        <div class="badges">
          <span class="event-badge">{getEventTypeLabel(approval.event_type)}</span>
          <span class="status-badge {getStatusBadgeClass(approval.status)}">
            {approval.status}
          </span>
        </div>
      </div>

      <div class="progress-section">
        <h2>{$_('approvals.voting_progress', { default: 'Voting Progress' })}</h2>
        <div class="progress-bar">
          <div
            class="progress-fill"
            style="width: {Math.min(
              100,
              ((approval.vote_count || 0) / (approval.required_votes || 1)) * 100
            )}%"
          ></div>
        </div>
        <div class="progress-text">
          {approval.vote_count || 0} / {approval.required_votes || 0} votes required
        </div>
      </div>

      {#if !approval.can_self_approve}
        <div class="info-message">
          {$_('approvals.solo_project', {
            default: 'This is a solo project. Only a system administrator can approve.'
          })}
        </div>
      {/if}

      <div class="votes-section">
        <h2>{$_('approvals.votes', { default: 'Votes' })}</h2>

        {#if approval.votes && approval.votes.length > 0}
          <div class="votes-list">
            {#each approval.votes as vote (vote.id)}
              <div
                class="vote-card"
                class:approve={vote.vote === 'approve'}
                class:reject={vote.vote === 'reject'}
              >
                <div class="vote-header">
                  <span class="vote-icon">
                    {#if vote.vote === 'approve'}<Check size={24} color="currentColor" />{:else}<X
                        size={24}
                        color="currentColor"
                      />{/if}
                  </span>
                  <div class="voter-info">
                    <div class="voter-name">
                      {vote.voter_display_name || vote.voter_username}
                    </div>
                    <div class="vote-time">
                      {formatDateWithWeekday(vote.voted_at)}
                    </div>
                  </div>
                  <span class="vote-badge">{vote.vote}</span>
                </div>
                {#if vote.reason}
                  <div class="vote-reason">
                    <strong>{$_('approvals.reason', { default: 'Reason' })}:</strong>
                    {vote.reason}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <div class="no-votes">
            {$_('approvals.no_votes_yet', { default: 'No votes yet' })}
          </div>
        {/if}
      </div>

      {#if approval.status === 'pending'}
        <div class="vote-form">
          <h2>{$_('approvals.cast_vote', { default: 'Cast Your Vote' })}</h2>

          <div class="form-group">
            <label for="reason"
              >{$_('approvals.reason_optional', { default: 'Reason (optional)' })}</label
            >
            <textarea
              id="reason"
              bind:value={voteReason}
              placeholder={$_('approvals.reason_placeholder', {
                default: 'Explain your decision...'
              })}
              rows="3"
            ></textarea>
          </div>

          <div class="action-buttons">
            <button class="btn-approve" onclick={() => handleVote('approve')} disabled={voting}>
              <Check size={16} />
              {$_('approvals.approve', { default: 'Approve' })}
            </button>
            <button class="btn-reject" onclick={() => handleVote('reject')} disabled={voting}>
              <X size={16} />
              {$_('approvals.reject', { default: 'Reject' })}
            </button>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }

  .btn-back {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.875rem;
    padding: 0.5rem 0;
    margin-bottom: 1rem;
  }

  .btn-back:hover {
    color: #2563eb;
    text-decoration: underline;
  }

  .approval-detail {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    padding: 2rem;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 2rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
  }

  h1 {
    margin: 0 0 0.5rem 0;
    font-size: 1.75rem;
    color: #111827;
  }

  .project-name {
    margin: 0;
    color: #6b7280;
    font-size: 1rem;
  }

  .badges {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.5rem;
  }

  .event-badge {
    background: #dbeafe;
    color: #1e40af;
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .status-badge {
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
    text-transform: uppercase;
  }

  .status-pending {
    background: #fef3c7;
    color: #92400e;
  }

  .status-approved {
    background: #d1fae5;
    color: #065f46;
  }

  .status-rejected {
    background: #fee2e2;
    color: #991b1b;
  }

  .progress-section {
    margin-bottom: 2rem;
  }

  .progress-section h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #374151;
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

  .progress-text {
    font-size: 0.875rem;
    color: #6b7280;
  }

  .info-message {
    background: #fef3c7;
    color: #92400e;
    padding: 1rem;
    border-radius: 4px;
    font-size: 0.875rem;
    margin-bottom: 2rem;
  }

  .votes-section {
    margin-bottom: 2rem;
  }

  .votes-section h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #374151;
    margin-bottom: 1rem;
  }

  .votes-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .vote-card {
    border: 1px solid #e5e7eb;
    border-radius: 6px;
    padding: 1rem;
    background: #f9fafb;
  }

  .vote-card.approve {
    border-left: 4px solid #10b981;
  }

  .vote-card.reject {
    border-left: 4px solid #ef4444;
  }

  .vote-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .vote-icon {
    display: flex;
    align-items: center;
  }

  .vote-card.approve .vote-icon {
    color: #10b981;
  }

  .vote-card.reject .vote-icon {
    color: #ef4444;
  }

  .voter-info {
    flex: 1;
  }

  .voter-name {
    font-weight: 500;
    color: #111827;
  }

  .vote-time {
    font-size: 0.75rem;
    color: #9ca3af;
  }

  .vote-badge {
    background: white;
    border: 1px solid #d1d5db;
    color: #374151;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
    text-transform: uppercase;
  }

  .vote-reason {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid #e5e7eb;
    color: #6b7280;
    font-size: 0.875rem;
  }

  .no-votes {
    color: #9ca3af;
    font-style: italic;
    padding: 2rem;
    text-align: center;
    background: #f9fafb;
    border-radius: 6px;
  }

  .vote-form {
    padding-top: 1.5rem;
    border-top: 1px solid #e5e7eb;
  }

  .vote-form h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #374151;
    margin-bottom: 1rem;
  }

  .form-group {
    margin-bottom: 1rem;
  }

  label {
    display: block;
    font-weight: 500;
    color: #374151;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }

  textarea {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.875rem;
    resize: vertical;
  }

  textarea:focus {
    outline: none;
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
  }

  .action-buttons {
    display: flex;
    gap: 0.75rem;
  }

  .btn-approve {
    flex: 1;
    background: #10b981;
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
  }

  .btn-approve:hover:not(:disabled) {
    background: #059669;
  }

  .btn-approve:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-reject {
    flex: 1;
    background: #ef4444;
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
  }

  .btn-reject:hover:not(:disabled) {
    background: #dc2626;
  }

  .btn-reject:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .loading,
  .error-message {
    padding: 2rem;
    text-align: center;
  }

  .error-message {
    color: #dc2626;
    background: #fee2e2;
    border-radius: 8px;
  }
</style>
