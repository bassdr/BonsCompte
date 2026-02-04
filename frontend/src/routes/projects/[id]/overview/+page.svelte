<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import {
    getDebts,
    getPayments,
    type DebtSummary,
    type PaymentWithContributions,
    type PairwiseBalance,
    type PaymentOccurrence
  } from '$lib/api';
  import { _ } from '$lib/i18n';
  import { participants } from '$lib/stores/project';
  import { formatDateWithWeekday, formatMonthYear as formatMonthYearI18n } from '$lib/format/date';
  import { formatCurrency } from '$lib/format/currency';
  import { SvelteSet, SvelteMap, SvelteDate } from 'svelte/reactivity';
  import type { PairwisePaymentBreakdown } from '$lib/api';
  import { Chart, registerables } from 'chart.js';
  import { getErrorKey } from '$lib/errors';
  import 'chartjs-adapter-date-fns';

  // Chart.js annotation plugin - loaded dynamically for SSR safety
  let annotationPlugin: typeof import('chartjs-plugin-annotation').default | null = null;
  let chartReady = $state(false);

  type XYPoint = { x: string; y: number };
  type LineChart = Chart<'line', XYPoint[], unknown>;

  // Chart instances for cleanup (not reactive - just for cleanup/destroy)
  let balanceChart: LineChart | null = null;

  // This Map is written by the chart, it creates an endless loop if we change it to a SvelteMap
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  let poolCharts = new Map<number, LineChart>();

  // Chart canvas references (not reactive - just for DOM binding)
  let balanceChartCanvas: HTMLCanvasElement | null = $state(null);

  // This Map is written by the chart, it creates an endless loop if we change it to a SvelteMap
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  let poolChartCanvases = new Map<number, HTMLCanvasElement>();

  // Colors for chart lines
  const CHART_COLORS = [
    '#7b61ff', // accent purple
    '#2a9d8f', // teal
    '#e76f51', // coral
    '#f4a261', // orange
    '#264653', // dark blue
    '#e9c46a', // yellow
    '#6c757d', // gray
    '#dc3545' // red
  ];

  // Type definitions for breakdown grouping
  interface BreakdownMonthData {
    monthKey: string;
    month: string;
    total: number;
    items: PairwisePaymentBreakdown[];
  }

  interface BreakdownYearData {
    year: string;
    total: number;
    months: SvelteMap<string, BreakdownMonthData>;
  }

  interface OccurrencesByDateData {
    date: string;
    dateDisplay: string;
    occurrences: PaymentOccurrence[];
    totalAmount: number;
  }

  interface OccurrencesByMonthData {
    monthKey: string;
    month: string;
    expanded: boolean;
    totalAmount: number;
    dates: SvelteMap<string, OccurrencesByDateData>;
    participantTotals: Map<string, number>;
  }

  interface OccurrencesByYearData {
    year: string;
    expanded: boolean;
    months: SvelteMap<string, OccurrencesByMonthData>;
  }

  let debts: DebtSummary | null = $state(null);
  let startDebts: DebtSummary | null = $state(null); // Debts at targetDate for range mode
  let allPayments: PaymentWithContributions[] = $state([]);
  let loading = $state(true);
  let errorKey = $state('');
  let showOccurrences = $state(false);

  // Track which balance rows are expanded to show pairwise details
  let expandedBalanceRows = new SvelteSet<number>();

  // Track which pool ownership entries are expanded
  let expandedPoolEntries = new SvelteSet<number>();
  let expandedPoolYears = new SvelteMap<number, SvelteSet<string>>();
  let expandedPoolMonths = new SvelteMap<number, SvelteMap<string, SvelteSet<string>>>();

  // Focus mode: filter to show only one participant's perspective
  let focusParticipantId = $state<number | null>(null);

  // Settlement mode: false = minimal (default), true = direct-only
  let useDirectSettlements = $state(false);

  // Include draft payments in calculations
  let includeDrafts = $state(false);

  function toggleBalanceRow(participantId: number) {
    if (expandedBalanceRows.has(participantId)) {
      expandedBalanceRows.delete(participantId);
    } else {
      expandedBalanceRows.add(participantId);
    }
  }

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

  // Track which pairwise rows are expanded
  let expandedPairwise = new SvelteSet<string>();
  let expandedPairwiseYears = new SvelteMap<string, SvelteSet<string>>();
  let expandedPairwiseMonths = new SvelteMap<string, SvelteMap<string, SvelteSet<string>>>();

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

  // Group breakdown by year/month, newest first
  function groupBreakdownByYearMonth(
    breakdown: PairwisePaymentBreakdown[]
  ): SvelteMap<string, BreakdownYearData> {
    const yearMap = new SvelteMap<string, BreakdownYearData>();

    for (const item of breakdown) {
      const year = getYear(item.occurrence_date);
      const monthKey = getMonthKey(item.occurrence_date);

      if (!yearMap.has(year)) {
        yearMap.set(year, {
          year,
          total: 0,
          months: new SvelteMap()
        });
      }

      const yearData = yearMap.get(year)!;
      if (!yearData.months.has(monthKey)) {
        yearData.months.set(monthKey, {
          monthKey,
          month: formatMonthYear(item.occurrence_date),
          total: 0,
          items: []
        });
      }

      const monthData = yearData.months.get(monthKey)!;
      monthData.items.push(item);
      monthData.total += item.amount;
      yearData.total += item.amount;
    }

    // Convert to array and sort by year (newest first)
    const sortedYears = [...yearMap.entries()].sort((a, b) => b[0].localeCompare(a[0]));

    // For each year, sort months (newest first) and items (newest first)
    const result = new SvelteMap<string, BreakdownYearData>();
    for (const [year, yearData] of sortedYears) {
      const sortedMonths = [...yearData.months.entries()].sort((a, b) => b[0].localeCompare(a[0]));

      yearData.months = new SvelteMap();
      for (const [monthKey, monthData] of sortedMonths) {
        monthData.items.sort((a, b) => b.occurrence_date.localeCompare(a.occurrence_date));
        yearData.months.set(monthKey, monthData);
      }

      result.set(year, yearData);
    }

    return result;
  }

  // Get pairwise balances for a specific participant
  function getPairwiseForParticipant(participantId: number): PairwiseBalance[] {
    if (!debts?.pairwise_balances) return [];
    return debts.pairwise_balances.filter((p) => p.participant_id === participantId);
  }

  // Check if a settlement involves the focused participant
  function settlementInvolvesFocus(s: {
    from_participant_id: number;
    to_participant_id: number;
  }): boolean {
    if (focusParticipantId === null) return true;
    return (
      s.from_participant_id === focusParticipantId || s.to_participant_id === focusParticipantId
    );
  }

  // Check if a payment involves the focused participant (as payer or contributor)
  function paymentInvolvesFocus(paymentId: number, payerId: number | null): boolean {
    if (focusParticipantId === null) return true;
    if (payerId === focusParticipantId) return true;

    const payment = paymentMap.get(paymentId);
    if (payment?.contributions) {
      return payment.contributions.some(
        (c: { participant_id: number }) => c.participant_id === focusParticipantId
      );
    }
    return false;
  }

  // Get active settlements based on mode (for end date / current view)
  let activeSettlements = $derived.by(() => {
    if (!debts) return [];
    return useDirectSettlements ? debts.direct_settlements : debts.settlements;
  });

  // Get active settlements at start date (for range mode comparison)
  let startActiveSettlements = $derived.by(() => {
    if (!startDebts) return [];
    return useDirectSettlements ? startDebts.direct_settlements : startDebts.settlements;
  });

  // Get filtered settlements based on focus (end date)
  let filteredSettlements = $derived.by(() => {
    if (!debts) return [];
    if (focusParticipantId === null) return activeSettlements;
    return activeSettlements.filter((s) => settlementInvolvesFocus(s));
  });

  // Get filtered settlements at start date (for range mode)
  let filteredStartSettlements = $derived.by(() => {
    if (!startDebts) return [];
    if (focusParticipantId === null) return startActiveSettlements;
    return startActiveSettlements.filter((s) => settlementInvolvesFocus(s));
  });

  // Get filtered pool ownerships based on focus
  let filteredPoolOwnerships = $derived.by(() => {
    if (!debts?.pool_ownerships || debts.pool_ownerships.length === 0) return [];
    if (focusParticipantId === null) return debts.pool_ownerships;

    return debts.pool_ownerships
      .map((pool) => {
        const filteredEntries = pool.entries.filter((e) => e.participant_id === focusParticipantId);
        return {
          ...pool,
          entries: filteredEntries,
          total_balance: filteredEntries.reduce((sum, e) => sum + e.ownership, 0)
        };
      })
      .filter((pool) => pool.entries.length > 0);
  });

  // Get pool IDs for filtering balances
  let poolIds = $derived.by(() => {
    if (!debts?.pool_ownerships) return new Set<number>();
    return new SvelteSet(debts.pool_ownerships.map((p) => p.pool_id));
  });

  // Get non-pool balances for display and selector
  let nonPoolBalances = $derived.by(() => {
    if (!debts) return [];
    return debts.balances.filter((b) => !poolIds.has(b.participant_id));
  });

  // Get filtered balances based on focus
  let filteredBalances = $derived.by(() => {
    if (!debts) return [];
    if (focusParticipantId === null) {
      return nonPoolBalances;
    }
    return nonPoolBalances.filter((b) => b.participant_id === focusParticipantId);
  });

  // Date state - default to today (using local date)
  function getLocalDateString(date: Date = new SvelteDate()): string {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}-${month}-${day}`;
  }

  function parseLocalDate(dateStr: string): Date {
    const [year, month, day] = dateStr.split('-').map(Number);
    return new SvelteDate(year, month - 1, day);
  }

  let targetDate = $state(getLocalDateString());
  let endDate = $state<string | null>(null);

  // Horizon options for the range selector
  type HorizonOption =
    | 'none'
    | 'thisMonth'
    | 'nextMonth'
    | '3months'
    | '6months'
    | '12months'
    | 'custom';

  // Compute end date for a given horizon from a start date
  function getHorizonEndDate(startDate: string, months: number): string {
    const date = parseLocalDate(startDate);
    date.setMonth(date.getMonth() + months);
    return getLocalDateString(date);
  }

  // Precomputed horizon dates (relative to targetDate)
  const now = new Date();
  let horizonThisMonth = $derived(
    getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 1, 0))
  );
  let horizonNextMonth = $derived(
    getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 2, 0))
  );
  let horizon3Months = $derived(getHorizonEndDate(targetDate, 3));
  let horizon6Months = $derived(getHorizonEndDate(targetDate, 6));
  let horizon12Months = $derived(getHorizonEndDate(targetDate, 12));

  // Track the user's explicitly chosen horizon (not derived, but set by user action)
  let chosenHorizon = $state<HorizonOption>('none');

  // Detect which horizon the current endDate matches (for UI button highlighting)
  let selectedHorizon = $derived.by((): HorizonOption => {
    if (endDate === null) return 'none';
    if (endDate === horizonThisMonth) return 'thisMonth';
    if (endDate === horizonNextMonth) return 'nextMonth';
    if (endDate === horizon3Months) return '3months';
    if (endDate === horizon6Months) return '6months';
    if (endDate === horizon12Months) return '12months';
    return 'custom';
  });

  // Set horizon by option
  function setHorizon(option: HorizonOption) {
    chosenHorizon = option;
    switch (option) {
      case 'none':
        endDate = null;
        break;
      case 'thisMonth':
        endDate = horizonThisMonth;
        break;
      case 'nextMonth':
        endDate = horizonNextMonth;
        break;
      case '3months':
        endDate = horizon3Months;
        break;
      case '6months':
        endDate = horizon6Months;
        break;
      case '12months':
        endDate = horizon12Months;
        break;
    }
  }

  // Effect: Keep endDate in sync with preset horizons when targetDate changes
  // This ensures 3/6/12 months stay selected when navigating start dates
  $effect(() => {
    // Only apply when user has explicitly chosen a preset horizon
    if (chosenHorizon === 'thisMonth') {
      endDate = horizonThisMonth;
    } else if (chosenHorizon === 'nextMonth') {
      endDate = horizonNextMonth;
    } else if (chosenHorizon === '3months') {
      endDate = horizon3Months;
    } else if (chosenHorizon === '6months') {
      endDate = horizon6Months;
    } else if (chosenHorizon === '12months') {
      endDate = horizon12Months;
    }
  });

  // Effect: Reset horizon to None if startDate >= endDate (invalid range)
  $effect(() => {
    if (endDate !== null && targetDate >= endDate) {
      endDate = null;
      chosenHorizon = 'none';
    }
  });

  // Range mode is active when endDate is set and > targetDate
  let isRangeMode = $derived(endDate !== null && endDate > targetDate);

  let projectId = $derived(parseInt($page.params.id ?? ''));

  // Initialize Chart.js (SSR-safe)
  onMount(() => {
    Chart.register(...registerables);

    // Load annotation plugin dynamically
    import('chartjs-plugin-annotation').then((mod) => {
      annotationPlugin = mod.default;
      Chart.register(annotationPlugin);
      chartReady = true;
    });

    // Cleanup on destroy
    return () => {
      if (balanceChart) {
        balanceChart.destroy();
        balanceChart = null;
      }
      poolCharts.forEach((chart) => chart.destroy());
      poolCharts.clear();
    };
  });

  // Get payment dates in range for x-axis
  let rangeDates = $derived.by(() => {
    if (!isRangeMode || !debts || !endDate) return [];
    const end = endDate; // Local variable for type narrowing
    // Filter payment dates to those within range
    return paymentDates.filter((d) => d >= targetDate && d <= end);
  });

  // Compute running balances at each date in range
  // Returns SvelteMap<date, SvelteMap<participantId, netBalance>>

  let balanceTimeSeries = $derived.by(() => {
    if (!isRangeMode || !debts || rangeDates.length === 0)
      return new SvelteMap<string, Map<number, number>>();

    const result = new SvelteMap<string, Map<number, number>>();
    const runningBalance = new SvelteMap<number, number>(); // participantId -> balance

    // Sort occurrences by date
    const sortedOccurrences = [...debts.occurrences].sort((a, b) =>
      a.occurrence_date.localeCompare(b.occurrence_date)
    );

    // Initialize all participants with 0 balance
    for (const balance of debts.balances) {
      if (!poolIds.has(balance.participant_id)) {
        runningBalance.set(balance.participant_id, 0);
      }
    }

    let occIdx = 0;

    // Process each date in the range
    for (const date of rangeDates) {
      // Process all occurrences up to and including this date
      while (
        occIdx < sortedOccurrences.length &&
        sortedOccurrences[occIdx].occurrence_date <= date
      ) {
        const occ = sortedOccurrences[occIdx];
        const payment = paymentMap.get(occ.payment_id);

        if (payment) {
          // Check if this is a pool-related transaction (should be excluded from user balances)
          const isPoolReceiver =
            occ.receiver_account_id !== null && poolIds.has(occ.receiver_account_id);
          const isPoolPayer = occ.payer_id !== null && poolIds.has(occ.payer_id);

          // Skip pool transfers for user balance computation
          if (!isPoolReceiver && !isPoolPayer) {
            if (occ.receiver_account_id !== null) {
              // User-to-user transfer or external inflow
              if (occ.payer_id !== null) {
                // User-to-user transfer: payer credit, receiver debit
                const payerCurrent = runningBalance.get(occ.payer_id) ?? 0;
                runningBalance.set(occ.payer_id, payerCurrent + occ.amount);

                const receiverCurrent = runningBalance.get(occ.receiver_account_id) ?? 0;
                runningBalance.set(occ.receiver_account_id, receiverCurrent - occ.amount);
              } else {
                // External inflow: receiver debit (holds money), contributors credit (owed their share)
                const receiverCurrent = runningBalance.get(occ.receiver_account_id) ?? 0;
                runningBalance.set(occ.receiver_account_id, receiverCurrent - occ.amount);

                if (payment.contributions) {
                  for (const contrib of payment.contributions) {
                    if (!poolIds.has(contrib.participant_id)) {
                      const current = runningBalance.get(contrib.participant_id) ?? 0;
                      runningBalance.set(contrib.participant_id, current + contrib.amount);
                    }
                  }
                }
              }
            } else {
              // External expense: payer credit, contributors debit
              if (occ.payer_id !== null && !poolIds.has(occ.payer_id)) {
                const current = runningBalance.get(occ.payer_id) ?? 0;
                runningBalance.set(occ.payer_id, current + occ.amount);
              }

              if (payment.contributions) {
                for (const contrib of payment.contributions) {
                  if (!poolIds.has(contrib.participant_id)) {
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

      // Snapshot current balances at this date
      result.set(date, new Map(runningBalance));
    }

    return result;
  });

  // Compute running pairwise balances for focus mode
  // Returns SvelteMap<date, SvelteMap<otherParticipantId, balance>>

  let pairwiseTimeSeries = $derived.by(() => {
    if (!isRangeMode || !debts || !focusParticipantId || rangeDates.length === 0) {
      return new SvelteMap<string, Map<number, number>>();
    }

    const result = new SvelteMap<string, Map<number, number>>();
    const runningPairwise = new SvelteMap<number, number>(); // otherParticipantId -> balance

    // Sort occurrences by date
    const sortedOccurrences = [...debts.occurrences].sort((a, b) =>
      a.occurrence_date.localeCompare(b.occurrence_date)
    );

    // Initialize all other participants with 0 balance
    for (const balance of debts.balances) {
      if (balance.participant_id !== focusParticipantId && !poolIds.has(balance.participant_id)) {
        runningPairwise.set(balance.participant_id, 0);
      }
    }

    let occIdx = 0;

    for (const date of rangeDates) {
      while (
        occIdx < sortedOccurrences.length &&
        sortedOccurrences[occIdx].occurrence_date <= date
      ) {
        const occ = sortedOccurrences[occIdx];
        const payment = paymentMap.get(occ.payment_id);

        if (payment) {
          const isPoolReceiver =
            occ.receiver_account_id !== null && poolIds.has(occ.receiver_account_id);
          const isPoolPayer = occ.payer_id !== null && poolIds.has(occ.payer_id);

          if (!isPoolReceiver && !isPoolPayer) {
            if (occ.receiver_account_id !== null) {
              // User-to-user transfer or external inflow
              if (occ.payer_id !== null) {
                // User-to-user transfer: direct payment from payer to receiver
                if (occ.payer_id === focusParticipantId) {
                  // Focused participant paid the receiver
                  const current = runningPairwise.get(occ.receiver_account_id) ?? 0;
                  runningPairwise.set(occ.receiver_account_id, current + occ.amount);
                } else if (occ.receiver_account_id === focusParticipantId) {
                  // Focused participant received from the payer
                  const current = runningPairwise.get(occ.payer_id) ?? 0;
                  runningPairwise.set(occ.payer_id, current - occ.amount);
                }
              } else {
                // External inflow: receiver holds money for contributors (owes them their share)
                if (occ.receiver_account_id === focusParticipantId && payment.contributions) {
                  // Focused participant received external funds, owes others their share
                  for (const contrib of payment.contributions) {
                    if (
                      contrib.participant_id !== focusParticipantId &&
                      !poolIds.has(contrib.participant_id)
                    ) {
                      const current = runningPairwise.get(contrib.participant_id) ?? 0;
                      runningPairwise.set(contrib.participant_id, current - contrib.amount);
                    }
                  }
                } else if (payment.contributions) {
                  // Check if focused participant is a contributor
                  const focusedContrib = payment.contributions.find(
                    (c) => c.participant_id === focusParticipantId
                  );
                  if (focusedContrib && !poolIds.has(occ.receiver_account_id)) {
                    // Someone else received external funds, they owe focused participant their share
                    const current = runningPairwise.get(occ.receiver_account_id) ?? 0;
                    runningPairwise.set(occ.receiver_account_id, current + focusedContrib.amount);
                  }
                }
              }
            } else {
              // External expense: payer paid for contributors
              if (occ.payer_id === focusParticipantId && payment.contributions) {
                for (const contrib of payment.contributions) {
                  if (
                    contrib.participant_id !== focusParticipantId &&
                    !poolIds.has(contrib.participant_id)
                  ) {
                    // Focused participant paid for this other person
                    const current = runningPairwise.get(contrib.participant_id) ?? 0;
                    runningPairwise.set(contrib.participant_id, current + contrib.amount);
                  }
                }
              }

              // If focused participant is a contributor and someone else paid
              if (
                occ.payer_id !== focusParticipantId &&
                occ.payer_id !== null &&
                !poolIds.has(occ.payer_id)
              ) {
                if (payment.contributions) {
                  const focusedContrib = payment.contributions.find(
                    (c) => c.participant_id === focusParticipantId
                  );
                  if (focusedContrib) {
                    // Someone else paid for the focused participant
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

  // Compute running pool ownership at each date
  // Returns SvelteMap<poolId, SvelteMap<date, Map<participantId, ownership>>>

  let poolOwnershipTimeSeries = $derived.by(() => {
    if (!isRangeMode || !debts || rangeDates.length === 0 || !debts.pool_ownerships) {
      return new SvelteMap<number, Map<string, Map<number, number>>>();
    }

    const result = new SvelteMap<number, Map<string, Map<number, number>>>();

    // Initialize for each pool
    for (const pool of debts.pool_ownerships) {
      const poolTimeSeries = new SvelteMap<string, Map<number, number>>();
      const runningOwnership = new SvelteMap<number, number>();

      // Initialize all participants with 0 ownership
      for (const entry of pool.entries) {
        runningOwnership.set(entry.participant_id, 0);
      }

      // Sort occurrences by date
      const sortedOccurrences = [...debts.occurrences].sort((a, b) =>
        a.occurrence_date.localeCompare(b.occurrence_date)
      );

      let occIdx = 0;

      for (const date of rangeDates) {
        while (
          occIdx < sortedOccurrences.length &&
          sortedOccurrences[occIdx].occurrence_date <= date
        ) {
          const occ = sortedOccurrences[occIdx];
          const payment = paymentMap.get(occ.payment_id);

          // Only update actual balance if affects_balance is true
          if (payment && occ.affects_balance) {
            // User -> Pool transfer (deposit)
            if (occ.receiver_account_id === pool.pool_id && occ.payer_id !== null) {
              const current = runningOwnership.get(occ.payer_id) ?? 0;
              runningOwnership.set(occ.payer_id, current + occ.amount);
            }

            // Pool -> User transfer (withdrawal) - pool is the payer
            if (occ.payer_id === pool.pool_id && occ.receiver_account_id !== null) {
              const current = runningOwnership.get(occ.receiver_account_id) ?? 0;
              runningOwnership.set(occ.receiver_account_id, current - occ.amount);
            }

            // Pool pays external expense - distribute consumption among contributors
            if (
              occ.payer_id === pool.pool_id &&
              occ.receiver_account_id === null &&
              payment.contributions
            ) {
              for (const contrib of payment.contributions) {
                if (!poolIds.has(contrib.participant_id)) {
                  const current = runningOwnership.get(contrib.participant_id) ?? 0;
                  runningOwnership.set(contrib.participant_id, current - contrib.amount);
                }
              }
            }

            // External funds to pool - contributors brought in money, their ownership increases
            if (
              occ.receiver_account_id === pool.pool_id &&
              occ.payer_id === null &&
              payment.contributions
            ) {
              for (const contrib of payment.contributions) {
                if (!poolIds.has(contrib.participant_id)) {
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

  // Compute pool total time series (sum of all participant ownerships per date)
  let poolTotalTimeSeries = $derived.by(() => {
    if (!isRangeMode || poolOwnershipTimeSeries.size === 0 || rangeDates.length === 0) {
      return new SvelteMap<number, Map<string, number>>();
    }

    const result = new SvelteMap<number, Map<string, number>>();

    for (const [poolId, poolData] of poolOwnershipTimeSeries) {
      const totals = new SvelteMap<string, number>();

      for (const date of rangeDates) {
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

  // Compute running pool expected minimum at each date (total and per-participant)
  // Returns SvelteMap<poolId, { total: SvelteMap<date, expectedMinimum>, perParticipant: SvelteMap<date, SvelteMap<participantId, expectedMinimum>> }>
  let poolExpectedMinimumTimeSeries = $derived.by(() => {
    if (!isRangeMode || !debts || rangeDates.length === 0 || !debts.pool_ownerships) {
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

    for (const pool of debts.pool_ownerships) {
      const totalTimeSeries = new SvelteMap<string, number>();
      const perParticipantTimeSeries = new SvelteMap<string, Map<number, number>>();

      let runningExpectedMin = 0;
      // Track per-participant expected minimum
      const participantExpectedMin = new SvelteMap<number, number>();
      for (const entry of pool.entries) {
        participantExpectedMin.set(entry.participant_id, 0);
      }

      // Sort occurrences by date
      const sortedOccurrences = [...debts.occurrences].sort((a, b) =>
        a.occurrence_date.localeCompare(b.occurrence_date)
      );

      let occIdx = 0;

      for (const date of rangeDates) {
        while (
          occIdx < sortedOccurrences.length &&
          sortedOccurrences[occIdx].occurrence_date <= date
        ) {
          const occ = sortedOccurrences[occIdx];

          // User -> Pool transfer (deposit) with affects_receiver_expectation=true
          if (
            occ.receiver_account_id === pool.pool_id &&
            occ.payer_id !== null &&
            occ.affects_receiver_expectation
          ) {
            runningExpectedMin += occ.amount;
            // The depositing user's expected min increases
            const current = participantExpectedMin.get(occ.payer_id) ?? 0;
            participantExpectedMin.set(occ.payer_id, current + occ.amount);
          }

          // Pool -> User transfer (withdrawal) with affects_payer_expectation=true
          if (
            occ.payer_id === pool.pool_id &&
            occ.receiver_account_id !== null &&
            occ.affects_payer_expectation
          ) {
            runningExpectedMin -= occ.amount;
            // The receiving user's expected min decreases
            const current = participantExpectedMin.get(occ.receiver_account_id) ?? 0;
            participantExpectedMin.set(occ.receiver_account_id, current - occ.amount);
          }

          // External funds to pool (including rules) with affects_receiver_expectation=true
          if (
            occ.receiver_account_id === pool.pool_id &&
            occ.payer_id === null &&
            occ.affects_receiver_expectation
          ) {
            runningExpectedMin += occ.amount;
            // Distribute among contributors based on their weights (look up payment)
            const payment = paymentMap.get(occ.payment_id);
            if (payment) {
              for (const contrib of payment.contributions) {
                const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
                participantExpectedMin.set(contrib.participant_id, current + contrib.amount);
              }
            }
          }

          // Pool pays external expense with affects_payer_expectation=true
          if (
            occ.payer_id === pool.pool_id &&
            occ.receiver_account_id === null &&
            occ.affects_payer_expectation
          ) {
            runningExpectedMin -= occ.amount;
            // Each contributor's share decreases their expected min (look up payment)
            const payment = paymentMap.get(occ.payment_id);
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
        // Store a snapshot of per-participant values for this date
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

  // Compute per-participant expected minimum for non-range mode (from occurrences)
  // Returns SvelteMap<poolId, SvelteMap<participantId, expectedMinimum>>
  let poolParticipantExpectedMinimum = $derived.by(() => {
    const result = new SvelteMap<number, Map<number, number>>();

    if (!debts?.pool_ownerships || !debts.occurrences) return result;

    for (const pool of debts.pool_ownerships) {
      const participantExpectedMin = new SvelteMap<number, number>();
      for (const entry of pool.entries) {
        participantExpectedMin.set(entry.participant_id, 0);
      }

      for (const occ of debts.occurrences) {
        // User -> Pool transfer (deposit) with affects_receiver_expectation=true
        if (
          occ.receiver_account_id === pool.pool_id &&
          occ.payer_id !== null &&
          occ.affects_receiver_expectation
        ) {
          const current = participantExpectedMin.get(occ.payer_id) ?? 0;
          participantExpectedMin.set(occ.payer_id, current + occ.amount);
        }

        // Pool -> User transfer (withdrawal) with affects_payer_expectation=true
        if (
          occ.payer_id === pool.pool_id &&
          occ.receiver_account_id !== null &&
          occ.affects_payer_expectation
        ) {
          const current = participantExpectedMin.get(occ.receiver_account_id) ?? 0;
          participantExpectedMin.set(occ.receiver_account_id, current - occ.amount);
        }

        // External funds to pool (including rules) with affects_receiver_expectation=true
        if (
          occ.receiver_account_id === pool.pool_id &&
          occ.payer_id === null &&
          occ.affects_receiver_expectation
        ) {
          const payment = paymentMap.get(occ.payment_id);
          if (payment) {
            for (const contrib of payment.contributions) {
              const current = participantExpectedMin.get(contrib.participant_id) ?? 0;
              participantExpectedMin.set(contrib.participant_id, current + contrib.amount);
            }
          }
        }

        // Pool pays external expense with affects_payer_expectation=true
        if (
          occ.payer_id === pool.pool_id &&
          occ.receiver_account_id === null &&
          occ.affects_payer_expectation
        ) {
          const payment = paymentMap.get(occ.payment_id);
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

  // Compute pool total stats (current, projected, min) for each pool
  interface PoolTotalStats {
    currentTotal: number;
    projectedTotal: number;
    minTotal: number;
    hasNegative: boolean;
    // Expected minimum fields
    currentExpectedMinimum: number;
    projectedExpectedMinimum: number; // value at end of period
    maxExpectedMinimum: number; // maximum expected minimum during period (the threshold to maintain)
    isBelowExpected: boolean; // true if balance < expectedMinimum at any point in range
  }

  let poolTotalStats = $derived.by(() => {
    const result = new SvelteMap<number, PoolTotalStats>();

    if (!debts?.pool_ownerships) return result;

    for (const pool of debts.pool_ownerships) {
      if (!isRangeMode) {
        // Non-range mode: use focused participant's ownership or total
        let currentValue: number;
        let currentExpectedMin: number;
        let isBelowExpected: boolean;

        if (focusParticipantId !== null) {
          // Focus mode: use focused participant's values
          const entry = pool.entries.find((e) => e.participant_id === focusParticipantId);
          currentValue = entry?.ownership ?? 0;
          // Get focused participant's expected minimum
          const participantExpMin = poolParticipantExpectedMinimum.get(pool.pool_id);
          currentExpectedMin = participantExpMin?.get(focusParticipantId) ?? 0;
          isBelowExpected = currentValue < currentExpectedMin;
        } else {
          // Normal mode: use pool totals
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
        // Range mode: compute from time series
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

        // Get ownership value for a date (total or focused participant)
        const getValueForDate = (date: string): number => {
          const participantOwnerships = poolData.get(date);
          if (!participantOwnerships) return 0;

          if (focusParticipantId !== null) {
            // Focus mode: use focused participant's ownership
            return participantOwnerships.get(focusParticipantId) ?? 0;
          } else {
            // Normal mode: sum all ownerships
            let sum = 0;
            for (const ownership of participantOwnerships.values()) {
              sum += ownership;
            }
            return sum;
          }
        };

        // Get expected minimum for a date (total or focused participant)
        const getExpectedMinForDate = (date: string): number => {
          if (!expectedMinData) return 0;
          if (focusParticipantId !== null) {
            // Focus mode: use focused participant's expected minimum
            return expectedMinData.perParticipant.get(date)?.get(focusParticipantId) ?? 0;
          } else {
            // Normal mode: use pool total expected minimum
            return expectedMinData.total.get(date) ?? 0;
          }
        };

        if (dates.length > 0) {
          currentTotal = getValueForDate(dates[0]);
          projectedTotal = getValueForDate(dates[dates.length - 1]);

          for (const date of dates) {
            const value = getValueForDate(date);
            const expectedMin = getExpectedMinForDate(date);

            // Track minimum balance
            if (value < minTotal) {
              minTotal = value;
            }
            if (value < 0) {
              hasNegative = true;
            }
            // Check if balance goes below expected minimum at any point
            if (value < expectedMin) {
              isBelowExpected = true;
            }
            // Track maximum expected minimum (the threshold to maintain)
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

  // Get today annotation for charts (only if today is within the visible range)
  function getTodayAnnotation() {
    if (!annotationPlugin) return {};

    // Don't show today line if it's outside the visible date range
    const isTodayInRange = todayStr >= targetDate && (!endDate || todayStr <= endDate);
    if (!isTodayInRange) return {};

    return {
      todayLine: {
        type: 'line' as const,
        xMin: todayStr,
        xMax: todayStr,
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

  // Render balance chart
  function renderBalanceChart(canvas: HTMLCanvasElement) {
    if (!chartReady || !isRangeMode || rangeDates.length === 0) return;

    // Destroy existing chart
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

    if (focusParticipantId !== null && pairwiseTimeSeries.size > 0) {
      // Focus mode: show pairwise balances
      const participantIds = [
        ...new Set([...pairwiseTimeSeries.values()].flatMap((m) => [...m.keys()]))
      ];

      participantIds.forEach((pid, idx) => {
        const otherBalance = debts?.balances.find((b) => b.participant_id === pid);
        if (!otherBalance) return;

        datasets.push({
          label: `vs ${otherBalance.participant_name}`,
          data: rangeDates.map((date) => ({
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
      // Normal mode: show all participant balances
      const participantIds = [
        ...new Set([...balanceTimeSeries.values()].flatMap((m) => [...m.keys()]))
      ];

      participantIds.forEach((pid, idx) => {
        const balance = debts?.balances.find((b) => b.participant_id === pid);
        if (!balance) return;

        datasets.push({
          label: balance.participant_name,
          data: rangeDates.map((date) => ({
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
              displayFormats: { day: 'MMM d' }
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

  // Render pool ownership chart
  function renderPoolChart(canvas: HTMLCanvasElement, poolId: number) {
    if (!chartReady || !isRangeMode || rangeDates.length === 0) return;

    // Destroy existing chart for this pool
    const existingChart = poolCharts.get(poolId);
    if (existingChart) {
      existingChart.destroy();
      poolCharts.delete(poolId);
    }

    const poolData = poolOwnershipTimeSeries.get(poolId);
    if (!poolData) return;

    const pool = debts?.pool_ownerships?.find((p) => p.pool_id === poolId);
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

    // Get participant IDs to show (filter by focus if applicable)
    const entriesToShow =
      focusParticipantId !== null
        ? pool.entries.filter((e) => e.participant_id === focusParticipantId)
        : pool.entries;

    // Track the index of the "balance" dataset for fill reference
    // In focus mode: it's the focused participant (index 0)
    // In normal mode: it's the Total line (added after participants)
    let balanceDatasetIndex = 0;

    entriesToShow.forEach((entry, idx) => {
      datasets.push({
        label: entry.participant_name,
        data: rangeDates.map((date) => ({
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

    // Add Total line (distinct styling: thick, dashed, dark)
    if (totalData && focusParticipantId === null) {
      balanceDatasetIndex = datasets.length; // Total line will be the balance reference
      datasets.push({
        label: $_('common.total'),
        data: rangeDates.map((date) => ({
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
        order: 0 // Draw on top
      });
    }

    // Add Expected Minimum line (only if there's expectation data)
    const expectedMinData = poolExpectedMinimumTimeSeries.get(poolId);

    // Helper to get expected minimum for a date (total or focused participant)
    const getExpectedMinForDate = (date: string): number => {
      if (!expectedMinData) return 0;
      if (focusParticipantId !== null) {
        // Focus mode: use focused participant's expected minimum
        return expectedMinData.perParticipant.get(date)?.get(focusParticipantId) ?? 0;
      } else {
        // Normal mode: use pool total expected minimum
        return expectedMinData.total.get(date) ?? 0;
      }
    };

    // Check if there's any expected min data (total or for focused participant)
    // Always show the line if there's expected minimum data, even if values are 0
    const hasExpectedMinData = expectedMinData !== undefined;

    if (hasExpectedMinData) {
      datasets.push({
        label: $_('overview.expectedMinimum'),
        data: rangeDates.map((date) => ({
          x: date,
          y: getExpectedMinForDate(date)
        })),
        borderColor: '#dc3545',
        backgroundColor: 'rgba(220, 53, 69, 0.15)',
        stepped: 'before',
        // Fill to balance dataset: show red area when expected min > balance
        fill: {
          target: balanceDatasetIndex,
          above: 'rgba(220, 53, 69, 0.15)', // When expected min is above balance (warning zone)
          below: 'transparent' // When expected min is below balance (no fill)
        },
        pointRadius: 2,
        pointHoverRadius: 4,
        borderWidth: 2,
        borderDash: [4, 4],
        order: 0
      });
    }

    // Build annotations: today line only (warning zones handled by dataset fill)
    const annotations = getTodayAnnotation();

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
              displayFormats: { day: 'MMM d' }
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

  // Effect to render balance chart when data or canvas changes
  $effect(() => {
    if (balanceChartCanvas && chartReady && isRangeMode && rangeDates.length > 0) {
      // Track dependencies
      const _ts = focusParticipantId !== null ? pairwiseTimeSeries : balanceTimeSeries;
      void _ts;
      renderBalanceChart(balanceChartCanvas);
    }
  });

  // Effect to render pool charts when data changes
  $effect(() => {
    if (chartReady && isRangeMode && rangeDates.length > 0 && debts?.pool_ownerships) {
      // Track dependencies
      const _pts = poolOwnershipTimeSeries;
      void _pts;

      for (const pool of debts.pool_ownerships) {
        const canvas = poolChartCanvases.get(pool.pool_id);
        if (canvas) {
          renderPoolChart(canvas, pool.pool_id);
        }
      }
    }
  });

  // Action to bind pool canvas to the map
  function bindPoolCanvas(node: HTMLCanvasElement, params: { poolId: number }) {
    poolChartCanvases.set(params.poolId, node);
    // Trigger render once the canvas is bound
    if (chartReady && isRangeMode && rangeDates.length > 0) {
      renderPoolChart(node, params.poolId);
    }
    return {
      destroy() {
        // Clean up chart when canvas is removed
        const chart = poolCharts.get(params.poolId);
        if (chart) {
          chart.destroy();
          poolCharts.delete(params.poolId);
        }
        poolChartCanvases.delete(params.poolId);
      }
    };
  }

  // Get unique payment dates (from all payments, considering recurrence)
  // Always include today as a navigation point
  let paymentDates = $derived.by(() => {
    if (!debts?.occurrences) return [];
    const dates = new SvelteSet(debts.occurrences.map((o) => o.occurrence_date));
    dates.add(todayStr); // Add today to the set
    return [...dates].sort();
  });

  // Find previous date with payments (from current occurrences)
  let previousPaymentDate = $derived.by(() => {
    const currentDates = paymentDates.filter((d) => d < targetDate);
    return currentDates.length > 0 ? currentDates[currentDates.length - 1] : null;
  });

  // Calculate next payment date from allPayments (need to project forward)
  let nextPaymentDate = $derived.by(() => {
    if (allPayments.length === 0) return null;

    const targetDateObj = parseLocalDate(targetDate);
    let nextDates: string[] = [];

    for (const payment of allPayments) {
      const paymentDateStr = payment.payment_date.split('T')[0];

      if (!payment.is_recurring) {
        // Non-recurring: include if after target date
        if (paymentDateStr > targetDate) {
          nextDates.push(paymentDateStr);
        }
      } else {
        // Recurring: find next occurrence after target date
        const nextOccurrence = getNextRecurringDate(payment, targetDateObj);
        if (nextOccurrence) {
          nextDates.push(nextOccurrence);
        }
      }
    }

    if (nextDates.length === 0) return null;
    nextDates.sort();
    return nextDates[0];
  });

  // Helper to get next recurring date after a given date
  function getNextRecurringDate(payment: PaymentWithContributions, afterDate: Date): string | null {
    const startDateStr = payment.payment_date.split('T')[0];
    const startDate = parseLocalDate(startDateStr);

    // Check end date if set
    if (payment.recurrence_end_date) {
      const endDate = parseLocalDate(payment.recurrence_end_date);
      if (endDate <= afterDate) return null;
    }

    const recurrenceType = payment.recurrence_type || 'monthly';
    const interval = payment.recurrence_interval || 1;

    // Helper to get days in month
    function getDaysInMonth(year: number, month: number): number {
      return new SvelteDate(year, month + 1, 0).getDate();
    }

    // Handle enhanced weekly with weekdays pattern
    if (payment.recurrence_weekdays && recurrenceType === 'weekly') {
      try {
        const weekPatterns: number[][] = JSON.parse(payment.recurrence_weekdays);
        if (weekPatterns.length === 0) return null;

        // Start searching from afterDate
        const searchStart = new SvelteDate(afterDate);
        searchStart.setDate(searchStart.getDate() + 1); // Day after target

        // Calculate which week in the cycle we're in
        const startWeekday = startDate.getDay();
        const weekStart = new SvelteDate(startDate);
        weekStart.setDate(weekStart.getDate() - startWeekday);

        const msPerWeek = 7 * 24 * 60 * 60 * 1000;

        // Search forward from afterDate
        for (let daysAhead = 0; daysAhead < 365 * 2; daysAhead++) {
          const checkDate = new SvelteDate(searchStart);
          checkDate.setDate(checkDate.getDate() + daysAhead);

          if (checkDate < startDate) continue;

          // Check end date
          if (payment.recurrence_end_date) {
            const endDate = parseLocalDate(payment.recurrence_end_date);
            if (checkDate > endDate) return null;
          }

          // Calculate week number from start
          const checkWeekStart = new SvelteDate(checkDate);
          checkWeekStart.setDate(checkWeekStart.getDate() - checkDate.getDay());
          const weeksDiff = Math.floor(
            (checkWeekStart.getTime() - weekStart.getTime()) / msPerWeek
          );
          const cycleWeek =
            ((weeksDiff % weekPatterns.length) + weekPatterns.length) % weekPatterns.length;

          const weekdays = weekPatterns[cycleWeek];
          if (weekdays.includes(checkDate.getDay())) {
            return getLocalDateString(checkDate);
          }
        }
        return null;
      } catch {
        // Fall through to legacy logic
      }
    }

    // Handle enhanced monthly with monthdays pattern
    if (payment.recurrence_monthdays && recurrenceType === 'monthly') {
      try {
        const monthdays: number[] = JSON.parse(payment.recurrence_monthdays);
        if (monthdays.length === 0) return null;

        // Start from month of afterDate
        let year = afterDate.getFullYear();
        let month = afterDate.getMonth();

        for (let monthsAhead = 0; monthsAhead < 12 * 10; monthsAhead++) {
          const daysInMonth = getDaysInMonth(year, month);

          for (const day of monthdays.sort((a, b) => a - b)) {
            const actualDay = Math.min(day, daysInMonth);
            const checkDate = new SvelteDate(year, month, actualDay);

            if (checkDate <= afterDate) continue;
            if (checkDate < startDate) continue;

            // Check end date
            if (payment.recurrence_end_date) {
              const endDate = parseLocalDate(payment.recurrence_end_date);
              if (checkDate > endDate) return null;
            }

            return getLocalDateString(checkDate);
          }

          // Move to next month
          month++;
          if (month > 11) {
            month = 0;
            year++;
          }
        }
        return null;
      } catch {
        // Fall through to legacy logic
      }
    }

    // Handle enhanced yearly with months pattern
    if (payment.recurrence_months && recurrenceType === 'yearly') {
      try {
        const months: number[] = JSON.parse(payment.recurrence_months);
        if (months.length === 0) return null;

        const baseDay = startDate.getDate();
        let year = afterDate.getFullYear();

        for (let yearsAhead = 0; yearsAhead < 50; yearsAhead++) {
          for (const month of months.sort((a, b) => a - b)) {
            if (month < 1 || month > 12) continue;

            const jsMonth = month - 1; // Convert 1-indexed to 0-indexed
            const daysInMonth = getDaysInMonth(year, jsMonth);
            const actualDay = Math.min(baseDay, daysInMonth);
            const checkDate = new SvelteDate(year, jsMonth, actualDay);

            if (checkDate <= afterDate) continue;
            if (checkDate < startDate) continue;

            // Check end date
            if (payment.recurrence_end_date) {
              const endDate = parseLocalDate(payment.recurrence_end_date);
              if (checkDate > endDate) return null;
            }

            return getLocalDateString(checkDate);
          }

          year++;
        }
        return null;
      } catch {
        // Fall through to legacy logic
      }
    }

    // Legacy simple interval logic
    const timesPer = payment.recurrence_times_per;

    // Calculate effective interval in days
    let dayInterval: number;
    if (timesPer) {
      // X times per period
      switch (recurrenceType) {
        case 'weekly':
          dayInterval = Math.max(1, Math.floor(7 / timesPer));
          break;
        case 'monthly':
          dayInterval = Math.max(1, Math.floor(30 / timesPer));
          break;
        case 'yearly':
          dayInterval = Math.max(1, Math.floor(365 / timesPer));
          break;
        default:
          dayInterval = 1;
      }
    } else {
      // Every X periods
      switch (recurrenceType) {
        case 'daily':
          dayInterval = interval;
          break;
        case 'weekly':
          dayInterval = interval * 7;
          break;
        case 'monthly':
          dayInterval = interval * 30;
          break;
        case 'yearly':
          dayInterval = interval * 365;
          break;
        default:
          dayInterval = 30;
      }
    }

    // Find next occurrence
    let current = new SvelteDate(startDate);
    const maxIterations = 1000; // Safety limit
    let iterations = 0;

    while (current <= afterDate && iterations < maxIterations) {
      current.setDate(current.getDate() + dayInterval);
      iterations++;
    }

    if (iterations >= maxIterations) return null;

    // Check against end date
    if (payment.recurrence_end_date) {
      const endDate = parseLocalDate(payment.recurrence_end_date);
      if (current > endDate) return null;
    }

    return getLocalDateString(current);
  }

  async function loadDebts() {
    loading = true;
    errorKey = '';
    try {
      // In range mode, load debts up to endDate; otherwise use targetDate
      const dateToLoad = isRangeMode && endDate ? endDate : targetDate;
      debts = await getDebts(projectId, dateToLoad, includeDrafts);

      // In range mode, also load debts at the start date for settlement comparison
      if (isRangeMode) {
        startDebts = await getDebts(projectId, targetDate, includeDrafts);
      } else {
        startDebts = null;
      }

      // Also load all payments to know future dates
      allPayments = await getPayments(projectId);
    } catch (e) {
      errorKey = getErrorKey(e, 'overview.failedToLoad');
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (projectId && targetDate) {
      // Track endDate and includeDrafts to reload when they change
      const _endDate = endDate;
      const _includeDrafts = includeDrafts;
      void _endDate;
      void _includeDrafts;
      loadDebts();
    }
  });

  // Track if we've already auto-expanded on initial load
  let autoExpandedOnce = $state(false);

  // Reset auto-expand flag when loading new data for a different date
  $effect(() => {
    if (projectId && targetDate) {
      autoExpandedOnce = false;
    }
  });

  // Auto-expand newest year and month on first load (only once per date)
  $effect(() => {
    if (groupedOccurrences.size > 0 && !autoExpandedOnce) {
      const firstYearEntry = [...groupedOccurrences.entries()][0];
      if (firstYearEntry) {
        const [year, yearData] = firstYearEntry;
        const newYears = new SvelteSet([year]);
        expandedYears = newYears;

        // Also expand first month
        const firstMonthEntry = [...yearData.months.entries()][0];
        if (firstMonthEntry) {
          const [monthKey] = firstMonthEntry;
          const newMonths = new SvelteMap<string, SvelteSet<string>>();
          newMonths.set(year, new SvelteSet([monthKey]));
          expandedMonths = newMonths;
        }
        autoExpandedOnce = true;
      }
    }
  });

  function goToPreviousPayment() {
    if (previousPaymentDate) {
      targetDate = previousPaymentDate;
    }
  }

  function goToNextPayment() {
    if (nextPaymentDate) {
      targetDate = nextPaymentDate;
    }
  }

  function formatDate(dateStr: string): string {
    return formatDateWithWeekday(dateStr);
  }

  // Calculate days difference
  function getDaysDiff(dateStr: string): number {
    const date = parseLocalDate(dateStr);
    const today = new SvelteDate();
    today.setHours(0, 0, 0, 0);
    date.setHours(0, 0, 0, 0);
    return Math.round((date.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  }

  // Get relative date label
  function getRelativeDateLabel(dateStr: string): string {
    const days = getDaysDiff(dateStr);
    if (days === 0) return $_('overview.today');
    if (days === 1) return $_('overview.tomorrow');
    if (days === -1) return $_('overview.yesterday');
    if (days > 0 && days <= 7) return $_('overview.inDays', { values: { days } });
    if (days < 0 && days >= -7) return $_('overview.daysAgo', { values: { days: Math.abs(days) } });
    return '';
  }

  // Check if date is in the future
  function isFutureDate(dateStr: string): boolean {
    return getDaysDiff(dateStr) > 0;
  }

  // Format recurrence info for display
  function formatRecurrence(payment: PaymentWithContributions): string {
    if (!payment.is_recurring) return '';
    const type = payment.recurrence_type || 'monthly';
    const interval = payment.recurrence_interval || 1;
    const timesPer = payment.recurrence_times_per;

    // Get translated recurrence type (adjective form for single interval)
    const typeKey = `transactions.recurrence.${type}`;
    const translatedType = $_(typeKey);

    if (timesPer) {
      return `${timesPer}×/${translatedType}`;
    } else if (interval === 1) {
      return translatedType;
    } else {
      // Use proper noun form with correct gender agreement
      const everyKey =
        type === 'daily'
          ? 'transactions.recurrence.everyNDays'
          : type === 'weekly'
            ? 'transactions.recurrence.everyNWeeks'
            : type === 'monthly'
              ? 'transactions.recurrence.everyNMonths'
              : 'transactions.recurrence.everyNYears';
      return $_(everyKey, { values: { n: interval } });
    }
  }

  let todayStr = $derived(getLocalDateString());

  // End date navigation - find previous/next payment dates relative to endDate
  let previousEndPaymentDate = $derived.by(() => {
    if (!endDate || !paymentDates.length) return null;
    // Find the payment date just before endDate, but after targetDate
    const currentEndDate = endDate; // Store for TypeScript narrowing
    const validDates = paymentDates.filter((d) => d < currentEndDate && d > targetDate);
    return validDates.length > 0 ? validDates[validDates.length - 1] : null;
  });

  // For nextEndPaymentDate, we need to project forward like nextPaymentDate does
  let nextEndPaymentDate = $derived.by(() => {
    if (!endDate || allPayments.length === 0) return null;

    const endDateObj = parseLocalDate(endDate);
    let nextDates: string[] = [];

    for (const payment of allPayments) {
      const paymentDateStr = payment.payment_date.split('T')[0];

      if (!payment.is_recurring) {
        // Non-recurring: include if after end date
        if (paymentDateStr > endDate) {
          nextDates.push(paymentDateStr);
        }
      } else {
        // Recurring: find next occurrence after endDate
        const nextOccurrence = getNextRecurringDate(payment, endDateObj);
        if (nextOccurrence) {
          nextDates.push(nextOccurrence);
        }
      }
    }

    if (nextDates.length === 0) return null;
    nextDates.sort();
    return nextDates[0];
  });

  function goToPreviousEndPayment() {
    if (previousEndPaymentDate) {
      endDate = previousEndPaymentDate;
      chosenHorizon = 'custom';
    }
  }

  function goToNextEndPayment() {
    if (nextEndPaymentDate) {
      endDate = nextEndPaymentDate;
      chosenHorizon = 'custom';
    }
  }

  // Map payment_id to full payment details for showing contributions
  let paymentMap = $derived.by(() => {
    const map = new SvelteMap<number, PaymentWithContributions>();
    for (const payment of allPayments) {
      map.set(payment.id, payment);
    }
    return map;
  });

  // Track expanded years and months
  let expandedYears = new SvelteSet<string>();
  let expandedMonths = new SvelteMap<string, SvelteSet<string>>();

  // Helper to get year from date string (YYYY-MM-DD format)
  function getYear(dateStr: string): string {
    return dateStr.substring(0, 4);
  }

  // Helper to get month key from date string (YYYY-MM format)
  function getMonthKey(dateStr: string): string {
    return dateStr.substring(0, 7);
  }

  // Helper to format month display (e.g., "December 2025")
  function formatMonthYear(dateStr: string): string {
    return formatMonthYearI18n(dateStr);
  }

  // Group occurrences by year/month/date, filtered by focus participant
  let groupedOccurrences = $derived.by(() => {
    if (!debts?.occurrences) return new SvelteMap<string, OccurrencesByYearData>();

    // First filter occurrences to only those involving the focused participant
    let occurrencesToShow = debts.occurrences;
    if (focusParticipantId !== null) {
      occurrencesToShow = debts.occurrences.filter((occ) =>
        paymentInvolvesFocus(occ.payment_id, occ.payer_id)
      );
    }

    // Then reverse for newest first
    const reversed = [...occurrencesToShow].reverse();

    // Group by year > month > date
    const yearMap = new SvelteMap<string, OccurrencesByYearData>();

    for (const occ of reversed) {
      const year = getYear(occ.occurrence_date);
      const monthKey = getMonthKey(occ.occurrence_date);
      const dateStr = occ.occurrence_date;

      if (!yearMap.has(year)) {
        yearMap.set(year, {
          year,
          expanded: expandedYears.has(year),
          months: new SvelteMap()
        });
      }

      const yearData = yearMap.get(year)!;
      if (!yearData.months.has(monthKey)) {
        yearData.months.set(monthKey, {
          monthKey,
          month: formatMonthYear(dateStr),
          expanded: expandedMonths.get(year)?.has(monthKey) ?? false,
          totalAmount: 0,
          dates: new SvelteMap(),
          participantTotals: new Map<string, number>()
        });
      }

      const monthData = yearData.months.get(monthKey)!;
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

      // Track per-participant contributions for this month
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

  // Toggle year expanded state
  function toggleYear(year: string) {
    if (expandedYears.has(year)) {
      expandedYears.delete(year);
    } else {
      expandedYears.add(year);
    }
  }

  // Toggle month expanded state
  function toggleMonth(year: string, monthKey: string) {
    if (!expandedMonths.has(year)) {
      expandedMonths.set(year, new SvelteSet());
    }
    const monthSet = expandedMonths.get(year)!;
    if (monthSet.has(monthKey)) {
      monthSet.delete(monthKey);
    } else {
      monthSet.add(monthKey);
    }
  }

  // Get total for a year
  function getYearTotal(yearData: OccurrencesByYearData): number {
    let total = 0;
    for (const monthData of yearData.months.values()) {
      total += monthData.totalAmount;
    }
    return total;
  }

  // Count total transactions in a month
  function getTransactionCount(monthData: OccurrencesByMonthData): number {
    let count = 0;
    for (const dateData of monthData.dates.values()) {
      count += dateData.occurrences.length;
    }
    return count;
  }

  // Count total transactions in a year
  function getYearTransactionCount(yearData: OccurrencesByYearData): number {
    let count = 0;
    for (const monthData of yearData.months.values()) {
      count += getTransactionCount(monthData);
    }
    return count;
  }

  // Get per-participant totals for a year
  function getYearParticipantTotals(yearData: OccurrencesByYearData): Map<string, number> {
    const totals = new SvelteMap<string, number>();
    for (const monthData of yearData.months.values()) {
      for (const [participantName, amount] of monthData.participantTotals.entries()) {
        const current = totals.get(participantName) || 0;
        totals.set(participantName, current + amount);
      }
    }
    return totals;
  }
</script>

<h2>{$_('overview.title')}</h2>

<!-- Date selector -->
<div class="date-selector card" class:range-mode={isRangeMode}>
  <!-- Start Date Row -->
  <div class="date-row">
    <span class="date-row-label">{$_('overview.from')}</span>
    <div class="date-row-center">
      <div class="date-nav-row">
        <button
          class="nav-btn"
          onclick={goToPreviousPayment}
          disabled={!previousPaymentDate}
          title={previousPaymentDate
            ? $_('overview.goToDate', { values: { date: formatDate(previousPaymentDate) } })
            : $_('overview.noPreviousPayment')}><ChevronLeft size={20} /></button
        >

        <div class="date-display">
          <input type="date" bind:value={targetDate} class="date-input" />
        </div>

        <button
          class="nav-btn"
          onclick={goToNextPayment}
          disabled={!nextPaymentDate}
          title={nextPaymentDate
            ? $_('overview.goToDate', { values: { date: formatDate(nextPaymentDate) } })
            : $_('overview.noNextPayment')}><ChevronRight size={20} /></button
        >
      </div>
    </div>
  </div>

  <!-- Horizon Selector -->
  <div class="horizon-selector">
    <span class="horizon-label">{$_('overview.horizon')}</span>
    <div class="horizon-buttons">
      <button
        class="horizon-btn"
        class:active={selectedHorizon === 'none'}
        onclick={() => setHorizon('none')}
      >
        {$_('overview.horizonNone')}
      </button>
      <button
        class="horizon-btn"
        class:active={selectedHorizon === 'thisMonth'}
        onclick={() => setHorizon('thisMonth')}
      >
        {$_('overview.thisMonth')}
      </button>
      <button
        class="horizon-btn"
        class:active={selectedHorizon === 'nextMonth'}
        onclick={() => setHorizon('nextMonth')}
      >
        {$_('overview.nextMonth')}
      </button>
      <button
        class="horizon-btn"
        class:active={selectedHorizon === '3months'}
        onclick={() => setHorizon('3months')}
      >
        {$_('overview.threeMonths')}
      </button>
      <button
        class="horizon-btn"
        class:active={selectedHorizon === '6months'}
        onclick={() => setHorizon('6months')}
      >
        {$_('overview.sixMonths')}
      </button>
      <button
        class="horizon-btn"
        class:active={selectedHorizon === '12months'}
        onclick={() => setHorizon('12months')}
      >
        {$_('overview.twelveMonths')}
      </button>
    </div>
  </div>

  <!-- End Date Row (only shown when horizon is set) -->
  {#if isRangeMode}
    <div class="date-row end-date-row">
      <span class="date-row-label">{$_('overview.to')}</span>
      <div class="date-nav-row">
        <button
          class="nav-btn"
          onclick={goToPreviousEndPayment}
          disabled={!previousEndPaymentDate}
          title={previousEndPaymentDate
            ? $_('overview.goToDate', { values: { date: formatDate(previousEndPaymentDate) } })
            : $_('overview.noPreviousPayment')}><ChevronLeft size={20} /></button
        >

        <div class="date-display">
          <input type="date" bind:value={endDate} min={targetDate} class="date-input" />
        </div>

        <button
          class="nav-btn"
          onclick={goToNextEndPayment}
          disabled={!nextEndPaymentDate}
          title={nextEndPaymentDate
            ? $_('overview.goToDate', { values: { date: formatDate(nextEndPaymentDate) } })
            : $_('overview.noNextPayment')}><ChevronRight size={20} /></button
        >
      </div>
    </div>
  {/if}

  <!-- Include drafts toggle -->
  <div class="include-drafts-toggle">
    <label class="checkbox-label">
      <input type="checkbox" bind:checked={includeDrafts} />
      {$_('overview.includeDrafts')}
    </label>
  </div>

  <!-- Focus mode selector -->
  {#if debts}
    <div class="focus-selector">
      <label for="focus-participant">{$_('overview.focusOn')}</label>
      <select id="focus-participant" bind:value={focusParticipantId}>
        <option value={null}>{$_('overview.allParticipants')}</option>
        {#each nonPoolBalances as b (b.participant_id)}
          <option value={b.participant_id}>{b.participant_name}</option>
        {/each}
      </select>
      {#if focusParticipantId !== null}
        <button class="clear-focus" onclick={() => (focusParticipantId = null)}
          >{$_('common.clear')}</button
        >
      {/if}
    </div>
  {/if}
</div>
{#if errorKey}
  <div class="error">{$_(errorKey)}</div>
{/if}

{#if loading}
  <p>{$_('common.loading')}</p>
{:else if debts}
  <!-- Pool Ownerships (shown for each pool) -->
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
        {@const hasWarning = isRangeMode
          ? stats.isBelowExpected
          : hasExpectedMin && stats.isBelowExpected}
        <div class="pool-total-banner" class:has-warning={hasWarning}>
          {#if isRangeMode && stats.isBelowExpected}
            <div class="pool-warning-badge">
              <TriangleAlert size={18} class="warning-icon" color="currentColor" />
              <span class="warning-text">
                {hasExpectedMin ? $_('overview.belowExpected') : $_('overview.poolNegativeWarning')}
              </span>
            </div>
          {:else if !isRangeMode && hasExpectedMin && stats.isBelowExpected}
            <div class="pool-warning-badge">
              <TriangleAlert size={18} class="warning-icon" color="currentColor" />
              <span class="warning-text">{$_('overview.belowExpected')}</span>
            </div>
          {/if}
          <div class="pool-total-stats">
            <div class="pool-stat">
              {#if isRangeMode}
                <span class="stat-label">{$_('overview.balanceStart')}</span>
              {:else}
                <span class="stat-label">{$_('overview.balance')}</span>
              {/if}
              <span
                class="stat-value"
                class:positive={stats.currentTotal > 0}
                class:negative={stats.currentTotal < 0}
              >
                {stats.currentTotal >= 0 ? '+' : ''}{formatCurrency(stats.currentTotal)}
              </span>
            </div>
            {#if isRangeMode}
              <div class="pool-stat">
                <span class="stat-label">{$_('overview.balanceEnd')}</span>
                <span
                  class="stat-value"
                  class:positive={stats.projectedTotal > 0}
                  class:negative={stats.projectedTotal < 0}
                >
                  {stats.projectedTotal >= 0 ? '+' : ''}{formatCurrency(stats.projectedTotal)}
                </span>
              </div>
              <div class="pool-stat">
                <span class="stat-label">{$_('overview.balanceLowest')}</span>
                <span
                  class="stat-value"
                  class:positive={stats.minTotal > 0}
                  class:negative={stats.minTotal < 0}
                >
                  {stats.minTotal >= 0 ? '+' : ''}{formatCurrency(stats.minTotal)}
                </span>
              </div>
            {/if}
            {#if hasExpectedMin}
              <div class="pool-stat">
                <span class="stat-label"
                  >{isRangeMode
                    ? $_('overview.expectedStart')
                    : $_('overview.expectedMinimum')}</span
                >
                <span class="stat-value">
                  {formatCurrency(stats.currentExpectedMinimum)}
                </span>
              </div>
              {#if isRangeMode}
                <div class="pool-stat">
                  <span class="stat-label">{$_('overview.expectedEnd')}</span>
                  <span class="stat-value">
                    {formatCurrency(stats.projectedExpectedMinimum)}
                  </span>
                </div>
                <div class="pool-stat">
                  <span class="stat-label">{$_('overview.expectedHighest')}</span>
                  <span class="stat-value">
                    {formatCurrency(stats.maxExpectedMinimum)}
                  </span>
                </div>
              {/if}
            {/if}
          </div>
        </div>
      {/if}

      {#if poolOwnership.entries.length === 0}
        <p class="no-ownership">{$_('overview.noPoolActivity')}</p>
      {:else if isRangeMode && chartReady}
        <!-- Graph view in range mode -->
        <div class="chart-container">
          <canvas use:bindPoolCanvas={{ poolId: poolOwnership.pool_id }}></canvas>
        </div>
      {:else}
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
              <tr
                class="balance-row"
                class:expanded={isExpanded}
                class:focused={focusParticipantId === entry.participant_id}
                onclick={() => togglePoolEntry(entry.participant_id)}
              >
                <td class="expand-cell">
                  <span class="expand-icon">{isExpanded ? '▼' : '▶'}</span>
                </td>
                <td>{entry.participant_name}</td>
                <td class:positive={entry.ownership > 0} class:negative={entry.ownership < 0}>
                  {entry.ownership >= 0 ? '+' : ''}{formatCurrency(entry.ownership)}
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
                                          <ul class="breakdown-list">
                                            {#each monthData.items as item (item.payment_id)}
                                              <li class="breakdown-item">
                                                <span class="breakdown-desc"
                                                  >{item.description}</span
                                                >
                                                <span class="breakdown-date"
                                                  >{item.occurrence_date}</span
                                                >
                                                <span class="breakdown-amount"
                                                  >{formatCurrency(item.amount)}</span
                                                >
                                              </li>
                                            {/each}
                                          </ul>
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
                                          <ul class="breakdown-list">
                                            {#each monthData.items as item (item.payment_id)}
                                              <li class="breakdown-item">
                                                <span class="breakdown-desc"
                                                  >{item.description}</span
                                                >
                                                <span class="breakdown-date"
                                                  >{item.occurrence_date}</span
                                                >
                                                <span class="breakdown-amount"
                                                  >{formatCurrency(item.amount)}</span
                                                >
                                              </li>
                                            {/each}
                                          </ul>
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

  <section class="card">
    <h3>{$_('overview.balances')}</h3>
    {#if isRangeMode && chartReady}
      <!-- Graph view in range mode -->
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
              class:focused={focusParticipantId === b.participant_id}
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
                                                  <ul class="breakdown-list">
                                                    {#each monthData.items as item (item.payment_id)}
                                                      <li class="breakdown-item">
                                                        <span class="breakdown-desc"
                                                          >{item.description}</span
                                                        >
                                                        <span class="breakdown-date"
                                                          >{item.occurrence_date}</span
                                                        >
                                                        <span class="breakdown-amount"
                                                          >{formatCurrency(item.amount)}</span
                                                        >
                                                      </li>
                                                    {/each}
                                                  </ul>
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
                                                  <ul class="breakdown-list">
                                                    {#each monthData.items as item (item.payment_id)}
                                                      <li class="breakdown-item">
                                                        <span class="breakdown-desc"
                                                          >{item.description}</span
                                                        >
                                                        <span class="breakdown-date"
                                                          >{item.occurrence_date}</span
                                                        >
                                                        <span class="breakdown-amount"
                                                          >{formatCurrency(item.amount)}</span
                                                        >
                                                      </li>
                                                    {/each}
                                                  </ul>
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

  <section class="card">
    <div class="settlements-header">
      <h3>{$_('overview.settlements')}</h3>
      <label class="settlement-mode-toggle">
        <input type="checkbox" bind:checked={useDirectSettlements} />
        {$_('overview.directOnlyMode')}
      </label>
    </div>

    {#if isRangeMode}
      <!-- Range mode: show start and end settlements side by side -->
      <div class="settlements-range">
        <div class="settlement-column">
          <h4 class="settlement-date-label">{$_('overview.atStart')} ({formatDate(targetDate)})</h4>
          {#if filteredStartSettlements.length === 0}
            <p class="all-settled">
              {focusParticipantId !== null
                ? $_('overview.noSettlementsForParticipant')
                : $_('overview.allSettled')}
            </p>
          {:else}
            <ul class="settlements-list">
              {#each filteredStartSettlements as s (`start-${s.from_participant_id}-${s.to_participant_id}`)}
                <li class:focused={focusParticipantId !== null}>
                  <span class="from" class:highlight={s.from_participant_id === focusParticipantId}
                    >{s.from_participant_name}</span
                  >
                  <span class="arrow">&rarr;</span>
                  <span class="to" class:highlight={s.to_participant_id === focusParticipantId}
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
            {$_('overview.atEnd')} ({formatDate(endDate ?? targetDate)})
          </h4>
          {#if filteredSettlements.length === 0}
            <p class="all-settled">
              {focusParticipantId !== null
                ? $_('overview.noSettlementsForParticipant')
                : $_('overview.allSettled')}
            </p>
          {:else}
            <ul class="settlements-list">
              {#each filteredSettlements as s (`end-${s.from_participant_id}-${s.to_participant_id}`)}
                <li class:focused={focusParticipantId !== null}>
                  <span class="from" class:highlight={s.from_participant_id === focusParticipantId}
                    >{s.from_participant_name}</span
                  >
                  <span class="arrow">&rarr;</span>
                  <span class="to" class:highlight={s.to_participant_id === focusParticipantId}
                    >{s.to_participant_name}</span
                  >
                  <span class="amount">{formatCurrency(s.amount)}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    {:else}
      <!-- Single date mode: show settlements as before -->
      {#if filteredSettlements.length === 0}
        <p class="all-settled">
          {focusParticipantId !== null
            ? $_('overview.noSettlementsForParticipant')
            : $_('overview.allSettled')}
        </p>
      {:else}
        <ul class="settlements-list">
          {#each filteredSettlements as s (`${s.from_participant_id}-${s.to_participant_id}`)}
            <li class:focused={focusParticipantId !== null}>
              <span class="from" class:highlight={s.from_participant_id === focusParticipantId}
                >{s.from_participant_name}</span
              >
              <span class="arrow">&rarr;</span>
              <span class="to" class:highlight={s.to_participant_id === focusParticipantId}
                >{s.to_participant_name}</span
              >
              <span class="amount">{formatCurrency(s.amount)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </section>

  <!-- All payment occurrences (expandable) -->
  {#if debts.occurrences && debts.occurrences.length > 0}
    {@const totalCount =
      focusParticipantId === null
        ? debts.occurrences.length
        : debts.occurrences.filter((occ) => paymentInvolvesFocus(occ.payment_id, occ.payer_id))
            .length}
    <section class="card occurrences-card">
      <button class="expand-header" onclick={() => (showOccurrences = !showOccurrences)}>
        <span class="expand-icon">{showOccurrences ? '▼' : '▶'}</span>
        <h3>{$_('overview.transactionsIncluded')} ({totalCount})</h3>
      </button>

      {#if showOccurrences}
        <div class="hierarchy-container">
          {#each [...groupedOccurrences.entries()] as [year, yearData] (year)}
            <!-- Year group -->
            <div class="year-group">
              <button class="year-header" onclick={() => toggleYear(year)}>
                {#if yearData.expanded}
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

              {#if yearData.expanded}
                {#each [...yearData.months.entries()] as [monthKey, monthData] (monthKey)}
                  <!-- Month group -->
                  <div class="month-group">
                    <button class="month-header" onclick={() => toggleMonth(year, monthKey)}>
                      {#if monthData.expanded}
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

                    {#if monthData.expanded}
                      {#each [...monthData.dates.entries()] as [_dateKey, dateData] (dateData.date)}
                        <!-- Date group -->
                        <div class="date-group">
                          <div class="date-label">
                            {formatDate(dateData.date)}
                            <span class="date-total">{formatCurrency(dateData.totalAmount)}</span>
                          </div>

                          <!-- Individual transactions -->
                          <div class="transactions">
                            {#each dateData.occurrences as occ (occ.payment_id)}
                              {@const payment = paymentMap.get(occ.payment_id)}
                              {@const occRelative = getRelativeDateLabel(occ.occurrence_date)}
                              {@const receiver = occ.receiver_account_id
                                ? $participants.find((pr) => pr.id === occ.receiver_account_id)
                                : null}
                              <div class="transaction">
                                <div class="trans-header">
                                  <span class="trans-desc">
                                    {occ.description}
                                    <span class="trans-badges">
                                      {#if occ.affects_balance === false}
                                        <span class="badge rule">{$_('transactions.typeRule')}</span
                                        >
                                      {:else}
                                        {#if occ.affects_payer_expectation}
                                          <span class="badge approved"
                                            >{$_('transactions.statusApproved')}</span
                                          >
                                        {/if}
                                        {#if occ.affects_receiver_expectation}
                                          <span class="badge earmarked"
                                            >{$_('transactions.statusEarmarked')}</span
                                          >
                                        {/if}
                                      {/if}
                                      {#if !occ.is_final}
                                        <span class="badge draft"
                                          >{$_('transactions.statusDraft')}</span
                                        >
                                      {/if}
                                    </span>
                                  </span>
                                  <span class="trans-amount">{formatCurrency(occ.amount)}</span>
                                </div>
                                <div class="trans-meta">
                                  {#if occ.payer_id === null && occ.receiver_account_id !== null}
                                    <!-- External inflow -->
                                    <span class="inflow-badge"
                                      >{$_('transactions.externalInflow')}</span
                                    >
                                    {$_('transactions.externalIndicator')} → {receiver?.name ??
                                      $_('common.unknown')}
                                  {:else if occ.receiver_account_id !== null && payment}
                                    <!-- Internal transfer -->
                                    <span class="transfer-badge">{$_('transactions.transfer')}</span
                                    >
                                    {payment.payer_name ?? $_('common.unknown')} → {receiver?.name ??
                                      $_('common.unknown')}
                                  {:else if payment}
                                    <!-- External expense -->
                                    {$_('overview.paidBy')}
                                    {payment.payer_name ?? $_('common.unknown')}
                                  {/if}
                                  {#if payment}
                                    {#if payment.is_recurring && payment.recurrence_end_date}
                                      {$_('overview.from')}
                                      {formatDate(payment.payment_date)}
                                      {$_('overview.to')}
                                      {formatDate(payment.recurrence_end_date)}
                                    {:else}
                                      {isFutureDate(occ.occurrence_date)
                                        ? $_('overview.from')
                                        : $_('overview.on')}
                                      {formatDate(occ.occurrence_date)}
                                    {/if}
                                    {#if payment.is_recurring}
                                      <span class="recurrence-badge"
                                        >{formatRecurrence(payment)}</span
                                      >
                                    {/if}
                                  {:else}
                                    {$_('overview.on')} {formatDate(occ.occurrence_date)}
                                  {/if}
                                  {#if occRelative}
                                    <span class="trans-relative">({occRelative})</span>
                                  {/if}
                                </div>
                                {#if payment && payment.contributions && payment.contributions.length > 0}
                                  <div class="trans-splits">
                                    {#each payment.contributions as c (c.participant_id)}
                                      <span class="chip">
                                        {c.participant_name}: {formatCurrency(c.amount)}
                                      </span>
                                    {/each}
                                  </div>
                                {/if}
                              </div>
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

  /* Chart container */
  .chart-container {
    position: relative;
    height: 350px;
    width: 100%;
    margin: 1rem 0;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
  }

  /* Pool Total Banner */
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

  h3 {
    margin-top: 0;
    margin-bottom: 1rem;
    color: var(--accent, #7b61ff);
  }

  /* Date selector styles */
  .date-selector {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
  }

  .date-nav-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    width: 100%;
  }

  .nav-btn {
    background: var(--accent, #7b61ff);
    color: white;
    border: none;
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    cursor: pointer;
    transition:
      background 0.2s,
      opacity 0.2s;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .nav-btn :global(svg) {
    transform: scaleY(1.3);
  }

  .nav-btn:hover:not(:disabled) {
    background: #6b51ef;
  }

  .nav-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .date-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0 0.5rem;
    min-width: 0;
  }

  .date-input {
    border: 1px solid #ddd;
    border-radius: 8px;
    padding: 0.5rem;
    font-size: 1rem;
    background: white;
    cursor: pointer;
  }

  .date-input:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .date-label {
    font-size: 0.85rem;
    color: #666;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: center;
  }

  /* Date row layout - table-like grid with 3 columns */
  .date-row {
    display: grid;
    grid-template-columns: 5rem 1fr 6.5rem;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
  }

  .date-row-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: #666;
    text-align: left;
  }

  .date-row-center {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .end-date-row {
    padding-top: 0.5rem;
    border-top: 1px solid #eee;
  }

  /* Horizon selector - uses same grid layout */
  .horizon-selector {
    display: grid;
    grid-template-columns: 5rem 1fr 6.5rem;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.5rem 0;
    border-top: 1px solid #eee;
  }

  .horizon-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: #666;
    text-align: left;
  }

  .horizon-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: center;
  }

  .horizon-btn {
    padding: 0.35rem 0.75rem;
    border: 1px solid #ddd;
    border-radius: 16px;
    background: white;
    color: #666;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .horizon-btn:hover {
    border-color: var(--accent, #7b61ff);
    color: var(--accent, #7b61ff);
  }

  .horizon-btn.active {
    background: var(--accent, #7b61ff);
    border-color: var(--accent, #7b61ff);
    color: white;
  }

  /* Settlements range mode */
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

    .date-selector {
      overflow-x: hidden;
      padding: 1rem;
    }

    .date-row {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      align-items: center;
      width: 100%;
    }

    .date-row-label {
      text-align: center;
      margin-bottom: 0;
    }

    .date-row-center {
      width: 100%;
    }

    .date-nav-row {
      width: 100%;
      justify-content: center;
      flex-wrap: wrap;
      gap: 0.5rem;
    }

    .date-display {
      max-width: 100%;
    }

    .date-input {
      max-width: 150px;
    }

    .horizon-selector {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      align-items: center;
      width: 100%;
    }

    .horizon-label {
      text-align: center;
    }

    .horizon-buttons {
      flex-wrap: wrap;
      justify-content: center;
    }

    .horizon-btn {
      font-size: 0.75rem;
      padding: 0.3rem 0.6rem;
    }

    .focus-selector {
      flex-direction: column;
      align-items: stretch;
      gap: 0.5rem;
    }

    .focus-selector label {
      text-align: center;
    }

    .focus-selector select {
      width: 100%;
    }
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
    list-style: none;
    padding: 0.5rem 0 0 1rem;
    margin: 0.5rem 0 0 1rem;
    font-size: 0.8rem;
    border-left: 2px solid #ddd;
  }

  .breakdown-item {
    display: flex;
    gap: 0.5rem;
    padding: 0.25rem 0;
    flex-wrap: wrap;
  }

  .breakdown-desc {
    flex: 1;
    min-width: 150px;
    color: #555;
  }

  .breakdown-date {
    color: #888;
    font-size: 0.75rem;
  }

  .breakdown-amount {
    color: var(--accent, #7b61ff);
    font-weight: 500;
  }

  .no-ownership {
    color: #888;
    font-style: italic;
  }

  /* Include drafts toggle */
  .include-drafts-toggle {
    display: flex;
    align-items: center;
    margin-bottom: 1rem;
  }

  .include-drafts-toggle .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    color: #666;
    cursor: pointer;
  }

  .include-drafts-toggle input[type='checkbox'] {
    width: 16px;
    height: 16px;
    cursor: pointer;
  }

  /* Focus mode styles */
  .focus-selector {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 12px;
    margin-bottom: 1.5rem;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.05);
  }

  .focus-selector label {
    font-weight: 600;
    color: #666;
  }

  .focus-selector select {
    padding: 0.5rem 1rem;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 0.95rem;
    background: white;
    min-width: 180px;
  }

  .focus-selector select:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .clear-focus {
    padding: 0.5rem 1rem;
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 0.85rem;
    cursor: pointer;
    color: #666;
  }

  .clear-focus:hover {
    background: #eee;
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

  /* Occurrences styles */
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
  }

  .expand-icon {
    color: var(--accent, #7b61ff);
    font-size: 0.85rem;
    display: inline-block;
    width: 1rem;
  }

  .hierarchy-container {
    padding: 0 1.5rem 1.5rem;
  }

  /* Year level */
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

  /* Month level */
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

  /* Date level */
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

  /* Transactions */
  .transactions {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .transaction {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.75rem;
    background: #fafafa;
    border-radius: 6px;
    border-left: 3px solid var(--accent, #7b61ff);
  }

  .trans-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }

  .trans-desc {
    flex: 1;
    font-weight: 500;
    font-size: 0.95rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .trans-amount {
    font-weight: 600;
    color: var(--accent, #7b61ff);
    min-width: fit-content;
    font-size: 0.95rem;
  }

  .trans-meta {
    font-size: 0.85rem;
    color: #777;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .trans-relative {
    color: #999;
    font-style: italic;
    font-size: 0.8rem;
  }

  .trans-splits {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    padding-top: 0.25rem;
  }

  .recurrence-badge {
    padding: 0.15rem 0.5rem;
    background: var(--accent, #7b61ff);
    color: white;
    border-radius: 12px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .trans-badges {
    display: inline-flex;
    gap: 0.25rem;
    margin-left: 0.25rem;
  }

  .badge.draft {
    background: #f0ad4e;
    color: white;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge.approved {
    background: #28a745;
    color: white;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge.rule {
    background: #7b61ff;
    color: white;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge.earmarked {
    background: #17a2b8;
    color: white;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .transfer-badge {
    background: linear-gradient(135deg, #4caf50 0%, #2e7d32 100%);
    color: white;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .inflow-badge {
    background: linear-gradient(135deg, #1976d2 0%, #0d47a1 100%);
    color: white;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .chip {
    padding: 0.25rem 0.5rem;
    background: #e8e4ff;
    border-radius: 12px;
    font-size: 0.8rem;
    color: #555;
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

  .expand-icon {
    color: var(--accent, #7b61ff);
    font-size: 0.8rem;
    flex: 0 0 auto;
  }
</style>
