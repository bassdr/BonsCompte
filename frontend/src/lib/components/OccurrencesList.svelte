<script lang="ts">
  import { _ } from '$lib/i18n';
  import { participants } from '$lib/stores/project';
  import { formatCurrency } from '$lib/format/currency';
  import TransactionCard from '$lib/components/TransactionCard.svelte';
  import { SvelteSet, SvelteMap } from 'svelte/reactivity';
  import type { PaymentOccurrence, PaymentWithContributions } from '$lib/api';
  import {
    paymentInvolvesFocus,
    getYear,
    getMonthKey,
    formatMonthYear,
    getYearTotal,
    getTransactionCount,
    getYearTransactionCount,
    getYearParticipantTotals,
    type OccurrencesByYearData
  } from '$lib/date-projection';

  let {
    occurrences,
    paymentMap,
    focusParticipantId = null,
    formatDate,
    title
  }: {
    occurrences: PaymentOccurrence[];
    paymentMap: SvelteMap<number, PaymentWithContributions>;
    focusParticipantId?: number | null;
    formatDate: (dateStr: string) => string;
    title?: string;
  } = $props();

  // ─── Local expand/collapse state (decoupled from data grouping) ────────

  let showOccurrences = $state(false);
  let expandedYears = new SvelteSet<string>();
  let expandedMonths = new SvelteMap<string, SvelteSet<string>>();

  // ─── Grouping logic (pure data, no UI state) ──────────────────────────

  const groupedOccurrences = $derived.by(() => {
    let occurrencesToShow = occurrences;
    if (focusParticipantId !== null) {
      occurrencesToShow = occurrences.filter((occ) =>
        paymentInvolvesFocus(occ.payment_id, occ.payer_id, focusParticipantId, paymentMap)
      );
    }

    const reversed = [...occurrencesToShow].reverse();
    const yearMap = new SvelteMap<string, OccurrencesByYearData>();

    for (const occ of reversed) {
      const year = getYear(occ.occurrence_date);
      const monthKeyStr = getMonthKey(occ.occurrence_date);
      const dateStr = occ.occurrence_date;

      if (!yearMap.has(year)) {
        yearMap.set(year, {
          year,
          expanded: false, // not used; expand state read directly from expandedYears
          months: new SvelteMap()
        });
      }

      const yearData = yearMap.get(year)!;
      if (!yearData.months.has(monthKeyStr)) {
        yearData.months.set(monthKeyStr, {
          monthKey: monthKeyStr,
          month: formatMonthYear(dateStr),
          expanded: false, // not used; expand state read directly from expandedMonths
          totalAmount: 0,
          dates: new SvelteMap(),
          participantTotals: new SvelteMap<string, number>()
        });
      }

      const monthData = yearData.months.get(monthKeyStr)!;
      if (!monthData.dates.has(dateStr)) {
        monthData.dates.set(dateStr, {
          date: dateStr,
          dateDisplay: formatDate(dateStr),
          occurrences: [],
          totalAmount: 0
        });
      }

      const dateData = monthData.dates.get(dateStr)!;
      dateData.occurrences.push(occ);
      dateData.totalAmount += occ.amount;
      monthData.totalAmount += occ.amount;

      const payment = paymentMap.get(occ.payment_id);
      if (payment && payment.contributions) {
        for (const contrib of payment.contributions) {
          const current = monthData.participantTotals.get(contrib.participant_name) || 0;
          monthData.participantTotals.set(contrib.participant_name, current + contrib.amount);
        }
      }
    }

    return yearMap;
  });

  // Auto-expand first year/month when data arrives
  let autoExpandedOnce = $state(false);
  let prevOccurrencesLength = $state(0);

  $effect(() => {
    const len = occurrences.length;
    if (len !== prevOccurrencesLength) {
      prevOccurrencesLength = len;
      autoExpandedOnce = false;
    }
  });

  $effect(() => {
    if (groupedOccurrences.size > 0 && !autoExpandedOnce) {
      const firstYearEntry = [...groupedOccurrences.entries()][0];
      if (firstYearEntry) {
        const [year, yearData] = firstYearEntry;
        expandedYears.clear();
        expandedYears.add(year);

        const firstMonthEntry = [...yearData.months.entries()][0];
        if (firstMonthEntry) {
          const [monthKey] = firstMonthEntry;
          expandedMonths.clear();
          expandedMonths.set(year, new SvelteSet([monthKey]));
        }
        autoExpandedOnce = true;
      }
    }
  });

  // ─── Toggle helpers ──────────────────────────────────────────────────────

  function toggleYear(year: string) {
    if (expandedYears.has(year)) {
      expandedYears.delete(year);
    } else {
      expandedYears.add(year);
    }
  }

  function toggleMonth(year: string, monthKeyStr: string) {
    if (!expandedMonths.has(year)) {
      expandedMonths.set(year, new SvelteSet());
    }
    const monthSet = expandedMonths.get(year)!;
    if (monthSet.has(monthKeyStr)) {
      monthSet.delete(monthKeyStr);
    } else {
      monthSet.add(monthKeyStr);
    }
  }

  // ─── Derived counts ──────────────────────────────────────────────────────

  const totalCount = $derived.by(() => {
    if (focusParticipantId === null) return occurrences.length;
    return occurrences.filter((occ) =>
      paymentInvolvesFocus(occ.payment_id, occ.payer_id, focusParticipantId, paymentMap)
    ).length;
  });
</script>

{#if occurrences.length > 0}
  <section class="card occurrences-card">
    <button class="expand-header" onclick={() => (showOccurrences = !showOccurrences)}>
      <span class="expand-icon">{showOccurrences ? '▼' : '▶'}</span>
      <h3>{title ?? $_('overview.transactionsIncluded')} ({totalCount})</h3>
    </button>

    {#if showOccurrences}
      <div class="hierarchy-container">
        {#each [...groupedOccurrences.entries()] as [year, yearData] (year)}
          <div class="year-group">
            <button class="year-header" onclick={() => toggleYear(year)}>
              {#if expandedYears.has(year)}
                <span class="expand-icon">▼</span>
                <span class="year-label">{year}</span>
              {:else}
                <div class="year-summary-box">
                  <div class="year-summary-header">
                    <span class="expand-icon">▶</span>
                    <span class="year-label">{year}</span>
                    <span class="year-total">{formatCurrency(getYearTotal(yearData))}</span>
                  </div>
                  <div class="year-summary-meta">
                    {getYearTransactionCount(yearData)}
                    {getYearTransactionCount(yearData) === 1
                      ? $_('overview.transaction')
                      : $_('overview.transactions')}
                  </div>
                  <div class="year-summary-participants">
                    {#each [...getYearParticipantTotals(yearData).entries()].sort((a, b) => b[1] - a[1]) as [participantName, amount] (participantName)}
                      <span class="participant-chip"
                        >{participantName}: {formatCurrency(amount)}</span
                      >
                    {/each}
                  </div>
                </div>
              {/if}
            </button>

            {#if expandedYears.has(year)}
              {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                <div class="month-group">
                  <button class="month-header" onclick={() => toggleMonth(year, monthKey)}>
                    {#if expandedMonths.get(year)?.has(monthKey)}
                      <span class="expand-icon">▼</span>
                      <span class="month-label">{monthData.month}</span>
                    {:else}
                      <div class="month-summary-box">
                        <div class="month-summary-header">
                          <span class="expand-icon">▶</span>
                          <span class="month-label">{monthData.month}</span>
                          <span class="month-total">{formatCurrency(monthData.totalAmount)}</span>
                        </div>
                        <div class="month-summary-meta">
                          {getTransactionCount(monthData)}
                          {getTransactionCount(monthData) === 1
                            ? $_('overview.transaction')
                            : $_('overview.transactions')}
                        </div>
                        <div class="month-summary-participants">
                          {#each [...monthData.participantTotals.entries()].sort((a, b) => b[1] - a[1]) as [participantName, amount] (participantName)}
                            <span class="participant-chip"
                              >{participantName}: {formatCurrency(amount)}</span
                            >
                          {/each}
                        </div>
                      </div>
                    {/if}
                  </button>

                  {#if expandedMonths.get(year)?.has(monthKey)}
                    {#each [...monthData.dates.entries()] as [_dateKey, dateData] (dateData.date)}
                      <div class="date-group">
                        <div class="date-label">
                          {formatDate(dateData.date)}
                          <span class="date-total">{formatCurrency(dateData.totalAmount)}</span>
                        </div>

                        <div class="transactions">
                          {#each dateData.occurrences as occ (occ.payment_id)}
                            {@const payment = paymentMap.get(occ.payment_id)}
                            {@const receiver = occ.receiver_account_id
                              ? $participants.find((pr) => pr.id === occ.receiver_account_id)
                              : null}
                            <TransactionCard
                              description={occ.description}
                              amount={occ.amount}
                              date={occ.occurrence_date}
                              originalDate={payment?.payment_date}
                              payerName={payment?.payer_name}
                              payerId={occ.payer_id}
                              receiverName={receiver?.name}
                              receiverAccountId={occ.receiver_account_id}
                              isFinal={occ.is_final}
                              affectsBalance={occ.affects_balance}
                              affectsPayerExpectation={occ.affects_payer_expectation}
                              affectsReceiverExpectation={occ.affects_receiver_expectation}
                              isRecurring={payment?.is_recurring ?? false}
                              recurrenceType={payment?.recurrence_type}
                              recurrenceInterval={payment?.recurrence_interval}
                              recurrenceEndDate={payment?.recurrence_end_date}
                              recurrenceWeekdays={payment?.recurrence_weekdays}
                              recurrenceMonthdays={payment?.recurrence_monthdays}
                              recurrenceMonths={payment?.recurrence_months}
                              contributions={payment?.contributions ?? []}
                            />
                          {/each}
                        </div>
                      </div>
                    {/each}
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .occurrences-card {
    padding: 0;
  }

  .expand-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 1.5rem;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
  }

  .expand-header:hover {
    background: rgba(0, 0, 0, 0.02);
  }

  .expand-header h3 {
    margin: 0;
    font-size: 1rem;
    color: var(--accent, #7b61ff);
  }

  .expand-icon {
    color: var(--accent, #7b61ff);
    font-size: 0.8rem;
    flex: 0 0 auto;
  }

  .hierarchy-container {
    padding: 0 1.5rem 1.5rem;
  }

  .year-group {
    margin-bottom: 0;
  }

  .year-header {
    display: block;
    width: 100%;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
  }

  .year-header:hover {
    background: rgba(123, 97, 255, 0.03);
  }

  .year-summary-box {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 1rem 0;
    border-bottom: 2px solid #f0f0f0;
  }

  .year-summary-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .year-label {
    flex: 1;
    font-weight: 600;
    font-size: 1.1rem;
    color: var(--accent, #7b61ff);
  }

  .year-total {
    font-weight: 700;
    color: #2a9d8f;
    font-size: 1.1rem;
    white-space: nowrap;
  }

  .year-summary-meta {
    font-size: 0.8rem;
    color: #999;
    padding-left: 1.5rem;
  }

  .year-summary-participants {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding-left: 1.5rem;
  }

  .month-group {
    margin-bottom: 0;
    padding-left: 1.5rem;
  }

  .month-header {
    display: block;
    width: 100%;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
  }

  .month-header:hover {
    background: rgba(0, 0, 0, 0.01);
  }

  .month-summary-box {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem 0;
    border-bottom: 1px solid #f5f5f5;
  }

  .month-summary-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .month-label {
    flex: 1;
    font-weight: 500;
    font-size: 1rem;
    color: #333;
  }

  .month-total {
    font-weight: 700;
    color: #2a9d8f;
    font-size: 1rem;
    white-space: nowrap;
  }

  .month-summary-meta {
    font-size: 0.8rem;
    color: #999;
    padding-left: 1.5rem;
  }

  .month-summary-participants {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding-left: 1.5rem;
  }

  .date-group {
    padding-left: 1.5rem;
    margin-bottom: 1rem;
  }

  .date-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
    font-weight: 500;
    color: #555;
    font-size: 0.95rem;
    border-bottom: 1px solid #eee;
    margin-bottom: 0.5rem;
  }

  .date-total {
    font-weight: 600;
    color: var(--accent, #7b61ff);
  }

  .transactions {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .participant-chip {
    padding: 0.3rem 0.6rem;
    background: #f0ebe5;
    border-radius: 12px;
    font-size: 0.75rem;
    color: #666;
    font-weight: 500;
    display: inline-block;
  }
</style>
