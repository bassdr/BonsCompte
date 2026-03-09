<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { _ } from '$lib/i18n';
  import { participants } from '$lib/stores/project';
  import { preferences } from '$lib/stores/preferences';
  import { formatCurrency } from '$lib/format/currency';
  import type { PairwiseBalance } from '$lib/api';
  import TransactionCard from '$lib/components/TransactionCard.svelte';
  import DateSelectorCard from '$lib/components/DateSelectorCard.svelte';
  import FocusCard from '$lib/components/FocusCard.svelte';
  import OccurrencesList from '$lib/components/OccurrencesList.svelte';
  import { SvelteSet, SvelteMap } from 'svelte/reactivity';
  import { Chart, registerables } from 'chart.js';
  import 'chartjs-adapter-date-fns';
  import {
    CHART_COLORS,
    getChartDateFormat,
    getDateFnsLocale,
    groupBreakdownByYearMonth,
    settlementInvolvesFocus
  } from '$lib/date-projection';
  import { createOverviewState } from '$lib/date-projection.svelte';

  // Chart.js annotation plugin
  let annotationPlugin: typeof import('chartjs-plugin-annotation').default | null = null;
  let chartReady = $state(false);

  type XYPoint = { x: string; y: number };
  type LineChart = Chart<'line', XYPoint[], unknown>;

  let balanceChart: LineChart | null = null;
  let balanceChartCanvas: HTMLCanvasElement | null = $state(null);

  let projectId = $derived(parseInt(page.params.id ?? ''));

  // Create shared projection state
  const projection = createOverviewState();

  // Settlement mode
  let useDirectSettlements = $state(false);

  // Expanded balance/pairwise rows
  let expandedBalanceRows = new SvelteSet<number>();
  let expandedPairwise = new SvelteSet<string>();
  let expandedPairwiseYears = new SvelteMap<string, SvelteSet<string>>();
  let expandedPairwiseMonths = new SvelteMap<string, SvelteMap<string, SvelteSet<string>>>();

  // Initialize Chart.js (SSR-safe)
  onMount(() => {
    Chart.register(...registerables);

    import('chartjs-plugin-annotation').then((mod) => {
      annotationPlugin = mod.default;
      Chart.register(annotationPlugin);
      chartReady = true;
    });

    return () => {
      if (balanceChart) {
        balanceChart.destroy();
        balanceChart = null;
      }
    };
  });

  // Load data when project/date/range changes
  $effect(() => {
    if (projectId && projection.targetDate) {
      const _endDate = projection.endDate;
      const _includeDrafts = projection.includeDrafts;
      void _endDate;
      void _includeDrafts;
      projection.loadDebts(projectId);
    }
  });

  // ─── Settlement occurrences (non-pool transactions only) ────────────────

  const settlementOccurrences = $derived.by(() => {
    if (!projection.debts?.occurrences) return [];
    return projection.debts.occurrences.filter(
      (occ) =>
        !projection.poolIds.has(occ.payer_id ?? -1) &&
        !projection.poolIds.has(occ.receiver_account_id ?? -1)
    );
  });

  // ─── Reconciliation-specific state ──────────────────────────────────────

  function toggleBalanceRow(participantId: number) {
    if (expandedBalanceRows.has(participantId)) {
      expandedBalanceRows.delete(participantId);
    } else {
      expandedBalanceRows.add(participantId);
    }
  }

  function togglePairwiseRow(participantId: number, otherId: number) {
    const key = `${participantId}-${otherId}`;
    if (expandedPairwise.has(key)) {
      expandedPairwise.delete(key);
    } else {
      expandedPairwise.add(key);
    }
  }

  function isPairwiseExpanded(participantId: number, otherId: number): boolean {
    return expandedPairwise.has(`${participantId}-${otherId}`);
  }

  function togglePairwiseYear(pairwiseKey: string, year: string) {
    if (!expandedPairwiseYears.has(pairwiseKey)) {
      expandedPairwiseYears.set(pairwiseKey, new SvelteSet());
    }
    const yearSet = expandedPairwiseYears.get(pairwiseKey)!;
    if (yearSet.has(year)) {
      yearSet.delete(year);
    } else {
      yearSet.add(year);
    }
  }

  function togglePairwiseMonth(pairwiseKey: string, year: string, monthKey: string) {
    if (!expandedPairwiseMonths.has(pairwiseKey)) {
      expandedPairwiseMonths.set(pairwiseKey, new SvelteMap());
    }
    const yearMap = expandedPairwiseMonths.get(pairwiseKey)!;
    if (!yearMap.has(year)) {
      yearMap.set(year, new SvelteSet());
    }
    const monthSet = yearMap.get(year)!;
    if (monthSet.has(monthKey)) {
      monthSet.delete(monthKey);
    } else {
      monthSet.add(monthKey);
    }
  }

  function getPairwiseForParticipant(participantId: number): PairwiseBalance[] {
    if (!projection.debts?.pairwise_balances) return [];
    return projection.debts.pairwise_balances.filter((p) => p.participant_id === participantId);
  }

  // ─── Derived balances/settlements ───────────────────────────────────────

  let filteredBalances = $derived.by(() => {
    if (!projection.debts) return [];
    if (projection.focusParticipantId === null) {
      return projection.nonPoolBalances;
    }
    return projection.nonPoolBalances.filter(
      (b) => b.participant_id === projection.focusParticipantId
    );
  });

  let activeSettlements = $derived.by(() => {
    if (!projection.debts) return [];
    return useDirectSettlements
      ? projection.debts.direct_settlements
      : projection.debts.settlements;
  });

  let startActiveSettlements = $derived.by(() => {
    if (!projection.startDebts) return [];
    return useDirectSettlements
      ? projection.startDebts.direct_settlements
      : projection.startDebts.settlements;
  });

  let filteredSettlements = $derived.by(() => {
    if (!projection.debts) return [];
    if (projection.focusParticipantId === null) return activeSettlements;
    return activeSettlements.filter((s) =>
      settlementInvolvesFocus(s, projection.focusParticipantId)
    );
  });

  let filteredStartSettlements = $derived.by(() => {
    if (!projection.startDebts) return [];
    if (projection.focusParticipantId === null) return startActiveSettlements;
    return startActiveSettlements.filter((s) =>
      settlementInvolvesFocus(s, projection.focusParticipantId)
    );
  });

  // ─── Balance time series ────────────────────────────────────────────────

  let balanceTimeSeries = $derived.by(() => {
    if (!projection.isRangeMode || !projection.debts || projection.rangeDates.length === 0)
      return new SvelteMap<string, Map<number, number>>();

    const result = new SvelteMap<string, Map<number, number>>();
    const runningBalance = new SvelteMap<number, number>();

    const sortedOccurrences = [...projection.debts.occurrences].sort((a, b) =>
      a.occurrence_date.localeCompare(b.occurrence_date)
    );

    for (const balance of projection.debts.balances) {
      if (!projection.poolIds.has(balance.participant_id)) {
        runningBalance.set(balance.participant_id, 0);
      }
    }

    let occIdx = 0;

    for (const date of projection.rangeDates) {
      while (
        occIdx < sortedOccurrences.length &&
        sortedOccurrences[occIdx].occurrence_date <= date
      ) {
        const occ = sortedOccurrences[occIdx];
        const payment = projection.paymentMap.get(occ.payment_id);

        if (payment) {
          const isPoolReceiver =
            occ.receiver_account_id !== null && projection.poolIds.has(occ.receiver_account_id);
          const isPoolPayer = occ.payer_id !== null && projection.poolIds.has(occ.payer_id);

          if (!isPoolReceiver && !isPoolPayer) {
            if (occ.receiver_account_id !== null) {
              if (occ.payer_id !== null) {
                const payerCurrent = runningBalance.get(occ.payer_id) ?? 0;
                runningBalance.set(occ.payer_id, payerCurrent + occ.amount);
                const receiverCurrent = runningBalance.get(occ.receiver_account_id) ?? 0;
                runningBalance.set(occ.receiver_account_id, receiverCurrent - occ.amount);
              } else {
                const receiverCurrent = runningBalance.get(occ.receiver_account_id) ?? 0;
                runningBalance.set(occ.receiver_account_id, receiverCurrent - occ.amount);
                if (payment.contributions) {
                  for (const contrib of payment.contributions) {
                    if (!projection.poolIds.has(contrib.participant_id)) {
                      const current = runningBalance.get(contrib.participant_id) ?? 0;
                      runningBalance.set(contrib.participant_id, current + contrib.amount);
                    }
                  }
                }
              }
            } else {
              if (occ.payer_id !== null && !projection.poolIds.has(occ.payer_id)) {
                const current = runningBalance.get(occ.payer_id) ?? 0;
                runningBalance.set(occ.payer_id, current + occ.amount);
              }
              if (payment.contributions) {
                for (const contrib of payment.contributions) {
                  if (!projection.poolIds.has(contrib.participant_id)) {
                    const current = runningBalance.get(contrib.participant_id) ?? 0;
                    runningBalance.set(contrib.participant_id, current - contrib.amount);
                  }
                }
              }
            }
          }
        }

        occIdx++;
      }

      result.set(date, new Map(runningBalance));
    }

    return result;
  });

  // Pairwise time series for focus mode
  let pairwiseTimeSeries = $derived.by(() => {
    if (
      !projection.isRangeMode ||
      !projection.debts ||
      !projection.focusParticipantId ||
      projection.rangeDates.length === 0
    ) {
      return new SvelteMap<string, Map<number, number>>();
    }

    const result = new SvelteMap<string, Map<number, number>>();
    const runningPairwise = new SvelteMap<number, number>();

    const sortedOccurrences = [...projection.debts.occurrences].sort((a, b) =>
      a.occurrence_date.localeCompare(b.occurrence_date)
    );

    for (const balance of projection.debts.balances) {
      if (
        balance.participant_id !== projection.focusParticipantId &&
        !projection.poolIds.has(balance.participant_id)
      ) {
        runningPairwise.set(balance.participant_id, 0);
      }
    }

    let occIdx = 0;

    for (const date of projection.rangeDates) {
      while (
        occIdx < sortedOccurrences.length &&
        sortedOccurrences[occIdx].occurrence_date <= date
      ) {
        const occ = sortedOccurrences[occIdx];
        const payment = projection.paymentMap.get(occ.payment_id);

        if (payment) {
          const isPoolReceiver =
            occ.receiver_account_id !== null && projection.poolIds.has(occ.receiver_account_id);
          const isPoolPayer = occ.payer_id !== null && projection.poolIds.has(occ.payer_id);

          if (!isPoolReceiver && !isPoolPayer) {
            if (occ.receiver_account_id !== null) {
              if (occ.payer_id !== null) {
                if (occ.payer_id === projection.focusParticipantId) {
                  const current = runningPairwise.get(occ.receiver_account_id) ?? 0;
                  runningPairwise.set(occ.receiver_account_id, current + occ.amount);
                } else if (occ.receiver_account_id === projection.focusParticipantId) {
                  const current = runningPairwise.get(occ.payer_id) ?? 0;
                  runningPairwise.set(occ.payer_id, current - occ.amount);
                }
              } else {
                if (
                  occ.receiver_account_id === projection.focusParticipantId &&
                  payment.contributions
                ) {
                  for (const contrib of payment.contributions) {
                    if (
                      contrib.participant_id !== projection.focusParticipantId &&
                      !projection.poolIds.has(contrib.participant_id)
                    ) {
                      const current = runningPairwise.get(contrib.participant_id) ?? 0;
                      runningPairwise.set(contrib.participant_id, current - contrib.amount);
                    }
                  }
                } else if (payment.contributions) {
                  const focusedContrib = payment.contributions.find(
                    (c) => c.participant_id === projection.focusParticipantId
                  );
                  if (focusedContrib && !projection.poolIds.has(occ.receiver_account_id)) {
                    const current = runningPairwise.get(occ.receiver_account_id) ?? 0;
                    runningPairwise.set(occ.receiver_account_id, current + focusedContrib.amount);
                  }
                }
              }
            } else {
              if (occ.payer_id === projection.focusParticipantId && payment.contributions) {
                for (const contrib of payment.contributions) {
                  if (
                    contrib.participant_id !== projection.focusParticipantId &&
                    !projection.poolIds.has(contrib.participant_id)
                  ) {
                    const current = runningPairwise.get(contrib.participant_id) ?? 0;
                    runningPairwise.set(contrib.participant_id, current + contrib.amount);
                  }
                }
              }

              if (
                occ.payer_id !== projection.focusParticipantId &&
                occ.payer_id !== null &&
                !projection.poolIds.has(occ.payer_id)
              ) {
                if (payment.contributions) {
                  const focusedContrib = payment.contributions.find(
                    (c) => c.participant_id === projection.focusParticipantId
                  );
                  if (focusedContrib) {
                    const current = runningPairwise.get(occ.payer_id) ?? 0;
                    runningPairwise.set(occ.payer_id, current - focusedContrib.amount);
                  }
                }
              }
            }
          }
        }

        occIdx++;
      }

      result.set(date, new Map(runningPairwise));
    }

    return result;
  });

  // ─── Balance chart ──────────────────────────────────────────────────────

  function getTodayAnnotation() {
    if (!annotationPlugin) return {};

    const isTodayInRange =
      projection.todayStr >= projection.targetDate &&
      (!projection.endDate || projection.todayStr <= projection.endDate);
    if (!isTodayInRange) return {};

    return {
      todayLine: {
        type: 'line' as const,
        xMin: projection.todayStr,
        xMax: projection.todayStr,
        borderColor: '#333',
        borderWidth: 2,
        borderDash: [5, 5],
        label: {
          display: true,
          content: $_('overview.today'),
          position: 'start' as const,
          backgroundColor: '#333',
          color: 'white',
          font: { size: 10 }
        }
      }
    };
  }

  function renderBalanceChart(canvas: HTMLCanvasElement) {
    if (!chartReady || !projection.isRangeMode || projection.rangeDates.length === 0) return;

    if (balanceChart) {
      balanceChart.destroy();
      balanceChart = null;
    }

    const datasets: {
      label: string;
      data: { x: string; y: number }[];
      borderColor: string;
      backgroundColor: string;
      stepped: 'before' | 'after' | 'middle' | boolean;
      fill: boolean;
      pointRadius: number;
      pointHoverRadius: number;
    }[] = [];

    const dateFormat = $preferences.date_format as string;

    if (projection.focusParticipantId !== null && pairwiseTimeSeries.size > 0) {
      const participantIds = [
        ...new Set([...pairwiseTimeSeries.values()].flatMap((m) => [...m.keys()]))
      ];

      participantIds.forEach((pid, idx) => {
        const otherBalance = projection.debts?.balances.find((b) => b.participant_id === pid);
        if (!otherBalance) return;

        datasets.push({
          label: `vs ${otherBalance.participant_name}`,
          data: projection.rangeDates.map((date) => ({
            x: date,
            y: pairwiseTimeSeries.get(date)?.get(pid) ?? 0
          })),
          borderColor: CHART_COLORS[idx % CHART_COLORS.length],
          backgroundColor: CHART_COLORS[idx % CHART_COLORS.length] + '20',
          stepped: 'before',
          fill: false,
          pointRadius: 3,
          pointHoverRadius: 5
        });
      });
    } else {
      const participantIds = [
        ...new Set([...balanceTimeSeries.values()].flatMap((m) => [...m.keys()]))
      ];

      participantIds.forEach((pid, idx) => {
        const balance = projection.debts?.balances.find((b) => b.participant_id === pid);
        if (!balance) return;

        datasets.push({
          label: balance.participant_name,
          data: projection.rangeDates.map((date) => ({
            x: date,
            y: balanceTimeSeries.get(date)?.get(pid) ?? 0
          })),
          borderColor: CHART_COLORS[idx % CHART_COLORS.length],
          backgroundColor: CHART_COLORS[idx % CHART_COLORS.length] + '20',
          stepped: 'before',
          fill: false,
          pointRadius: 3,
          pointHoverRadius: 5
        });
      });
    }

    balanceChart = new Chart(canvas, {
      type: 'line',
      data: { datasets },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: {
          mode: 'index',
          intersect: false
        },
        plugins: {
          legend: {
            position: 'bottom',
            labels: { usePointStyle: true }
          },
          annotation: {
            annotations: getTodayAnnotation()
          },
          tooltip: {
            callbacks: {
              title: (items) => {
                if (!items.length) return '';
                const dateStr = projection.rangeDates[items[0].dataIndex];
                return dateStr ? projection.formatDate(dateStr) : '';
              },
              label: (context) => {
                const value = context.parsed.y ?? 0;
                return `${context.dataset.label}: ${formatCurrency(value)}`;
              }
            }
          }
        },
        scales: {
          x: {
            type: 'time',
            time: {
              unit: 'day',
              displayFormats: { day: getChartDateFormat(dateFormat) }
            },
            adapters: {
              date: { locale: getDateFnsLocale() }
            },
            title: { display: false }
          },
          y: {
            title: { display: true, text: $_('overview.balance') },
            ticks: {
              callback: (value) => formatCurrency(value as number)
            }
          }
        }
      }
    });
  }

  // Effect to render balance chart
  $effect(() => {
    if (
      balanceChartCanvas &&
      chartReady &&
      projection.isRangeMode &&
      projection.rangeDates.length > 0
    ) {
      const _ts = projection.focusParticipantId !== null ? pairwiseTimeSeries : balanceTimeSeries;
      void _ts;
      renderBalanceChart(balanceChartCanvas);
    }
  });
</script>

<h2>{$_('reconciliation.title')}</h2>

<DateSelectorCard {projection} />
<FocusCard {projection} />

{#if projection.errorKey}
  <div class="error">{$_(projection.errorKey)}</div>
{/if}

{#if projection.loading}
  <p>{$_('common.loading')}</p>
{:else if projection.debts}
  <!-- Balances -->
  <section class="card">
    <h3>{$_('overview.balances')}</h3>
    {#if projection.isRangeMode && chartReady}
      <div class="chart-container">
        <canvas bind:this={balanceChartCanvas}></canvas>
      </div>
    {:else}
      <table class="balance-table">
        <thead>
          <tr>
            <th></th>
            <th>{$_('overview.participant')}</th>
            <th>{$_('overview.balance')}</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredBalances as b (b.participant_id)}
            {@const isExpanded = expandedBalanceRows.has(b.participant_id)}
            {@const pairwise = getPairwiseForParticipant(b.participant_id)}
            <tr
              class="balance-row"
              class:expanded={isExpanded}
              class:focused={projection.focusParticipantId === b.participant_id}
              onclick={() => toggleBalanceRow(b.participant_id)}
            >
              <td class="expand-cell">
                <span class="expand-icon">{isExpanded ? '▼' : '▶'}</span>
              </td>
              <td>{b.participant_name}</td>
              <td class:positive={b.net_balance > 0} class:negative={b.net_balance < 0}>
                {b.net_balance >= 0 ? '+' : ''}{formatCurrency(b.net_balance)}
              </td>
            </tr>
            {#if isExpanded}
              <tr class="pairwise-row">
                <td colspan="3">
                  <div class="pairwise-details">
                    {#if pairwise.length === 0}
                      <p class="no-relationships">{$_('overview.noRelationships')}</p>
                    {:else}
                      <div class="pairwise-list">
                        {#each pairwise as pw (pw.other_participant_id)}
                          {@const isExpanded = isPairwiseExpanded(
                            b.participant_id,
                            pw.other_participant_id
                          )}
                          <button
                            class="pairwise-item"
                            class:expanded={isExpanded}
                            onclick={() =>
                              togglePairwiseRow(b.participant_id, pw.other_participant_id)}
                          >
                            <span class="pairwise-toggle">{isExpanded ? '▼' : '▶'}</span>
                            <span class="pairwise-name">{pw.other_participant_name}</span>
                            <span
                              class="pairwise-net"
                              class:positive={pw.net > 0}
                              class:negative={pw.net < 0}
                            >
                              {pw.net >= 0 ? '+' : ''}{formatCurrency(pw.net)}
                            </span>
                          </button>
                          {#if isExpanded}
                            {@const pairwiseKey = `${b.participant_id}-${pw.other_participant_id}`}
                            <div class="pairwise-details-expanded">
                              {#if pw.amount_paid_for > 0.01}
                                <div class="detail-section">
                                  <div class="detail-header">
                                    {$_('overview.amountPaidFor', {
                                      values: {
                                        payer: b.participant_name,
                                        amount: formatCurrency(pw.amount_paid_for),
                                        paidFor: pw.other_participant_name
                                      }
                                    })}
                                  </div>
                                  {#if pw.paid_for_breakdown && pw.paid_for_breakdown.length > 0}
                                    {@const groupedYears = groupBreakdownByYearMonth(
                                      pw.paid_for_breakdown
                                    )}
                                    <div class="breakdown-hierarchy">
                                      {#each [...groupedYears.entries()] as [year, yearData] (year)}
                                        {@const yearExpanded =
                                          expandedPairwiseYears.get(pairwiseKey)?.has(year) ??
                                          false}
                                        <div class="breakdown-year">
                                          <button
                                            class="breakdown-year-btn"
                                            onclick={() => togglePairwiseYear(pairwiseKey, year)}
                                          >
                                            <span class="expand-icon"
                                              >{yearExpanded ? '▼' : '▶'}</span
                                            >
                                            <span class="year-text">{year}</span>
                                            <span class="year-total"
                                              >{formatCurrency(yearData.total)}</span
                                            >
                                          </button>
                                          {#if yearExpanded}
                                            {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                                              {@const monthExpanded =
                                                expandedPairwiseMonths
                                                  .get(pairwiseKey)
                                                  ?.get(year)
                                                  ?.has(monthKey) ?? false}
                                              <div class="breakdown-month">
                                                <button
                                                  class="breakdown-month-btn"
                                                  onclick={() =>
                                                    togglePairwiseMonth(
                                                      pairwiseKey,
                                                      year,
                                                      monthKey
                                                    )}
                                                >
                                                  <span class="expand-icon"
                                                    >{monthExpanded ? '▼' : '▶'}</span
                                                  >
                                                  <span class="month-text">{monthData.month}</span>
                                                  <span class="month-total"
                                                    >{formatCurrency(monthData.total)}</span
                                                  >
                                                </button>
                                                {#if monthExpanded}
                                                  <div class="breakdown-list">
                                                    {#each monthData.items as item (item.payment_id)}
                                                      {@const payment = projection.paymentMap.get(
                                                        item.payment_id
                                                      )}
                                                      {@const receiver =
                                                        payment?.receiver_account_id
                                                          ? $participants.find(
                                                              (pr) =>
                                                                pr.id ===
                                                                payment.receiver_account_id
                                                            )
                                                          : null}
                                                      <TransactionCard
                                                        description={item.description}
                                                        amount={item.amount}
                                                        date={item.occurrence_date}
                                                        originalDate={payment?.payment_date}
                                                        payerName={payment?.payer_name}
                                                        payerId={payment?.payer_id}
                                                        receiverName={receiver?.name}
                                                        receiverAccountId={payment?.receiver_account_id}
                                                        isFinal={payment?.is_final ?? true}
                                                        affectsBalance={payment?.affects_balance ??
                                                          true}
                                                        affectsPayerExpectation={payment?.affects_payer_expectation ??
                                                          false}
                                                        affectsReceiverExpectation={payment?.affects_receiver_expectation ??
                                                          false}
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
                                                {/if}
                                              </div>
                                            {/each}
                                          {/if}
                                        </div>
                                      {/each}
                                    </div>
                                  {/if}
                                </div>
                              {/if}
                              {#if pw.amount_owed_by > 0.01}
                                <div class="detail-section">
                                  <div class="detail-header">
                                    {$_('overview.amountPaidFor', {
                                      values: {
                                        payer: pw.other_participant_name,
                                        amount: formatCurrency(pw.amount_owed_by),
                                        paidFor: b.participant_name
                                      }
                                    })}
                                  </div>
                                  {#if pw.owed_by_breakdown && pw.owed_by_breakdown.length > 0}
                                    {@const groupedYears = groupBreakdownByYearMonth(
                                      pw.owed_by_breakdown
                                    )}
                                    <div class="breakdown-hierarchy">
                                      {#each [...groupedYears.entries()] as [year, yearData] (year)}
                                        {@const yearExpanded =
                                          expandedPairwiseYears
                                            .get(`${pairwiseKey}-owed`)
                                            ?.has(year) ?? false}
                                        <div class="breakdown-year">
                                          <button
                                            class="breakdown-year-btn"
                                            onclick={() =>
                                              togglePairwiseYear(`${pairwiseKey}-owed`, year)}
                                          >
                                            <span class="expand-icon"
                                              >{yearExpanded ? '▼' : '▶'}</span
                                            >
                                            <span class="year-text">{year}</span>
                                            <span class="year-total"
                                              >{formatCurrency(yearData.total)}</span
                                            >
                                          </button>
                                          {#if yearExpanded}
                                            {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                                              {@const monthExpanded =
                                                expandedPairwiseMonths
                                                  .get(`${pairwiseKey}-owed`)
                                                  ?.get(year)
                                                  ?.has(monthKey) ?? false}
                                              <div class="breakdown-month">
                                                <button
                                                  class="breakdown-month-btn"
                                                  onclick={() =>
                                                    togglePairwiseMonth(
                                                      `${pairwiseKey}-owed`,
                                                      year,
                                                      monthKey
                                                    )}
                                                >
                                                  <span class="expand-icon"
                                                    >{monthExpanded ? '▼' : '▶'}</span
                                                  >
                                                  <span class="month-text">{monthData.month}</span>
                                                  <span class="month-total"
                                                    >{formatCurrency(monthData.total)}</span
                                                  >
                                                </button>
                                                {#if monthExpanded}
                                                  <div class="breakdown-list">
                                                    {#each monthData.items as item (item.payment_id)}
                                                      {@const payment = projection.paymentMap.get(
                                                        item.payment_id
                                                      )}
                                                      {@const receiver =
                                                        payment?.receiver_account_id
                                                          ? $participants.find(
                                                              (pr) =>
                                                                pr.id ===
                                                                payment.receiver_account_id
                                                            )
                                                          : null}
                                                      <TransactionCard
                                                        description={item.description}
                                                        amount={item.amount}
                                                        date={item.occurrence_date}
                                                        originalDate={payment?.payment_date}
                                                        payerName={payment?.payer_name}
                                                        payerId={payment?.payer_id}
                                                        receiverName={receiver?.name}
                                                        receiverAccountId={payment?.receiver_account_id}
                                                        isFinal={payment?.is_final ?? true}
                                                        affectsBalance={payment?.affects_balance ??
                                                          true}
                                                        affectsPayerExpectation={payment?.affects_payer_expectation ??
                                                          false}
                                                        affectsReceiverExpectation={payment?.affects_receiver_expectation ??
                                                          false}
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
                                                {/if}
                                              </div>
                                            {/each}
                                          {/if}
                                        </div>
                                      {/each}
                                    </div>
                                  {/if}
                                </div>
                              {/if}
                            </div>
                          {/if}
                        {/each}
                      </div>
                    {/if}
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <!-- Settlements -->
  <section class="card">
    <div class="settlements-header">
      <h3>{$_('overview.settlements')}</h3>
      <label class="settlement-mode-toggle">
        <input type="checkbox" bind:checked={useDirectSettlements} />
        {$_('overview.directOnlyMode')}
      </label>
    </div>

    {#if projection.isRangeMode}
      <div class="settlements-range">
        <div class="settlement-column">
          <h4 class="settlement-date-label">
            {$_('overview.atStart')} ({projection.formatDate(projection.targetDate)})
          </h4>
          {#if filteredStartSettlements.length === 0}
            <p class="all-settled">
              {projection.focusParticipantId !== null
                ? $_('overview.noSettlementsForParticipant')
                : $_('overview.allSettled')}
            </p>
          {:else}
            <ul class="settlements-list">
              {#each filteredStartSettlements as s (`start-${s.from_participant_id}-${s.to_participant_id}`)}
                <li class:focused={projection.focusParticipantId !== null}>
                  <span
                    class="from"
                    class:highlight={s.from_participant_id === projection.focusParticipantId}
                    >{s.from_participant_name}</span
                  >
                  <span class="arrow">&rarr;</span>
                  <span
                    class="to"
                    class:highlight={s.to_participant_id === projection.focusParticipantId}
                    >{s.to_participant_name}</span
                  >
                  <span class="amount">{formatCurrency(s.amount)}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        <div class="settlement-column">
          <h4 class="settlement-date-label">
            {$_('overview.atEnd')} ({projection.formatDate(
              projection.endDate ?? projection.targetDate
            )})
          </h4>
          {#if filteredSettlements.length === 0}
            <p class="all-settled">
              {projection.focusParticipantId !== null
                ? $_('overview.noSettlementsForParticipant')
                : $_('overview.allSettled')}
            </p>
          {:else}
            <ul class="settlements-list">
              {#each filteredSettlements as s (`end-${s.from_participant_id}-${s.to_participant_id}`)}
                <li class:focused={projection.focusParticipantId !== null}>
                  <span
                    class="from"
                    class:highlight={s.from_participant_id === projection.focusParticipantId}
                    >{s.from_participant_name}</span
                  >
                  <span class="arrow">&rarr;</span>
                  <span
                    class="to"
                    class:highlight={s.to_participant_id === projection.focusParticipantId}
                    >{s.to_participant_name}</span
                  >
                  <span class="amount">{formatCurrency(s.amount)}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    {:else if filteredSettlements.length === 0}
      <p class="all-settled">
        {projection.focusParticipantId !== null
          ? $_('overview.noSettlementsForParticipant')
          : $_('overview.allSettled')}
      </p>
    {:else}
      <ul class="settlements-list">
        {#each filteredSettlements as s (`${s.from_participant_id}-${s.to_participant_id}`)}
          <li class:focused={projection.focusParticipantId !== null}>
            <span
              class="from"
              class:highlight={s.from_participant_id === projection.focusParticipantId}
              >{s.from_participant_name}</span
            >
            <span class="arrow">&rarr;</span>
            <span class="to" class:highlight={s.to_participant_id === projection.focusParticipantId}
              >{s.to_participant_name}</span
            >
            <span class="amount">{formatCurrency(s.amount)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <OccurrencesList
    occurrences={settlementOccurrences}
    paymentMap={projection.paymentMap}
    focusParticipantId={projection.focusParticipantId}
    formatDate={projection.formatDate}
  />
{/if}

<style>
  h2 {
    margin-bottom: 1.5rem;
  }

  .card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .chart-container {
    position: relative;
    height: 350px;
    width: 100%;
    margin: 1rem 0;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
  }

  h3 {
    margin-top: 0;
    margin-bottom: 1rem;
    color: var(--accent, #7b61ff);
  }

  .error {
    background: #fee;
    color: #c00;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .balance-table {
    width: 100%;
    border-collapse: collapse;
  }

  .balance-table th,
  .balance-table td {
    padding: 0.75rem;
    text-align: left;
    border-bottom: 1px solid #eee;
  }

  .balance-table th {
    font-weight: 600;
    color: #666;
  }

  .positive {
    color: #2a9d8f;
    font-weight: 600;
  }

  .negative {
    color: #e76f51;
    font-weight: 600;
  }

  .balance-row {
    cursor: pointer;
    transition: background 0.15s;
  }

  .balance-row:hover {
    background: rgba(123, 97, 255, 0.05);
  }

  .balance-row.expanded {
    background: rgba(123, 97, 255, 0.08);
  }

  .expand-cell {
    width: 30px;
    padding: 0.75rem 0.5rem !important;
    text-align: center;
  }

  .balance-table td:last-child {
    text-align: right;
  }

  .balance-table th:last-child {
    text-align: right;
  }

  .pairwise-row {
    background: #fafafa;
  }

  .pairwise-row td {
    padding: 0 !important;
  }

  .pairwise-details {
    padding: 1rem 1.5rem;
    border-left: 3px solid var(--accent, #7b61ff);
    margin-left: 0.5rem;
  }

  .no-relationships {
    color: #888;
    font-style: italic;
    margin: 0;
  }

  .pairwise-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .pairwise-item {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem;
    background: white;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    transition: background 0.15s;
  }

  .pairwise-item:hover {
    background: rgba(123, 97, 255, 0.05);
  }

  .pairwise-item.expanded {
    background: rgba(123, 97, 255, 0.08);
  }

  .pairwise-toggle {
    color: var(--accent, #7b61ff);
    font-size: 0.8rem;
    flex: 0 0 auto;
  }

  .pairwise-name {
    font-weight: 600;
    min-width: 100px;
    flex: 1;
  }

  .pairwise-net {
    font-weight: 600;
    text-align: right;
    min-width: 100px;
  }

  .pairwise-details-expanded {
    padding: 0.75rem 1.5rem;
    background: #fafafa;
    border-radius: 8px;
    margin-top: 0.5rem;
    border-left: 3px solid var(--accent, #7b61ff);
  }

  .detail-section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-bottom: 1rem;
    align-items: stretch;
  }

  .detail-section:last-child {
    margin-bottom: 0;
  }

  .detail-header {
    color: #222;
    font-weight: 500;
    padding: 0.75rem 0;
    margin-bottom: 0.5rem;
    border-bottom: 2px solid #d0d0d0;
    text-align: left;
  }

  .breakdown-hierarchy {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .breakdown-year {
    display: flex;
    flex-direction: column;
  }

  .breakdown-year-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    background: white;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
  }

  .breakdown-year-btn:hover {
    background: rgba(123, 97, 255, 0.05);
  }

  .breakdown-month {
    display: flex;
    flex-direction: column;
    margin-left: 1rem;
    margin-top: 0.5rem;
  }

  .breakdown-month-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem;
    background: #fafafa;
    border: 1px solid #e8e8e8;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
  }

  .breakdown-month-btn:hover {
    background: rgba(123, 97, 255, 0.05);
  }

  .year-text,
  .month-text {
    flex: 1;
    font-weight: 500;
    font-size: 115%;
    color: #333;
  }

  .year-total,
  .month-total {
    font-weight: 600;
    color: var(--accent, #7b61ff);
  }

  .breakdown-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    list-style: none;
    padding: 0.5rem 0 0 1rem;
    margin: 0.5rem 0 0 1rem;
    font-size: 0.8rem;
    border-left: 2px solid #ddd;
  }

  .expand-icon {
    color: var(--accent, #7b61ff);
    font-size: 0.8rem;
    flex: 0 0 auto;
  }

  .focused {
    background: rgba(123, 97, 255, 0.1) !important;
  }

  .balance-row.focused {
    border-left: 3px solid var(--accent, #7b61ff);
  }

  .highlight {
    font-weight: 700;
    color: var(--accent, #7b61ff);
  }

  .settlements-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .settlements-header h3 {
    margin: 0;
  }

  .settlement-mode-toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    color: #666;
    cursor: pointer;
    user-select: none;
  }

  .settlement-mode-toggle input[type='checkbox'] {
    cursor: pointer;
  }

  .all-settled {
    text-align: center;
    padding: 2rem;
    color: #2a9d8f;
    font-size: 1.2rem;
    font-weight: 600;
  }

  .settlements-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .settlements-list li {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem;
    border-bottom: 1px solid #eee;
  }

  .settlements-list li:last-child {
    border-bottom: none;
  }

  .from {
    font-weight: 600;
    color: #e76f51;
  }

  .arrow {
    color: #999;
  }

  .to {
    font-weight: 600;
    color: #2a9d8f;
  }

  .amount {
    margin-left: auto;
    font-weight: 700;
    color: var(--accent, #7b61ff);
    font-size: 1.1rem;
  }

  .settlements-range {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
  }

  .settlement-column {
    min-width: 0;
  }

  .settlement-date-label {
    font-size: 0.9rem;
    font-weight: 600;
    color: #666;
    margin-bottom: 0.75rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid #eee;
  }

  @media (max-width: 600px) {
    .settlements-range {
      grid-template-columns: 1fr;
      gap: 1rem;
    }
  }
</style>
