<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import {
		getDebts,
		getPayments,
		type DebtSummary,
		type PaymentWithContributions,
		type PairwiseBalance,
		type PaymentOccurrence
	} from '$lib/api';
	import { _ } from '$lib/i18n';
	import { formatDateWithWeekday, formatMonthYear as formatMonthYearI18n } from '$lib/format/date';
	import { formatCurrency } from '$lib/format/currency';
	import { SvelteSet, SvelteMap, SvelteDate } from 'svelte/reactivity';
	import type { PairwisePaymentBreakdown } from '$lib/api';
	import { Chart, registerables } from 'chart.js';
	import 'chartjs-adapter-date-fns';

	// Chart.js annotation plugin - loaded dynamically for SSR safety
	let annotationPlugin: typeof import('chartjs-plugin-annotation').default | null = null;
	let chartReady = $state(false);

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type LineChart = Chart<'line', any, unknown>;

	// Chart instances for cleanup (not reactive - just for cleanup/destroy)
	let balanceChart: LineChart | null = null;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	let poolCharts = new Map<number, LineChart>();

	// Chart canvas references (not reactive - just for DOM binding)
	let balanceChartCanvas: HTMLCanvasElement | null = $state(null);
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
	let allPayments: PaymentWithContributions[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let showOccurrences = $state(false);

	// Track which balance rows are expanded to show pairwise details
	// svelte-ignore non_reactive_update (SvelteSet is already reactive)
	let expandedBalanceRows = new SvelteSet<number>();

	// Track which pool ownership entries are expanded
	// svelte-ignore non_reactive_update (SvelteSet is already reactive)
	let expandedPoolEntries = new SvelteSet<number>();
	// svelte-ignore non_reactive_update (SvelteMap is already reactive)
	let expandedPoolYears = new SvelteMap<number, SvelteSet<string>>();
	// svelte-ignore non_reactive_update (SvelteMap is already reactive)
	let expandedPoolMonths = new SvelteMap<number, SvelteMap<string, SvelteSet<string>>>();

	// Focus mode: filter to show only one participant's perspective
	let focusParticipantId = $state<number | null>(null);

	// Settlement mode: false = minimal (default), true = direct-only
	let useDirectSettlements = $state(false);

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
	// svelte-ignore non_reactive_update (SvelteSet/SvelteMap are already reactive)
	let expandedPairwise = new SvelteSet<string>();
	// svelte-ignore non_reactive_update (SvelteMap is already reactive)
	let expandedPairwiseYears = new SvelteMap<string, SvelteSet<string>>();
	// svelte-ignore non_reactive_update (SvelteMap is already reactive)
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

	// Get active settlements based on mode
	let activeSettlements = $derived.by(() => {
		if (!debts) return [];
		return useDirectSettlements ? debts.direct_settlements : debts.settlements;
	});

	// Get filtered settlements based on focus
	let filteredSettlements = $derived.by(() => {
		if (!debts) return [];
		if (focusParticipantId === null) return activeSettlements;
		return activeSettlements.filter((s) => settlementInvolvesFocus(s));
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
						// Credit the payer
						if (occ.payer_id !== null && !poolIds.has(occ.payer_id)) {
							const current = runningBalance.get(occ.payer_id) ?? 0;
							runningBalance.set(occ.payer_id, current + occ.amount);
						}

						// Debit contributors based on their weights
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
						// If focused participant is the payer
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

					if (payment) {
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
					}

					occIdx++;
				}

				poolTimeSeries.set(date, new Map(runningOwnership));
			}

			result.set(pool.pool_id, poolTimeSeries);
		}

		return result;
	});

	// Get today annotation for charts
	function getTodayAnnotation() {
		if (!annotationPlugin) return {};
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
			tension: number;
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
					tension: 0.3,
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
					tension: 0.3,
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

		const datasets: {
			label: string;
			data: { x: string; y: number }[];
			borderColor: string;
			backgroundColor: string;
			tension: number;
			fill: boolean;
			pointRadius: number;
			pointHoverRadius: number;
		}[] = [];

		// Get participant IDs to show (filter by focus if applicable)
		const entriesToShow =
			focusParticipantId !== null
				? pool.entries.filter((e) => e.participant_id === focusParticipantId)
				: pool.entries;

		entriesToShow.forEach((entry, idx) => {
			datasets.push({
				label: entry.participant_name,
				data: rangeDates.map((date) => ({
					x: date,
					y: poolData.get(date)?.get(entry.participant_id) ?? 0
				})),
				borderColor: CHART_COLORS[idx % CHART_COLORS.length],
				backgroundColor: CHART_COLORS[idx % CHART_COLORS.length] + '20',
				tension: 0.3,
				fill: false,
				pointRadius: 3,
				pointHoverRadius: 5
			});
		});

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
		error = '';
		try {
			// In range mode, load debts up to endDate; otherwise use targetDate
			const dateToLoad = isRangeMode && endDate ? endDate : targetDate;
			debts = await getDebts(projectId, dateToLoad);
			// Also load all payments to know future dates
			allPayments = await getPayments(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load debts';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (projectId && targetDate) {
			// Track endDate to reload when range changes
			const _endDate = endDate;
			void _endDate;
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

	function goToToday() {
		targetDate = getLocalDateString();
		endDate = null; // Clear range mode when going to today
	}

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
		const typeKey = `payments.recurrence.${type}`;
		const translatedType = $_(typeKey);

		if (timesPer) {
			return `${timesPer}×/${translatedType}`;
		} else if (interval === 1) {
			return translatedType;
		} else {
			// Use proper noun form with correct gender agreement
			const everyKey =
				type === 'daily'
					? 'payments.recurrence.everyNDays'
					: type === 'weekly'
						? 'payments.recurrence.everyNWeeks'
						: type === 'monthly'
							? 'payments.recurrence.everyNMonths'
							: 'payments.recurrence.everyNYears';
			return $_(everyKey, { values: { n: interval } });
		}
	}

	let todayStr = $derived(getLocalDateString());
	let isToday = $derived(targetDate === todayStr);
	let daysDiff = $derived(getDaysDiff(targetDate));
	let _isFuture = $derived(daysDiff > 0);
	let _isPast = $derived(daysDiff < 0);
	let relativeDateLabel = $derived(getRelativeDateLabel(targetDate));

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
	<div class="date-range-row">
		<!-- Start Date -->
		<div class="date-nav-row">
			<button
				class="nav-btn"
				onclick={goToPreviousPayment}
				disabled={!previousPaymentDate || isRangeMode}
				title={previousPaymentDate
					? `Go to ${formatDate(previousPaymentDate)}`
					: 'No earlier payments'}>⟨</button
			>

			<div class="date-display">
				<span class="date-label-small">{$_('overview.from')}</span>
				<input type="date" bind:value={targetDate} class="date-input" />
				<span class="date-label">
					{formatDate(targetDate)}
					{#if !isRangeMode && relativeDateLabel && !isToday}
						<span class="relative-label">({relativeDateLabel})</span>
					{/if}
				</span>
			</div>

			<button
				class="nav-btn"
				onclick={goToNextPayment}
				disabled={!nextPaymentDate || isRangeMode}
				title={nextPaymentDate ? `Go to ${formatDate(nextPaymentDate)}` : 'No future payments'}
				>⟩</button
			>
		</div>

		<!-- End Date (optional) -->
		<div class="date-range-to">
			<span class="range-arrow">{isRangeMode ? '→' : ''}</span>
			<div class="date-display end-date-display">
				<span class="date-label-small">{$_('overview.to')}</span>
				<input
					type="date"
					bind:value={endDate}
					min={targetDate}
					class="date-input"
					placeholder={$_('overview.selectEndDate')}
				/>
				{#if endDate}
					<span class="date-label">{formatDate(endDate)}</span>
				{:else}
					<span class="date-label hint">{$_('overview.optional')}</span>
				{/if}
			</div>
		</div>
	</div>

	<div class="date-actions">
		<button class="today-btn" onclick={goToToday} disabled={isToday && !endDate}>
			{$_('overview.today')}
		</button>
		{#if isRangeMode}
			<button class="clear-range-btn" onclick={() => (endDate = null)}>
				{$_('overview.clearRange')}
			</button>
			<span class="range-badge">{$_('overview.rangeMode')}</span>
		{/if}
	</div>
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

{#if error}
	<div class="error">{error}</div>
{/if}

{#if loading}
	<p>{$_('common.loading')}</p>
{:else if debts}
	<!-- Pool Ownerships (shown for each pool) -->
	{#each filteredPoolOwnerships as poolOwnership (poolOwnership.pool_id)}
		<section class="card">
			<h3>{poolOwnership.pool_name}</h3>
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
					<tfoot>
						<tr class="total-row">
							<td></td>
							<td><strong>{$_('common.total')}</strong></td>
							<td
								class:positive={poolOwnership.total_balance > 0}
								class:negative={poolOwnership.total_balance < 0}
							>
								<strong
									>{poolOwnership.total_balance >= 0 ? '+' : ''}{formatCurrency(
										poolOwnership.total_balance
									)}</strong
								>
							</td>
						</tr>
					</tfoot>
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
				<h3>{$_('overview.paymentsIncluded')} ({totalCount})</h3>
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
															<div class="transaction">
																<div class="trans-header">
																	<span class="trans-desc">
																		{occ.description}
																		{#if occ.is_recurring}
																			<span class="occ-tag recurring-tag"
																				>{$_('overview.recurring')}</span
																			>
																		{/if}
																	</span>
																	<span class="trans-amount">{formatCurrency(occ.amount)}</span>
																</div>
																<div class="trans-meta">
																	{#if payment}
																		{$_('overview.paidBy')}
																		{payment.payer_name ?? $_('common.unknown')}
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
		font-size: 1rem;
		cursor: pointer;
		transition:
			background 0.2s,
			opacity 0.2s;
		flex-shrink: 0;
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

	.relative-label {
		color: #888;
		font-style: italic;
	}

	.today-btn {
		background: #2a9d8f;
		color: white;
		border: none;
		border-radius: 8px;
		padding: 0.5rem 1rem;
		font-size: 0.9rem;
		cursor: pointer;
		transition: background 0.2s;
	}

	.today-btn:hover:not(:disabled) {
		background: #21867a;
	}

	.today-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Date range mode styles */
	.date-range-row {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		flex-wrap: wrap;
		width: 100%;
	}

	.date-range-to {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.range-arrow {
		font-size: 1.25rem;
		color: var(--accent, #7b61ff);
		min-width: 1rem;
	}

	.end-date-display {
		opacity: 0.7;
		transition: opacity 0.2s;
	}

	.date-selector.range-mode .end-date-display {
		opacity: 1;
	}

	.date-label-small {
		font-size: 0.7rem;
		color: #999;
		text-transform: uppercase;
		font-weight: 600;
		letter-spacing: 0.5px;
	}

	.date-label.hint {
		color: #999;
		font-style: italic;
	}

	.date-actions {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-wrap: wrap;
		justify-content: center;
	}

	.clear-range-btn {
		background: #e76f51;
		color: white;
		border: none;
		border-radius: 8px;
		padding: 0.5rem 1rem;
		font-size: 0.9rem;
		cursor: pointer;
		transition: background 0.2s;
	}

	.clear-range-btn:hover {
		background: #d45d41;
	}

	.range-badge {
		background: var(--accent, #7b61ff);
		color: white;
		padding: 0.25rem 0.75rem;
		border-radius: 12px;
		font-size: 0.75rem;
		font-weight: 600;
	}

	.date-selector.range-mode {
		border-left: 3px solid var(--accent, #7b61ff);
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

	/* Total row in table footer */
	.total-row td {
		border-top: 2px solid #e0e0e0;
		padding-top: 0.75rem;
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

	.occ-tag {
		padding: 0.1rem 0.4rem;
		border-radius: 4px;
		font-size: 0.65rem;
		font-weight: 600;
		text-transform: uppercase;
	}

	.recurring-tag {
		background: var(--accent, #7b61ff);
		color: white;
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
