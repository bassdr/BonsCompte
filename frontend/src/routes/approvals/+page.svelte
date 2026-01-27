<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getMyPendingApprovals,
    getActionableApprovals,
    castVote,
    getPendingRecoveries,
    voteOnRecovery,
    type ProjectApproval,
    type PendingRecoveryIntent
  } from '$lib/api';
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';

  let myApprovals = $state<ProjectApproval[]>([]);
  let actionableApprovals = $state<ProjectApproval[]>([]);
  let pendingRecoveries = $state<PendingRecoveryIntent[]>([]);
  let loading = $state(true);
  let error = $state('');
  let votingInProgress = $state<Set<number>>(new Set());
  let recoveryVotingInProgress = $state<Set<string>>(new Set());

  async function loadData() {
    try {
      loading = true;
      error = '';
      [myApprovals, actionableApprovals, pendingRecoveries] = await Promise.all([
        getMyPendingApprovals(),
        getActionableApprovals(),
        getPendingRecoveries()
      ]);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load approvals';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  async function handleVote(approvalId: number, vote: 'approve' | 'reject') {
    if (votingInProgress.has(approvalId)) return;

    votingInProgress.add(approvalId);

    try {
      await castVote(approvalId, vote);
      // Refresh data after voting
      await loadData();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to cast vote';
    } finally {
      votingInProgress.delete(approvalId);
    }
  }

  async function handleRecoveryVote(token: string, vote: 'approve' | 'reject') {
    if (recoveryVotingInProgress.has(token)) return;

    recoveryVotingInProgress.add(token);

    try {
      await voteOnRecovery(token, vote);
      // Refresh data after voting
      await loadData();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to cast vote';
    } finally {
      recoveryVotingInProgress.delete(token);
    }
  }

  function viewDetails(id: number) {
    goto(`/approvals/${id}`);
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

  function formatExpiresAt(expiresAt: string): string {
    try {
      const date = new Date(expiresAt.replace(' ', 'T') + 'Z');
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } catch {
      return expiresAt;
    }
  }
</script>

<div class="container">
  <h1>{$_('approvals.title', { default: 'Approvals' })}</h1>

  {#if loading}
    <div class="loading">{$_('common.loading', { default: 'Loading...' })}</div>
  {:else if error}
    <div class="error-message">{error}</div>
  {:else}
    <!-- Recovery Requests Section (trusted users) -->
    {#if pendingRecoveries.length > 0}
      <section class="section">
        <h2>{$_('recovery.pendingRecoveries')}</h2>

        <div class="approvals-grid">
          {#each pendingRecoveries as recovery (recovery.id)}
            <div class="approval-card recovery-card">
              <div class="card-header">
                <div>
                  <h3>{recovery.display_name || recovery.username}</h3>
                  <p class="project-name">@{recovery.username}</p>
                </div>
                <span class="event-badge recovery-badge"
                  >{$_('approvals.event.password_reset', { default: 'Password Reset' })}</span
                >
              </div>

              <p class="recovery-description">
                {$_('recovery.userRequestsRecovery', {
                  values: { username: recovery.display_name || recovery.username }
                })}
              </p>

              <div class="progress-section">
                <div class="progress-bar">
                  <div
                    class="progress-fill"
                    style="width: {Math.min(
                      100,
                      (recovery.approvals_count / recovery.required_approvals) * 100
                    )}%"
                  ></div>
                </div>
                <div class="progress-text">
                  {recovery.approvals_count} / {recovery.required_approvals}
                  {$_('recovery.approvals')}
                </div>
              </div>

              <div class="expires-info">
                {$_('recovery.expiresAt')}: {formatExpiresAt(recovery.expires_at)}
              </div>

              {#if recovery.is_blocked}
                <div class="blocked-notice">
                  {recovery.user_vote === 'reject'
                    ? $_('recovery.blockedByYou')
                    : $_('recovery.blockedByRejection')}
                </div>
              {/if}

              {#if recovery.user_has_voted}
                <div class="voted-status">
                  {$_('recovery.yourVote')}:
                  <span class={recovery.user_vote === 'approve' ? 'vote-approve' : 'vote-reject'}>
                    {recovery.user_vote === 'approve'
                      ? $_('recovery.votedApprove')
                      : $_('recovery.votedReject')}
                  </span>
                </div>
                <!-- Allow changing vote -->
                <div class="change-vote">
                  <button
                    class="btn-change-vote"
                    onclick={() =>
                      handleRecoveryVote(
                        recovery.token,
                        recovery.user_vote === 'approve' ? 'reject' : 'approve'
                      )}
                    disabled={recoveryVotingInProgress.has(recovery.token)}
                  >
                    {recovery.user_vote === 'approve'
                      ? $_('recovery.changeToReject')
                      : $_('recovery.changeToApprove')}
                  </button>
                </div>
              {:else}
                <div class="action-buttons">
                  <button
                    class="btn-approve"
                    onclick={() => handleRecoveryVote(recovery.token, 'approve')}
                    disabled={recoveryVotingInProgress.has(recovery.token)}
                  >
                    {$_('recovery.voteApprove')}
                  </button>
                  <button
                    class="btn-reject"
                    onclick={() => handleRecoveryVote(recovery.token, 'reject')}
                    disabled={recoveryVotingInProgress.has(recovery.token)}
                  >
                    {$_('recovery.voteReject')}
                  </button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- My Pending Approvals Section -->
    <section class="section">
      <h2>{$_('approvals.my_pending', { default: 'My Pending Approvals' })}</h2>

      {#if myApprovals.length === 0}
        <div class="empty-state">
          <p>
            {$_('approvals.no_pending_approvals', { default: 'You have no pending approvals.' })}
          </p>
        </div>
      {:else}
        <div class="approvals-grid">
          {#each myApprovals as approval (approval.id)}
            <div class="approval-card">
              <div class="card-header">
                <h3>{approval.project_name}</h3>
                <span class="event-badge">{getEventTypeLabel(approval.event_type)}</span>
              </div>

              <div class="progress-section">
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
                  {approval.vote_count || 0} / {approval.required_votes || 0} votes
                </div>
              </div>

              <button class="btn-secondary" onclick={() => viewDetails(approval.id)}>
                {$_('common.view_details', { default: 'View Details' })}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Actionable Approvals Section -->
    <section class="section">
      <h2>{$_('approvals.can_vote_on', { default: 'Approvals You Can Vote On' })}</h2>

      {#if actionableApprovals.length === 0}
        <div class="empty-state">
          <p>
            {$_('approvals.no_actionable_approvals', {
              default: 'There are no approvals that need your vote.'
            })}
          </p>
        </div>
      {:else}
        <div class="approvals-grid">
          {#each actionableApprovals as approval (approval.id)}
            <div class="approval-card">
              <div class="card-header">
                <div>
                  <h3>{approval.username || 'User'}</h3>
                  <p class="project-name">{approval.project_name}</p>
                </div>
                <span class="event-badge">{getEventTypeLabel(approval.event_type)}</span>
              </div>

              <div class="progress-section">
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
                  {approval.vote_count || 0} / {approval.required_votes || 0} votes
                </div>
              </div>

              <div class="action-buttons">
                <button
                  class="btn-approve"
                  onclick={() => handleVote(approval.id, 'approve')}
                  disabled={votingInProgress.has(approval.id)}
                >
                  ✓ {$_('approvals.approve', { default: 'Approve' })}
                </button>
                <button
                  class="btn-reject"
                  onclick={() => handleVote(approval.id, 'reject')}
                  disabled={votingInProgress.has(approval.id)}
                >
                  ✗ {$_('approvals.reject', { default: 'Reject' })}
                </button>
              </div>

              <button class="btn-link" onclick={() => viewDetails(approval.id)}>
                {$_('common.view_details', { default: 'View Details' })}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }

  h1 {
    margin-bottom: 2rem;
    color: #111827;
  }

  .section {
    margin-bottom: 3rem;
  }

  .section h2 {
    font-size: 1.25rem;
    font-weight: 600;
    color: #374151;
    margin-bottom: 1rem;
  }

  .approvals-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 1.5rem;
  }

  .approval-card {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    padding: 1.5rem;
    transition: box-shadow 0.2s;
  }

  .approval-card:hover {
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
  }

  .card-header h3 {
    margin: 0 0 0.25rem 0;
    font-size: 1.125rem;
    color: #111827;
  }

  .project-name {
    margin: 0;
    font-size: 0.875rem;
    color: #6b7280;
  }

  .event-badge {
    background: #dbeafe;
    color: #1e40af;
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .progress-section {
    margin-bottom: 1rem;
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: #e5e7eb;
    border-radius: 3px;
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

  .action-buttons {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .btn-approve {
    flex: 1;
    background: #10b981;
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
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
    padding: 0.5rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
  }

  .btn-reject:hover:not(:disabled) {
    background: #dc2626;
  }

  .btn-reject:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    width: 100%;
    background: #f3f4f6;
    color: #374151;
    border: 1px solid #d1d5db;
    padding: 0.5rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
  }

  .btn-secondary:hover {
    background: #e5e7eb;
  }

  .btn-link {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    text-decoration: underline;
    font-size: 0.875rem;
    padding: 0.25rem 0;
    width: 100%;
    text-align: center;
  }

  .btn-link:hover {
    color: #2563eb;
  }

  .empty-state {
    background: #f9fafb;
    border: 2px dashed #d1d5db;
    border-radius: 8px;
    padding: 3rem 2rem;
    text-align: center;
  }

  .empty-state p {
    margin: 0;
    color: #6b7280;
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

  .recovery-card {
    border-left: 4px solid #f59e0b;
  }

  .recovery-badge {
    background: #fef3c7 !important;
    color: #92400e !important;
  }

  .recovery-description {
    color: #6b7280;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .expires-info {
    font-size: 0.75rem;
    color: #9ca3af;
    margin-bottom: 1rem;
  }

  .voted-status {
    background: #f3f4f6;
    padding: 0.75rem;
    border-radius: 4px;
    font-size: 0.875rem;
    text-align: center;
  }

  .vote-approve {
    color: #059669;
    font-weight: 600;
  }

  .vote-reject {
    color: #dc2626;
    font-weight: 600;
  }

  .blocked-notice {
    background: #fef3c7;
    color: #92400e;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    font-size: 0.8rem;
    margin-bottom: 0.75rem;
    text-align: center;
  }

  .change-vote {
    margin-top: 0.5rem;
  }

  .btn-change-vote {
    width: 100%;
    background: #f3f4f6;
    color: #4b5563;
    border: 1px solid #d1d5db;
    padding: 0.4rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
    transition: background 0.2s;
  }

  .btn-change-vote:hover:not(:disabled) {
    background: #e5e7eb;
  }

  .btn-change-vote:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
