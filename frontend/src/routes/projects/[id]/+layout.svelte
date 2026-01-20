<script lang="ts">
  import { page } from '$app/stores';
  import { resolve } from '$app/paths';
  import { loadProject, clearProject, currentProject, isAdmin } from '$lib/stores/project';
  import type { Snippet } from 'svelte';
  import { _ } from '$lib/i18n';
  import { formatDateWithWeekday, getLocalDateString } from '$lib/format';
  import { SvelteMap } from 'svelte/reactivity';
  import { getDebts, type DebtSummary } from '$lib/api';

  import { participants } from '$lib/stores/project';

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

  // Warning pool stats: compute when each pool or user will go negative within the warning horizon
  // This is independent of the current view and always computed from today to warning horizon
  interface UserWarning {
    participantId: number;
    participantName: string;
    firstNegativeDate: string;
  }

  interface WarningPoolStats {
    poolTotalNegative: boolean;
    poolFirstNegativeDate: string | null;
    poolName: string;
    userWarnings: UserWarning[];
  }

  // Warning horizon: compute end date based on project settings
  function getWarningHorizonEndDate(setting: string): string {
    const now = new Date();
    switch (setting) {
      case 'end_of_current_month':
        // Last day of current month
        return getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 1, 0));
      case '3_months':
        return getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 3, now.getDay()));
      case '6_months':
        return getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 6, now.getDay()));
      case 'end_of_next_month':
      default:
        // Last day of next month
        return getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 2, 0));
    }
  }

  // Warning horizons: compute the maximum horizon needed across all pools
  let maxWarningHorizonEndDate = $derived.by(() => {
    const poolParticipants = $participants.filter((p) => p.account_type === 'pool');
    if (poolParticipants.length === 0) return null;

    const horizons: string[] = [];
    for (const pool of poolParticipants) {
      if (pool.warning_horizon_account) horizons.push(pool.warning_horizon_account);
      if (pool.warning_horizon_users) horizons.push(pool.warning_horizon_users);
    }

    if (horizons.length === 0) return null;

    // Convert all horizons to dates and find the maximum
    const dates = horizons.map((h) => getWarningHorizonEndDate(h));
    return dates.reduce((max, date) => (date > max ? date : max));
  });

  // Warning debts: loaded separately for the warning period (today to warning horizon)
  let warningDebts: DebtSummary | null = $state(null);
  let warningStartDebts: DebtSummary | null = $state(null); // Debts at today for starting balance
  let loadingWarningDebts = $state(false);

  let warningPoolStats = $derived.by(() => {
    const result = new SvelteMap<number, WarningPoolStats>();

    if (!warningDebts?.pool_ownerships || !warningStartDebts?.pool_ownerships) {
      return result;
    }

    const today = getLocalDateString();

    for (const pool of warningDebts.pool_ownerships) {
      const startPoolData = warningStartDebts.pool_ownerships.find(
        (p) => p.pool_id === pool.pool_id
      );

      // Get pool participant settings
      const poolParticipant = $participants.find((p) => p.id === pool.pool_id);
      if (!poolParticipant) continue;

      // === Pool Total Warning ===
      // Track both actual balance and expected minimum (dual ledger)
      let runningBalance = startPoolData?.total_balance ?? 0;
      let runningExpectedMinimum = startPoolData?.expected_minimum ?? 0;
      let poolTotalNegative = false;
      let poolFirstNegativeDate: string | null = null;

      // Only calculate if account warning is enabled
      if (poolParticipant.warning_horizon_account) {
        const accountHorizonEnd = getWarningHorizonEndDate(poolParticipant.warning_horizon_account);

        // Check if already below expected minimum today
        if (runningBalance < runningExpectedMinimum) {
          poolTotalNegative = true;
          poolFirstNegativeDate = today;
        } else {
          const poolOccurrences = warningDebts.occurrences
            .filter(
              (occ) =>
                occ.occurrence_date > today &&
                occ.occurrence_date <= accountHorizonEnd &&
                (occ.payer_id === pool.pool_id || occ.receiver_account_id === pool.pool_id)
            )
            .sort((a, b) => a.occurrence_date.localeCompare(b.occurrence_date));

          for (const occ of poolOccurrences) {
            // Update actual balance (only if affects_balance is true)
            if (occ.affects_balance) {
              if (occ.payer_id === pool.pool_id) {
                runningBalance -= occ.amount;
              }
              if (occ.receiver_account_id === pool.pool_id) {
                runningBalance += occ.amount;
              }
            }

            // Update expected minimum based on separate payer/receiver expectation flags
            // If payer is this pool and affects_payer_expectation is true → decrease expected minimum
            if (occ.payer_id === pool.pool_id && occ.affects_payer_expectation) {
              runningExpectedMinimum -= occ.amount;
            }
            // If receiver is this pool and affects_receiver_expectation is true → increase expected minimum
            if (occ.receiver_account_id === pool.pool_id && occ.affects_receiver_expectation) {
              runningExpectedMinimum += occ.amount;
            }

            // Warn when balance drops below expected minimum
            if (runningBalance < runningExpectedMinimum) {
              poolTotalNegative = true;
              poolFirstNegativeDate = occ.occurrence_date;
              break;
            }
          }
        }
      }

      // === Per-User Warnings ===
      const userWarnings: UserWarning[] = [];

      // Only calculate if user warning is enabled
      if (poolParticipant.warning_horizon_users) {
        const usersHorizonEnd = getWarningHorizonEndDate(poolParticipant.warning_horizon_users);

        for (const entry of pool.entries) {
          const startEntry = startPoolData?.entries.find(
            (e) => e.participant_id === entry.participant_id
          );

          let userBalance = startEntry?.ownership ?? 0;

          // Already negative today
          if (userBalance < 0) {
            userWarnings.push({
              participantId: entry.participant_id,
              participantName: entry.participant_name,
              firstNegativeDate: today
            });
            continue;
          }

          // Track ALL ownership changes using the breakdown data
          // Contributed breakdown: user deposits to pool (ownership increases)
          // Consumed breakdown: pool pays expenses (ownership decreases by user's share)
          const ownershipChanges: { date: string; delta: number }[] = [];

          // Add contributions (ownership increases)
          for (const contrib of entry.contributed_breakdown) {
            if (contrib.occurrence_date > today && contrib.occurrence_date <= usersHorizonEnd) {
              ownershipChanges.push({
                date: contrib.occurrence_date,
                delta: contrib.amount
              });
            }
          }

          // Add consumption (ownership decreases)
          for (const consumed of entry.consumed_breakdown) {
            if (consumed.occurrence_date > today && consumed.occurrence_date <= usersHorizonEnd) {
              ownershipChanges.push({
                date: consumed.occurrence_date,
                delta: -consumed.amount
              });
            }
          }

          // Sort by date to process chronologically
          ownershipChanges.sort((a, b) => a.date.localeCompare(b.date));

          // Find first date where ownership goes negative
          let firstNegativeDate: string | null = null;
          for (const change of ownershipChanges) {
            userBalance += change.delta;
            if (userBalance < 0 && !firstNegativeDate) {
              firstNegativeDate = change.date;
              break;
            }
          }

          if (firstNegativeDate) {
            userWarnings.push({
              participantId: entry.participant_id,
              participantName: entry.participant_name,
              firstNegativeDate
            });
          }
        }
      }
      result.set(pool.pool_id, {
        poolTotalNegative,
        poolFirstNegativeDate,
        poolName: pool.pool_name,
        userWarnings
      });
    }

    return result;
  });

  let projectId = $derived(parseInt($page.params.id ?? ''));

  // Load warning debts separately (from today to warning horizon)
  async function loadWarningDebts() {
    if (!projectId || !maxWarningHorizonEndDate) return;

    const today = getLocalDateString();
    // Don't load if warning horizon is in the past
    if (maxWarningHorizonEndDate <= today) {
      warningDebts = null;
      warningStartDebts = null;
      return;
    }

    loadingWarningDebts = true;
    try {
      // Load debts at today (for starting balance) and at max warning horizon end date
      // Both exclude drafts for warning calculation
      const [startDebts, endDebts] = await Promise.all([
        getDebts(projectId, today, false),
        getDebts(projectId, maxWarningHorizonEndDate, false)
      ]);
      warningStartDebts = startDebts;
      warningDebts = endDebts;
    } catch (e) {
      console.error('Failed to load warning debts:', e);
      warningDebts = null;
      warningStartDebts = null;
    } finally {
      loadingWarningDebts = false;
    }
  }

  // Effect to load warning debts when project or warning horizon changes
  $effect(() => {
    if (projectId && maxWarningHorizonEndDate) {
      loadWarningDebts();
    }
  });
</script>

{#if loading}
  <div class="loading">{$_('common.loading')}</div>
{:else if error}
  <div class="error">
    <h2>{$_('common.error')}</h2>
    <p>{error}</p>
    <a href={resolve('/')}>{$_('nav.projects')}</a>
  </div>
{:else if $currentProject}
  <div class="project-layout">
    <header class="project-header">
      <div class="project-title">
        <a href={resolve('/')} class="back-link">&larr;</a>
        <h1>{$currentProject.name}</h1>
      </div>

      <!-- Pool Warning Banners (always visible, independent of range mode) -->
      {#if loadingWarningDebts}
        <div class="pool-warning-banner loading">
          <span class="warning-icon">⏳</span>
          <span class="warning-text">{$_('common.loading')}</span>
        </div>
      {:else}
        {#each [...warningPoolStats.entries()] as [poolId, warnStats] (poolId)}
          {@const hasAnyWarning = warnStats.poolTotalNegative || warnStats.userWarnings.length > 0}
          {#if hasAnyWarning}
            <div class="pool-warning-banner" title={$_('overview.warningPeriod')}>
              <span class="warning-icon">⚠️</span>
              <div class="warning-content">
                {#if warnStats.poolTotalNegative && warnStats.poolFirstNegativeDate}
                  <span class="warning-text">
                    {#if warnStats.poolFirstNegativeDate <= getLocalDateString()}
                      {$_('overview.poolIsNegative', { values: { pool: warnStats.poolName } })}
                    {:else}
                      {$_('overview.poolWillGoNegative', {
                        values: {
                          pool: warnStats.poolName,
                          date: formatDateWithWeekday(warnStats.poolFirstNegativeDate)
                        }
                      })}
                    {/if}
                  </span>
                {/if}
                {#each warnStats.userWarnings as userWarn (userWarn.participantId)}
                  <span class="warning-text">
                    {#if userWarn.firstNegativeDate <= getLocalDateString()}
                      {$_('overview.userIsNegative', {
                        values: {
                          user: userWarn.participantName,
                          pool: warnStats.poolName
                        }
                      })}
                    {:else}
                      {$_('overview.userWillGoNegative', {
                        values: {
                          user: userWarn.participantName,
                          pool: warnStats.poolName,
                          date: formatDateWithWeekday(userWarn.firstNegativeDate)
                        }
                      })}
                    {/if}
                  </span>
                {/each}
              </div>
            </div>
          {/if}
        {/each}
      {/if}
      <nav class="project-nav">
        <a
          href={resolve(`/projects/${$page.params.id}/overview`)}
          class:active={$page.url.pathname.includes('/overview')}
        >
          {$_('overview.title')}
        </a>
        <!-- Not yet functionnal
        <a
          href={resolve(`/projects/${$page.params.id}/cashflow`)}
          class:active={$page.url.pathname.includes('/cashflow')}
        >
          {$_('cashflow.title')}
        </a>
        -->
        <a
          href={resolve(`/projects/${$page.params.id}/transactions`)}
          class:active={$page.url.pathname.includes('/transactions')}
        >
          {$_('nav.transactions')}
        </a>
        <a
          href={resolve(`/projects/${$page.params.id}/history`)}
          class:active={$page.url.pathname.includes('/history')}
        >
          {$_('history.title')}
        </a>
        {#if $isAdmin}
          <a
            href={resolve(`/projects/${$page.params.id}/settings`)}
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

  /* Always-visible warning banner (clickable) */
  .pool-warning-banner {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    background: linear-gradient(135deg, rgba(220, 53, 69, 0.12), rgba(220, 53, 69, 0.06));
    color: #dc3545;
    padding: 0.75rem 1rem;
    border: 1px solid rgba(220, 53, 69, 0.3);
    border-radius: 8px;
    margin-bottom: 1rem;
    font-weight: 500;
  }

  .pool-warning-banner:active {
    transform: translateY(0);
  }

  .pool-warning-banner.loading {
    background: linear-gradient(135deg, rgba(108, 117, 125, 0.12), rgba(108, 117, 125, 0.06));
    border-color: rgba(108, 117, 125, 0.3);
    color: #6c757d;
    cursor: default;
  }

  .pool-warning-banner.loading:hover {
    background: linear-gradient(135deg, rgba(108, 117, 125, 0.12), rgba(108, 117, 125, 0.06));
    transform: none;
  }

  .pool-warning-banner .warning-icon {
    flex-shrink: 0;
  }

  .pool-warning-banner .warning-content {
    flex-grow: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    text-align: left;
  }

  .pool-warning-banner .warning-text {
    display: block;
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
