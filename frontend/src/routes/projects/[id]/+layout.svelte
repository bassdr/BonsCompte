<script lang="ts">
  import { page } from '$app/stores';
  import { resolve } from '$app/paths';
  import { loadProject, clearProject, currentProject, isAdmin } from '$lib/stores/project';
  import type { Snippet } from 'svelte';
  import { _ } from '$lib/i18n';
  import { formatDateWithWeekday, getLocalDateString } from '$lib/format';
  import { SvelteMap } from 'svelte/reactivity';
  import { getDebts, type DebtSummary, type PaymentOccurrence } from '$lib/api';

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
    firstBelowExpectedDate: string; // when ownership < expected_minimum
  }

  interface WarningPoolStats {
    poolTotalNegative: boolean; // balance < expected_minimum
    poolFirstNegativeDate: string | null;
    expectedMinNegative: boolean; // expected_minimum < 0 (configuration error)
    expectedMinFirstNegativeDate: string | null;
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
      let expectedMinNegative = false;
      let expectedMinFirstNegativeDate: string | null = null;

      // Only calculate if account warning is enabled
      if (poolParticipant.warning_horizon_account) {
        const accountHorizonEnd = getWarningHorizonEndDate(poolParticipant.warning_horizon_account);

        // Check if expected minimum is already negative today (configuration error)
        if (runningExpectedMinimum < 0) {
          expectedMinNegative = true;
          expectedMinFirstNegativeDate = today;
        }

        // Check if already below expected minimum today
        if (runningBalance < runningExpectedMinimum) {
          poolTotalNegative = true;
          poolFirstNegativeDate = today;
        }

        // Process future occurrences
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

          // Warn when expected minimum goes negative (configuration error)
          if (!expectedMinNegative && runningExpectedMinimum < 0) {
            expectedMinNegative = true;
            expectedMinFirstNegativeDate = occ.occurrence_date;
          }

          // Warn when balance drops below expected minimum
          if (!poolTotalNegative && runningBalance < runningExpectedMinimum) {
            poolTotalNegative = true;
            poolFirstNegativeDate = occ.occurrence_date;
          }
        }
      }

      // === Per-User Warnings ===
      // Track when user's ownership drops below their expected minimum (similar to pool logic)
      const userWarnings: UserWarning[] = [];

      // Only calculate if user warning is enabled
      if (poolParticipant.warning_horizon_users) {
        const usersHorizonEnd = getWarningHorizonEndDate(poolParticipant.warning_horizon_users);
        const poolTotalOwnership = startPoolData?.total_balance ?? 0;
        const poolExpectedMin = startPoolData?.expected_minimum ?? 0;

        // Build a map of occurrences by payment_id for looking up expectation flags
        const occurrencesByPayment = new SvelteMap<number, PaymentOccurrence[]>();
        for (const occ of warningDebts!.occurrences) {
          const existing = occurrencesByPayment.get(occ.payment_id) || [];
          existing.push(occ);
          occurrencesByPayment.set(occ.payment_id, existing);
        }

        for (const entry of pool.entries) {
          const startEntry = startPoolData?.entries.find(
            (e) => e.participant_id === entry.participant_id
          );

          let userOwnership = startEntry?.ownership ?? 0;
          // Compute user's starting expected minimum proportionally to their ownership share
          // If pool total is 0, use 0 as starting expected minimum
          let userExpectedMin =
            poolTotalOwnership !== 0 ? (userOwnership / poolTotalOwnership) * poolExpectedMin : 0;

          // Already below expected minimum today
          if (userOwnership < userExpectedMin) {
            userWarnings.push({
              participantId: entry.participant_id,
              participantName: entry.participant_name,
              firstBelowExpectedDate: today
            });
            continue;
          }

          // Track ownership and expected minimum changes using breakdown data
          // Contributed = user deposits to pool (ownership increases)
          // Consumed = pool pays expenses (user's share decreases)
          const changes: {
            date: string;
            ownershipDelta: number;
            expectedMinDelta: number;
          }[] = [];

          // Process contributions (deposits to pool)
          for (const contrib of entry.contributed_breakdown) {
            if (contrib.occurrence_date > today && contrib.occurrence_date <= usersHorizonEnd) {
              // Look up the occurrence to check expectation flags
              const occs = occurrencesByPayment.get(contrib.payment_id) || [];
              const occ = occs.find((o) => o.occurrence_date === contrib.occurrence_date);

              const affectsBalance = occ?.affects_balance ?? true;
              const affectsExpectation = occ?.affects_receiver_expectation ?? false;

              changes.push({
                date: contrib.occurrence_date,
                ownershipDelta: affectsBalance ? contrib.amount : 0,
                expectedMinDelta: affectsExpectation ? contrib.amount : 0
              });
            }
          }

          // Process consumption (pool expenses - user's share decreases)
          for (const consumed of entry.consumed_breakdown) {
            if (consumed.occurrence_date > today && consumed.occurrence_date <= usersHorizonEnd) {
              // Look up the occurrence to check expectation flags
              const occs = occurrencesByPayment.get(consumed.payment_id) || [];
              const occ = occs.find((o) => o.occurrence_date === consumed.occurrence_date);

              const affectsBalance = occ?.affects_balance ?? true;
              const affectsExpectation = occ?.affects_payer_expectation ?? false;

              changes.push({
                date: consumed.occurrence_date,
                ownershipDelta: affectsBalance ? -consumed.amount : 0,
                expectedMinDelta: affectsExpectation ? -consumed.amount : 0
              });
            }
          }

          // Sort by date to process chronologically
          changes.sort((a, b) => a.date.localeCompare(b.date));

          // Find first date where ownership goes below expected minimum
          let firstBelowExpectedDate: string | null = null;
          for (const change of changes) {
            userOwnership += change.ownershipDelta;
            userExpectedMin += change.expectedMinDelta;
            if (userOwnership < userExpectedMin && !firstBelowExpectedDate) {
              firstBelowExpectedDate = change.date;
              break;
            }
          }

          if (firstBelowExpectedDate) {
            userWarnings.push({
              participantId: entry.participant_id,
              participantName: entry.participant_name,
              firstBelowExpectedDate
            });
          }
        }
      }
      result.set(pool.pool_id, {
        poolTotalNegative,
        poolFirstNegativeDate,
        expectedMinNegative,
        expectedMinFirstNegativeDate,
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
          {@const hasAnyWarning =
            warnStats.poolTotalNegative ||
            warnStats.expectedMinNegative ||
            warnStats.userWarnings.length > 0}
          {#if hasAnyWarning}
            <div class="pool-warning-banner" title={$_('overview.warningPeriod')}>
              <span class="warning-icon">⚠️</span>
              <div class="warning-content">
                {#if warnStats.expectedMinNegative && warnStats.expectedMinFirstNegativeDate}
                  <span class="warning-text">
                    {#if warnStats.expectedMinFirstNegativeDate <= getLocalDateString()}
                      {$_('overview.expectedMinIsNegative', {
                        values: { pool: warnStats.poolName }
                      })}
                    {:else}
                      {$_('overview.expectedMinWillGoNegative', {
                        values: {
                          pool: warnStats.poolName,
                          date: formatDateWithWeekday(warnStats.expectedMinFirstNegativeDate)
                        }
                      })}
                    {/if}
                  </span>
                {/if}
                {#if warnStats.poolTotalNegative && warnStats.poolFirstNegativeDate}
                  <span class="warning-text">
                    {#if warnStats.poolFirstNegativeDate <= getLocalDateString()}
                      {$_('overview.poolIsBelow', { values: { pool: warnStats.poolName } })}
                    {:else}
                      {$_('overview.poolWillGoBelow', {
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
                    {#if userWarn.firstBelowExpectedDate <= getLocalDateString()}
                      {$_('overview.userIsBelowExpected', {
                        values: {
                          user: userWarn.participantName,
                          pool: warnStats.poolName
                        }
                      })}
                    {:else}
                      {$_('overview.userWillGoBelowExpected', {
                        values: {
                          user: userWarn.participantName,
                          pool: warnStats.poolName,
                          date: formatDateWithWeekday(userWarn.firstBelowExpectedDate)
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
