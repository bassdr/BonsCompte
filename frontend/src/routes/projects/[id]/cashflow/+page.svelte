<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import { _ } from '$lib/i18n';
  import { participants } from '$lib/stores/project';
  import { preferences } from '$lib/stores/preferences';
  import { formatCurrency } from '$lib/format/currency';
  import type { PaymentOccurrence } from '$lib/api';
  import TransactionCard from '$lib/components/TransactionCard.svelte';
  import DateSelectorCard from '$lib/components/DateSelectorCard.svelte';
  import OccurrencesList from '$lib/components/OccurrencesList.svelte';
  import { SvelteSet, SvelteMap } from 'svelte/reactivity';
  import { Chart, registerables } from 'chart.js';
  import 'chartjs-adapter-date-fns';
  import {
    CHART_COLORS,
    getChartDateFormat,
    getDateFnsLocale,
    groupBreakdownByYearMonth,
    type PoolTotalStats
  } from '$lib/date-projection';
  import { createOverviewState } from '$lib/date-projection.svelte';

  // Chart.js annotation plugin - loaded dynamically for SSR safety
  let annotationPlugin: typeof import('chartjs-plugin-annotation').default | null = null;
  let chartReady = $state(false);

  type XYPoint = { x: string; y: number };
  type LineChart = Chart<'line', XYPoint[], unknown>;

  // This Map is written by the chart, it creates an endless loop if we change it to a SvelteMap
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  let poolCharts = new Map<number, LineChart>();

  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  let poolChartCanvases = new Map<number, HTMLCanvasElement>();

  let projectId = $derived(parseInt(page.params.id ?? ''));

  // Redirect to reconciliation if no pool accounts exist
  const hasPools = $derived($participants.some((p) => p.account_type === 'pool'));
  $effect(() => {
    if ($participants.length > 0 && !hasPools) {
      goto(`/projects/${page.params.id}/reconciliation`, { replaceState: true });
    }
  });

  // Create shared projection state
  const projection = createOverviewState();

  // Initialize Chart.js (SSR-safe)
  onMount(() => {
    Chart.register(...registerables);

    import('chartjs-plugin-annotation').then((mod) => {
      annotationPlugin = mod.default;
      Chart.register(annotationPlugin);
      chartReady = true;
    });

    return () => {
      poolCharts.forEach((chart) => chart.destroy());
      poolCharts.clear();
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

  // ─── Pool-specific local state ──────────────────────────────────────────

  // Track which pool ownership entries are expanded
  let expandedPoolEntries = new SvelteSet<number>();
  let expandedPoolYears = new SvelteMap<number, SvelteSet<string>>();
  let expandedPoolMonths = new SvelteMap<number, SvelteMap<string, SvelteSet<string>>>();

  function togglePoolEntry(participantId: number) {
    if (expandedPoolEntries.has(participantId)) {
      expandedPoolEntries.delete(participantId);
    } else {
      expandedPoolEntries.add(participantId);
    }
  }

  function isPoolEntryExpanded(participantId: number): boolean {
    return expandedPoolEntries.has(participantId);
  }

  function togglePoolYear(participantId: number, year: string) {
    if (!expandedPoolYears.has(participantId)) {
      expandedPoolYears.set(participantId, new SvelteSet());
    }
    const yearSet = expandedPoolYears.get(participantId)!;
    if (yearSet.has(year)) {
      yearSet.delete(year);
    } else {
      yearSet.add(year);
    }
  }

  function togglePoolMonth(participantId: number, year: string, monthKey: string) {
    if (!expandedPoolMonths.has(participantId)) {
      expandedPoolMonths.set(participantId, new SvelteMap());
    }
    const yearMap = expandedPoolMonths.get(participantId)!;
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

  // ─── Pool-specific derived state ────────────────────────────────────────

  let filteredPoolOwnerships = $derived.by(() => {
    if (!projection.debts?.pool_ownerships || projection.debts.pool_ownerships.length === 0)
      return [];
    if (projection.focusParticipantId === null) return projection.debts.pool_ownerships;

    return projection.debts.pool_ownerships
      .map((pool) => {
        const filteredEntries = pool.entries.filter(
          (e) => e.participant_id === projection.focusParticipantId
        );
        return {
          ...pool,
          entries: filteredEntries,
          total_balance: filteredEntries.reduce((sum, e) => sum + e.ownership, 0)
        };
      })
      .filter((pool) => pool.entries.length > 0);
  });

  // ─── Per-pool occurrences (only transactions involving each pool) ──────

  const poolOccurrences = $derived.by(() => {
    const result = new SvelteMap<number, PaymentOccurrence[]>();
    if (!projection.debts?.occurrences || !projection.debts?.pool_ownerships) return result;

    for (const pool of projection.debts.pool_ownerships) {
      const poolId = pool.pool_id;
      const filtered = projection.debts.occurrences.filter(
        (occ) => occ.payer_id === poolId || occ.receiver_account_id === poolId
      );
      if (filtered.length > 0) {
        result.set(poolId, filtered);
      }
    }
    return result;
  });

  // ─── Pool ownership time series ─────────────────────────────────────────

  let poolOwnershipTimeSeries = $derived.by(() => {
    if (
      !projection.isRangeMode ||
      !projection.debts ||
      projection.rangeDates.length === 0 ||
      !projection.debts.pool_ownerships
    ) {
      return new SvelteMap<number, Map<string, Map<number, number>>>();
    }

    const result = new SvelteMap<number, Map<string, Map<number, number>>>();

    for (const pool of projection.debts.pool_ownerships) {
      const poolTimeSeries = new SvelteMap<string, Map<number, number>>();
      const runningOwnership = new SvelteMap<number, number>();

      for (const entry of pool.entries) {
        runningOwnership.set(entry.participant_id, 0);
      }

      const sortedOccurrences = [...projection.debts.occurrences].sort((a, b) =>
        a.occurrence_date.localeCompare(b.occurrence_date)
      );

      let occIdx = 0;

      for (const date of projection.rangeDates) {
        while (
          occIdx < sortedOccurrences.length &&
          sortedOccurrences[occIdx].occurrence_date <= date
        ) {
          const occ = sortedOccurrences[occIdx];
          const payment = projection.paymentMap.get(occ.payment_id);

          if (payment && occ.affects_balance) {
            if (occ.receiver_account_id === pool.pool_id && occ.payer_id !== null) {
              const current = runningOwnership.get(occ.payer_id) ?? 0;
              runningOwnership.set(occ.payer_id, current + occ.amount);
            }

            if (occ.payer_id === pool.pool_id && occ.receiver_account_id !== null) {
              const current = runningOwnership.get(occ.receiver_account_id) ?? 0;
              runningOwnership.set(occ.receiver_account_id, current - occ.amount);
            }

            if (
              occ.payer_id === pool.pool_id &&
              occ.receiver_account_id === null &&
              payment.contributions
            ) {
              for (const contrib of payment.contributions) {
                if (!projection.poolIds.has(contrib.participant_id)) {
                  const current = runningOwnership.get(contrib.participant_id) ?? 0;
                  runningOwnership.set(contrib.participant_id, current - contrib.amount);
                }
              }
            }

            if (
              occ.receiver_account_id === pool.pool_id &&
              occ.payer_id === null &&
              payment.contributions
            ) {
              for (const contrib of payment.contributions) {
                if (!projection.poolIds.has(contrib.participant_id)) {
                  const current = runningOwnership.get(contrib.participant_id) ?? 0;
                  runningOwnership.set(contrib.participant_id, current + contrib.amount);
                }
              }
            }
          }

          occIdx++;
        }

        poolTimeSeries.set(date, new Map(runningOwnership));
      }

      result.set(pool.pool_id, poolTimeSeries);
    }

    return result;
  });

  // Pool total time series
  let poolTotalTimeSeries = $derived.by(() => {
    if (
      !projection.isRangeMode ||
      poolOwnershipTimeSeries.size === 0 ||
      projection.rangeDates.length === 0
    ) {
      return new SvelteMap<number, Map<string, number>>();
    }

    const result = new SvelteMap<number, Map<string, number>>();

    for (const [poolId, poolData] of poolOwnershipTimeSeries) {
      const totals = new SvelteMap<string, number>();

      for (const date of projection.rangeDates) {
        const participantOwnerships = poolData.get(date);
        if (participantOwnerships) {
          let total = 0;
          for (const ownership of participantOwnerships.values()) {
            total += ownership;
          }
          totals.set(date, total);
        } else {
          totals.set(date, 0);
        }
      }

      result.set(poolId, totals);
    }

    return result;
  });

  // Pool expected minimum time series
  let poolExpectedMinimumTimeSeries = $derived.by(() => {
    if (
      !projection.isRangeMode ||
      !projection.debts ||
      projection.rangeDates.length === 0 ||
      !projection.debts.pool_ownerships
    ) {
      return new SvelteMap<
        number,
        {
          total: Map<string, number>;
          perParticipant: Map<string, Map<number, number>>;
        }
      >();
    }

    const result = new SvelteMap<
      number,
      {
        total: Map<string, number>;
        perParticipant: Map<string, Map<number, number>>;
      }
    >();

    for (const pool of projection.debts.pool_ownerships) {
      const totalTimeSeries = new SvelteMap<string, number>();
      const perParticipantTimeSeries = new SvelteMap<string, Map<number, number>>();

      let runningExpectedMin = 0;
      const participantExpectedMin = new SvelteMap<number, number>();
      for (const entry of pool.entries) {
        participantExpectedMin.set(entry.participant_id, 0);
      }

      const sortedOccurrences = [...projection.debts.occurrences].sort((a, b) =>
        a.occurrence_date.localeCompare(b.occurrence_date)
      );

      let occIdx = 0;

      for (const date of projection.rangeDates) {
        while (
          occIdx < sortedOccurrences.length &&
          sortedOccurrences[occIdx].occurrence_date <= date
        ) {
          const occ = sortedOccurrences[occIdx];

          if (
            occ.receiver_account_id === pool.pool_id &&
            occ.payer_id !== null &&
            occ.affects_receiver_expectation
          ) {
            runningExpectedMin += occ.amount;
            const current = participantExpectedMin.get(occ.payer_id) ?? 0;
            participantExpectedMin.set(occ.payer_id, current + occ.amount);
          }

          if (
            occ.payer_id === pool.pool_id &&
            occ.receiver_account_id !== null &&
            occ.affects_payer_expectation
          ) {
            runningExpectedMin -= occ.amount;
            const current = participantExpectedMin.get(occ.receiver_account_id) ?? 0;
            participantExpectedMin.set(occ.receiver_account_id, current - occ.amount);
          }

          if (
            occ.receiver_account_id === pool.pool_id &&
            occ.payer_id === null &&
            occ.affects_receiver_expectation
          ) {
            runningExpectedMin += occ.amount;
            const payment = projection.paymentMap.get(occ.payment_id);
            if (payment) {
              for (const contrib of payment.contributions) {
                const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
                participantExpectedMin.set(contrib.participant_id, current + contrib.amount);
              }
            }
          }

          if (
            occ.payer_id === pool.pool_id &&
            occ.receiver_account_id === null &&
            occ.affects_payer_expectation
          ) {
            runningExpectedMin -= occ.amount;
            const payment = projection.paymentMap.get(occ.payment_id);
            if (payment) {
              for (const contrib of payment.contributions) {
                const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
                participantExpectedMin.set(contrib.participant_id, current - contrib.amount);
              }
            }
          }

          occIdx++;
        }

        totalTimeSeries.set(date, runningExpectedMin);
        const participantSnapshot = new SvelteMap<number, number>();
        for (const [pid, value] of participantExpectedMin) {
          participantSnapshot.set(pid, value);
        }
        perParticipantTimeSeries.set(date, participantSnapshot);
      }

      result.set(pool.pool_id, {
        total: totalTimeSeries,
        perParticipant: perParticipantTimeSeries
      });
    }

    return result;
  });

  // Per-participant expected minimum for non-range mode
  let poolParticipantExpectedMinimum = $derived.by(() => {
    const result = new SvelteMap<number, Map<number, number>>();

    if (!projection.debts?.pool_ownerships || !projection.debts.occurrences) return result;

    for (const pool of projection.debts.pool_ownerships) {
      const participantExpectedMin = new SvelteMap<number, number>();
      for (const entry of pool.entries) {
        participantExpectedMin.set(entry.participant_id, 0);
      }

      for (const occ of projection.debts.occurrences) {
        if (
          occ.receiver_account_id === pool.pool_id &&
          occ.payer_id !== null &&
          occ.affects_receiver_expectation
        ) {
          const current = participantExpectedMin.get(occ.payer_id) ?? 0;
          participantExpectedMin.set(occ.payer_id, current + occ.amount);
        }

        if (
          occ.payer_id === pool.pool_id &&
          occ.receiver_account_id !== null &&
          occ.affects_payer_expectation
        ) {
          const current = participantExpectedMin.get(occ.receiver_account_id) ?? 0;
          participantExpectedMin.set(occ.receiver_account_id, current - occ.amount);
        }

        if (
          occ.receiver_account_id === pool.pool_id &&
          occ.payer_id === null &&
          occ.affects_receiver_expectation
        ) {
          const payment = projection.paymentMap.get(occ.payment_id);
          if (payment) {
            for (const contrib of payment.contributions) {
              const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
              participantExpectedMin.set(contrib.participant_id, current + contrib.amount);
            }
          }
        }

        if (
          occ.payer_id === pool.pool_id &&
          occ.receiver_account_id === null &&
          occ.affects_payer_expectation
        ) {
          const payment = projection.paymentMap.get(occ.payment_id);
          if (payment) {
            for (const contrib of payment.contributions) {
              const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
              participantExpectedMin.set(contrib.participant_id, current - contrib.amount);
            }
          }
        }
      }

      result.set(pool.pool_id, participantExpectedMin);
    }

    return result;
  });

  // Pool total stats
  let poolTotalStats = $derived.by(() => {
    const result = new SvelteMap<number, PoolTotalStats>();

    if (!projection.debts?.pool_ownerships) return result;

    for (const pool of projection.debts.pool_ownerships) {
      if (!projection.isRangeMode) {
        let currentValue: number;
        let currentExpectedMin: number;
        let isBelowExpected: boolean;

        if (projection.focusParticipantId !== null) {
          const entry = pool.entries.find(
            (e) => e.participant_id === projection.focusParticipantId
          );
          currentValue = entry?.ownership ?? 0;
          const participantExpMin = poolParticipantExpectedMinimum.get(pool.pool_id);
          currentExpectedMin = participantExpMin?.get(projection.focusParticipantId) ?? 0;
          isBelowExpected = currentValue < currentExpectedMin;
        } else {
          currentValue = pool.total_balance;
          currentExpectedMin = pool.expected_minimum;
          isBelowExpected = pool.is_below_expected;
        }

        result.set(pool.pool_id, {
          currentTotal: currentValue,
          projectedTotal: currentValue,
          minTotal: currentValue,
          hasNegative: currentValue < 0,
          currentExpectedMinimum: currentExpectedMin,
          projectedExpectedMinimum: currentExpectedMin,
          maxExpectedMinimum: currentExpectedMin,
          isBelowExpected
        });
      } else {
        const poolData = poolOwnershipTimeSeries.get(pool.pool_id);
        const expectedMinData = poolExpectedMinimumTimeSeries.get(pool.pool_id);

        if (!poolData || poolData.size === 0) {
          result.set(pool.pool_id, {
            currentTotal: 0,
            projectedTotal: 0,
            minTotal: 0,
            hasNegative: false,
            currentExpectedMinimum: 0,
            projectedExpectedMinimum: 0,
            maxExpectedMinimum: 0,
            isBelowExpected: false
          });
          continue;
        }

        let currentTotal = 0;
        let projectedTotal = 0;
        let minTotal = Infinity;
        let hasNegative = false;
        let isBelowExpected = false;
        let maxExpectedMin = -Infinity;

        const dates = [...poolData.keys()];

        const getValueForDate = (date: string): number => {
          const participantOwnerships = poolData.get(date);
          if (!participantOwnerships) return 0;

          if (projection.focusParticipantId !== null) {
            return participantOwnerships.get(projection.focusParticipantId) ?? 0;
          } else {
            let sum = 0;
            for (const ownership of participantOwnerships.values()) {
              sum += ownership;
            }
            return sum;
          }
        };

        const getExpectedMinForDate = (date: string): number => {
          if (!expectedMinData) return 0;
          if (projection.focusParticipantId !== null) {
            return (
              expectedMinData.perParticipant.get(date)?.get(projection.focusParticipantId) ?? 0
            );
          } else {
            return expectedMinData.total.get(date) ?? 0;
          }
        };

        if (dates.length > 0) {
          currentTotal = getValueForDate(dates[0]);
          projectedTotal = getValueForDate(dates[dates.length - 1]);

          for (const date of dates) {
            const value = getValueForDate(date);
            const expectedMin = getExpectedMinForDate(date);

            if (value < minTotal) {
              minTotal = value;
            }
            if (value < 0) {
              hasNegative = true;
            }
            if (value < expectedMin) {
              isBelowExpected = true;
            }
            if (expectedMin > maxExpectedMin) {
              maxExpectedMin = expectedMin;
            }
          }
        }

        if (minTotal === Infinity) minTotal = 0;
        if (maxExpectedMin === -Infinity) maxExpectedMin = 0;

        const currentExpectedMin = getExpectedMinForDate(dates[0]);
        const projectedExpectedMin = getExpectedMinForDate(dates[dates.length - 1]);

        result.set(pool.pool_id, {
          currentTotal,
          projectedTotal,
          minTotal,
          hasNegative,
          currentExpectedMinimum: currentExpectedMin,
          projectedExpectedMinimum: projectedExpectedMin,
          maxExpectedMinimum: maxExpectedMin,
          isBelowExpected
        });
      }
    }

    return result;
  });

  // ─── Chart rendering ────────────────────────────────────────────────────

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

  function renderPoolChart(canvas: HTMLCanvasElement, poolId: number) {
    if (!chartReady || !projection.isRangeMode || projection.rangeDates.length === 0) return;

    const existingChart = poolCharts.get(poolId);
    if (existingChart) {
      existingChart.destroy();
      poolCharts.delete(poolId);
    }

    const poolData = poolOwnershipTimeSeries.get(poolId);
    if (!poolData) return;

    const pool = projection.debts?.pool_ownerships?.find((p) => p.pool_id === poolId);
    if (!pool) return;

    const totalData = poolTotalTimeSeries.get(poolId);

    interface DatasetConfig {
      label: string;
      data: { x: string; y: number }[];
      borderColor: string;
      backgroundColor: string;
      stepped: 'before' | 'after' | 'middle' | boolean;
      fill: boolean | { target: number; above: string; below: string };
      pointRadius: number;
      pointHoverRadius: number;
      borderWidth?: number;
      borderDash?: number[];
      order?: number;
    }

    const datasets: DatasetConfig[] = [];

    const entriesToShow =
      projection.focusParticipantId !== null
        ? pool.entries.filter((e) => e.participant_id === projection.focusParticipantId)
        : pool.entries;

    let balanceDatasetIndex = 0;

    entriesToShow.forEach((entry, idx) => {
      datasets.push({
        label: entry.participant_name,
        data: projection.rangeDates.map((date) => ({
          x: date,
          y: poolData.get(date)?.get(entry.participant_id) ?? 0
        })),
        borderColor: CHART_COLORS[idx % CHART_COLORS.length],
        backgroundColor: CHART_COLORS[idx % CHART_COLORS.length] + '20',
        stepped: 'before',
        fill: false,
        pointRadius: 3,
        pointHoverRadius: 5,
        order: 1
      });
    });

    if (totalData && projection.focusParticipantId === null) {
      balanceDatasetIndex = datasets.length;
      datasets.push({
        label: $_('common.total'),
        data: projection.rangeDates.map((date) => ({
          x: date,
          y: totalData.get(date) ?? 0
        })),
        borderColor: '#1a1a1a',
        backgroundColor: '#1a1a1a20',
        stepped: 'before',
        fill: false,
        pointRadius: 4,
        pointHoverRadius: 6,
        borderWidth: 3,
        borderDash: [6, 4],
        order: 0
      });
    }

    const expectedMinData = poolExpectedMinimumTimeSeries.get(poolId);

    const getExpectedMinForDate = (date: string): number => {
      if (!expectedMinData) return 0;
      if (projection.focusParticipantId !== null) {
        return expectedMinData.perParticipant.get(date)?.get(projection.focusParticipantId) ?? 0;
      } else {
        return expectedMinData.total.get(date) ?? 0;
      }
    };

    const hasExpectedMinData = expectedMinData !== undefined;

    if (hasExpectedMinData) {
      datasets.push({
        label: $_('overview.expectedMinimum'),
        data: projection.rangeDates.map((date) => ({
          x: date,
          y: getExpectedMinForDate(date)
        })),
        borderColor: '#dc3545',
        backgroundColor: 'rgba(220, 53, 69, 0.15)',
        stepped: 'before',
        fill: {
          target: balanceDatasetIndex,
          above: 'rgba(220, 53, 69, 0.15)',
          below: 'transparent'
        },
        pointRadius: 2,
        pointHoverRadius: 4,
        borderWidth: 2,
        borderDash: [4, 4],
        order: 0
      });
    }

    const annotations = getTodayAnnotation();
    const dateFormat = $preferences.date_format as string;

    const chart = new Chart(canvas, {
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
            annotations
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
                const base = `${context.dataset.label}: ${formatCurrency(value)}`;
                if (!expectedMinData) return base;
                const dateStr = projection.rangeDates[context.dataIndex];
                if (!dateStr) return base;
                const idx = context.datasetIndex;
                if (idx < entriesToShow.length) {
                  const entry = entriesToShow[idx];
                  const expMin =
                    expectedMinData.perParticipant.get(dateStr)?.get(entry.participant_id) ?? 0;
                  if (expMin === 0 && value === 0) return base;
                  const diff = value - expMin;
                  if (diff >= 0) {
                    return `${base} (${$_('overview.aboveExpected', { values: { amount: formatCurrency(diff) } })})`;
                  } else {
                    return `${base} (${$_('overview.belowExpectedBy', { values: { amount: formatCurrency(Math.abs(diff)) } })})`;
                  }
                }
                if (
                  totalData &&
                  projection.focusParticipantId === null &&
                  idx === entriesToShow.length
                ) {
                  const totalExpMin = expectedMinData.total.get(dateStr) ?? 0;
                  if (totalExpMin === 0 && value === 0) return base;
                  const diff = value - totalExpMin;
                  if (diff >= 0) {
                    return `${base} (${$_('overview.aboveExpected', { values: { amount: formatCurrency(diff) } })})`;
                  } else {
                    return `${base} (${$_('overview.belowExpectedBy', { values: { amount: formatCurrency(Math.abs(diff)) } })})`;
                  }
                }
                return base;
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
            title: { display: true, text: $_('overview.ownership') },
            ticks: {
              callback: (value) => formatCurrency(value as number)
            }
          }
        }
      }
    });

    poolCharts.set(poolId, chart);
  }

  // Effect to render pool charts when data changes
  $effect(() => {
    if (
      chartReady &&
      projection.isRangeMode &&
      projection.rangeDates.length > 0 &&
      projection.debts?.pool_ownerships
    ) {
      const _pts = poolOwnershipTimeSeries;
      void _pts;

      for (const pool of projection.debts.pool_ownerships) {
        const canvas = poolChartCanvases.get(pool.pool_id);
        if (canvas) {
          renderPoolChart(canvas, pool.pool_id);
        }
      }
    }
  });

  function bindPoolCanvas(node: HTMLCanvasElement, params: { poolId: number }) {
    poolChartCanvases.set(params.poolId, node);
    if (chartReady && projection.isRangeMode && projection.rangeDates.length > 0) {
      renderPoolChart(node, params.poolId);
    }
    return {
      destroy() {
        const chart = poolCharts.get(params.poolId);
        if (chart) {
          chart.destroy();
          poolCharts.delete(params.poolId);
        }
        poolChartCanvases.delete(params.poolId);
      }
    };
  }
</script>

<h2>{$_('cashflow.title')}</h2>

<DateSelectorCard {projection} />

{#if projection.errorKey}
  <div class="error">{$_(projection.errorKey)}</div>
{/if}

{#if projection.loading}
  <p>{$_('common.loading')}</p>
{:else if projection.debts}
  <!-- Pool Ownerships -->
  {#each filteredPoolOwnerships as poolOwnership (poolOwnership.pool_id)}
    {@const stats = poolTotalStats.get(poolOwnership.pool_id)}
    <section class="card">
      <h3>{poolOwnership.pool_name}</h3>

      <!-- Pool Total Summary Banner -->
      {#if stats && poolOwnership.entries.length > 0}
        {@const hasExpectedMin =
          stats.currentExpectedMinimum > 0 ||
          stats.projectedExpectedMinimum > 0 ||
          stats.maxExpectedMinimum > 0}
        {@const hasWarning = projection.isRangeMode
          ? stats.isBelowExpected
          : hasExpectedMin && stats.isBelowExpected}
        <div class="pool-total-banner" class:has-warning={hasWarning}>
          {#if projection.isRangeMode && stats.isBelowExpected}
            <div class="pool-warning-badge">
              <TriangleAlert size={18} class="warning-icon" color="currentColor" />
              <span class="warning-text">
                {hasExpectedMin ? $_('overview.belowExpected') : $_('overview.poolNegativeWarning')}
              </span>
            </div>
          {:else if !projection.isRangeMode && hasExpectedMin && stats.isBelowExpected}
            <div class="pool-warning-badge">
              <TriangleAlert size={18} class="warning-icon" color="currentColor" />
              <span class="warning-text">{$_('overview.belowExpected')}</span>
            </div>
          {/if}
          <div class="pool-stats-sections">
            <div class="pool-stats-section">
              <span class="section-label">{$_('overview.balance')}</span>
              <div class="pool-total-stats">
                <div class="pool-stat">
                  {#if projection.isRangeMode}
                    <span class="stat-label">{$_('overview.start')}</span>
                  {/if}
                  <span
                    class="stat-value"
                    class:positive={stats.currentTotal > 0}
                    class:negative={stats.currentTotal < 0}
                  >
                    {stats.currentTotal >= 0 ? '+' : ''}{formatCurrency(stats.currentTotal)}
                    {#if hasExpectedMin}
                      {@const diff = stats.currentTotal - stats.currentExpectedMinimum}
                      <span class="stat-diff" class:positive={diff >= 0} class:negative={diff < 0}>
                        ({diff >= 0
                          ? $_('overview.aboveExpected', {
                              values: { amount: formatCurrency(diff) }
                            })
                          : $_('overview.belowExpectedBy', {
                              values: { amount: formatCurrency(Math.abs(diff)) }
                            })})
                      </span>
                    {/if}
                  </span>
                </div>
                {#if projection.isRangeMode}
                  <div class="pool-stat">
                    <span class="stat-label">{$_('overview.end')}</span>
                    <span
                      class="stat-value"
                      class:positive={stats.projectedTotal > 0}
                      class:negative={stats.projectedTotal < 0}
                    >
                      {stats.projectedTotal >= 0 ? '+' : ''}{formatCurrency(stats.projectedTotal)}
                      {#if hasExpectedMin}
                        {@const diff = stats.projectedTotal - stats.projectedExpectedMinimum}
                        <span
                          class="stat-diff"
                          class:positive={diff >= 0}
                          class:negative={diff < 0}
                        >
                          ({diff >= 0
                            ? $_('overview.aboveExpected', {
                                values: { amount: formatCurrency(diff) }
                              })
                            : $_('overview.belowExpectedBy', {
                                values: { amount: formatCurrency(Math.abs(diff)) }
                              })})
                        </span>
                      {/if}
                    </span>
                  </div>
                  <div class="pool-stat">
                    <span class="stat-label">{$_('overview.lowest')}</span>
                    <span
                      class="stat-value"
                      class:positive={stats.minTotal > 0}
                      class:negative={stats.minTotal < 0}
                    >
                      {stats.minTotal >= 0 ? '+' : ''}{formatCurrency(stats.minTotal)}
                      {#if hasExpectedMin}
                        {@const diff = stats.minTotal - stats.maxExpectedMinimum}
                        <span
                          class="stat-diff"
                          class:positive={diff >= 0}
                          class:negative={diff < 0}
                        >
                          ({diff >= 0
                            ? $_('overview.aboveExpected', {
                                values: { amount: formatCurrency(diff) }
                              })
                            : $_('overview.belowExpectedBy', {
                                values: { amount: formatCurrency(Math.abs(diff)) }
                              })})
                        </span>
                      {/if}
                    </span>
                  </div>
                {/if}
              </div>
            </div>
            {#if hasExpectedMin}
              <div class="pool-stats-section">
                <span class="section-label">{$_('overview.expectedMinimum')}</span>
                <div class="pool-total-stats">
                  <div class="pool-stat">
                    {#if projection.isRangeMode}
                      <span class="stat-label">{$_('overview.start')}</span>
                    {/if}
                    <span class="stat-value">
                      {formatCurrency(stats.currentExpectedMinimum)}
                    </span>
                  </div>
                  {#if projection.isRangeMode}
                    <div class="pool-stat">
                      <span class="stat-label">{$_('overview.end')}</span>
                      <span class="stat-value">
                        {formatCurrency(stats.projectedExpectedMinimum)}
                      </span>
                    </div>
                    <div class="pool-stat">
                      <span class="stat-label">{$_('overview.highest')}</span>
                      <span class="stat-value">
                        {formatCurrency(stats.maxExpectedMinimum)}
                      </span>
                    </div>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      {#if poolOwnership.entries.length === 0}
        <p class="no-ownership">{$_('overview.noPoolActivity')}</p>
      {:else if projection.isRangeMode && chartReady}
        <div class="chart-container">
          <canvas use:bindPoolCanvas={{ poolId: poolOwnership.pool_id }}></canvas>
        </div>
      {:else}
        {@const tableHasExpectedMin =
          stats &&
          (stats.currentExpectedMinimum > 0 ||
            stats.projectedExpectedMinimum > 0 ||
            stats.maxExpectedMinimum > 0)}
        {@const participantExpMinMap = poolParticipantExpectedMinimum.get(poolOwnership.pool_id)}
        <table class="balance-table">
          <thead>
            <tr>
              <th></th>
              <th>{$_('overview.participant')}</th>
              <th>{$_('overview.ownership')}</th>
            </tr>
          </thead>
          <tbody>
            {#each poolOwnership.entries as entry (entry.participant_id)}
              {@const isExpanded = isPoolEntryExpanded(entry.participant_id)}
              {@const entryExpMin = participantExpMinMap?.get(entry.participant_id) ?? 0}
              {@const entryDiff = entry.ownership - entryExpMin}
              <tr
                class="balance-row"
                class:expanded={isExpanded}
                class:focused={projection.focusParticipantId === entry.participant_id}
                onclick={() => togglePoolEntry(entry.participant_id)}
              >
                <td class="expand-cell">
                  <span class="expand-icon">{isExpanded ? '▼' : '▶'}</span>
                </td>
                <td>{entry.participant_name}</td>
                <td class:positive={entry.ownership > 0} class:negative={entry.ownership < 0}>
                  {entry.ownership >= 0 ? '+' : ''}{formatCurrency(entry.ownership)}
                  {#if tableHasExpectedMin}
                    <span
                      class="stat-diff"
                      class:positive={entryDiff >= 0}
                      class:negative={entryDiff < 0}
                    >
                      ({entryDiff >= 0
                        ? $_('overview.aboveExpected', {
                            values: { amount: formatCurrency(entryDiff) }
                          })
                        : $_('overview.belowExpectedBy', {
                            values: { amount: formatCurrency(Math.abs(entryDiff)) }
                          })})
                    </span>
                  {/if}
                </td>
              </tr>
              {#if isExpanded}
                <tr class="pairwise-row">
                  <td colspan="3">
                    <div class="pairwise-details">
                      {#if entry.contributed > 0.01}
                        <div class="detail-section">
                          <div class="detail-header">
                            {$_('overview.amountContributed', {
                              values: {
                                payer: entry.participant_name,
                                amount: formatCurrency(entry.contributed)
                              }
                            })}
                          </div>
                          {#if entry.contributed_breakdown && entry.contributed_breakdown.length > 0}
                            {@const groupedYears = groupBreakdownByYearMonth(
                              entry.contributed_breakdown
                            )}
                            <div class="breakdown-hierarchy">
                              {#each [...groupedYears.entries()] as [year, yearData] (year)}
                                {@const yearExpanded =
                                  expandedPoolYears.get(entry.participant_id)?.has(year) ?? false}
                                <div class="breakdown-year">
                                  <button
                                    class="breakdown-year-btn"
                                    onclick={() => togglePoolYear(entry.participant_id, year)}
                                  >
                                    <span class="expand-icon">{yearExpanded ? '▼' : '▶'}</span>
                                    <span class="year-text">{year}</span>
                                    <span class="year-total">{formatCurrency(yearData.total)}</span>
                                  </button>
                                  {#if yearExpanded}
                                    {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                                      {@const monthExpanded =
                                        expandedPoolMonths
                                          .get(entry.participant_id)
                                          ?.get(year)
                                          ?.has(monthKey) ?? false}
                                      <div class="breakdown-month">
                                        <button
                                          class="breakdown-month-btn"
                                          onclick={() =>
                                            togglePoolMonth(entry.participant_id, year, monthKey)}
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
                                              {@const receiver = payment?.receiver_account_id
                                                ? $participants.find(
                                                    (pr) => pr.id === payment.receiver_account_id
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
                                                affectsBalance={payment?.affects_balance ?? true}
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
                      {#if entry.consumed > 0.01}
                        <div class="detail-section">
                          <div class="detail-header">
                            {$_('overview.amountConsumed', {
                              values: {
                                consumer: entry.participant_name,
                                amount: formatCurrency(entry.consumed)
                              }
                            })}
                          </div>
                          {#if entry.consumed_breakdown && entry.consumed_breakdown.length > 0}
                            {@const groupedYears = groupBreakdownByYearMonth(
                              entry.consumed_breakdown
                            )}
                            <div class="breakdown-hierarchy">
                              {#each [...groupedYears.entries()] as [year, yearData] (year)}
                                {@const yearExpanded =
                                  expandedPoolYears
                                    .get(entry.participant_id)
                                    ?.has(`${year}-consumed`) ?? false}
                                <div class="breakdown-year">
                                  <button
                                    class="breakdown-year-btn"
                                    onclick={() =>
                                      togglePoolYear(entry.participant_id, `${year}-consumed`)}
                                  >
                                    <span class="expand-icon">{yearExpanded ? '▼' : '▶'}</span>
                                    <span class="year-text">{year}</span>
                                    <span class="year-total">{formatCurrency(yearData.total)}</span>
                                  </button>
                                  {#if yearExpanded}
                                    {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                                      {@const monthExpanded =
                                        expandedPoolMonths
                                          .get(entry.participant_id)
                                          ?.get(`${year}-consumed`)
                                          ?.has(monthKey) ?? false}
                                      <div class="breakdown-month">
                                        <button
                                          class="breakdown-month-btn"
                                          onclick={() =>
                                            togglePoolMonth(
                                              entry.participant_id,
                                              `${year}-consumed`,
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
                                              {@const receiver = payment?.receiver_account_id
                                                ? $participants.find(
                                                    (pr) => pr.id === payment.receiver_account_id
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
                                                affectsBalance={payment?.affects_balance ?? true}
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
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/each}

  {#each filteredPoolOwnerships as poolOwnership (poolOwnership.pool_id)}
    {@const occs = poolOccurrences.get(poolOwnership.pool_id) ?? []}
    {#if occs.length > 0}
      <OccurrencesList
        occurrences={occs}
        paymentMap={projection.paymentMap}
        focusParticipantId={projection.focusParticipantId}
        formatDate={projection.formatDate}
        title="{$_('overview.transactionsIncluded')} — {poolOwnership.pool_name}"
      />
    {/if}
  {/each}
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

  .pool-total-banner {
    background: linear-gradient(135deg, rgba(123, 97, 255, 0.08), rgba(123, 97, 255, 0.02));
    border: 1px solid rgba(123, 97, 255, 0.2);
    border-radius: 12px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .pool-total-banner.has-warning {
    background: linear-gradient(135deg, rgba(220, 53, 69, 0.08), rgba(220, 53, 69, 0.02));
    border-color: rgba(220, 53, 69, 0.3);
  }

  .pool-warning-badge {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(220, 53, 69, 0.15);
    color: #dc3545;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    margin-bottom: 0.75rem;
    font-weight: 500;
  }

  .warning-text {
    font-size: 0.9rem;
  }

  .pool-stats-sections {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .pool-stats-section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .section-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .pool-total-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
  }

  .pool-stat {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 120px;
  }

  .stat-label {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .stat-value {
    font-size: 1.1rem;
    font-weight: 600;
    color: #333;
  }

  .stat-value.positive {
    color: #198754;
  }

  .stat-value.negative {
    color: #dc3545;
  }

  .stat-diff {
    display: block;
    font-size: 0.85rem;
    font-weight: 400;
  }

  .stat-diff.positive {
    color: #198754;
  }

  .stat-diff.negative {
    color: #dc3545;
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

  .no-ownership {
    color: #888;
    font-style: italic;
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
</style>
