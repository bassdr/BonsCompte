<script lang="ts">
    import { page } from '$app/stores';
    import { getDebts, getPayments, type DebtSummary, type PaymentWithContributions, type PairwiseBalance } from "$lib/api";
    import { _ } from '$lib/i18n';
    import { formatDate as formatDateI18n, formatDateWithWeekday, formatMonthYear as formatMonthYearI18n } from '$lib/format/date';
    import { formatCurrency } from '$lib/format/currency';

    let debts: DebtSummary | null = $state(null);
    let allPayments: PaymentWithContributions[] = $state([]);
    let loading = $state(true);
    let error = $state('');
    let showOccurrences = $state(false);

    // Track which balance rows are expanded to show pairwise details
    let expandedBalanceRows = $state<Set<number>>(new Set());

    // Track which pool ownership entries are expanded
    let expandedPoolEntries = $state<Set<number>>(new Set());
    let expandedPoolYears = $state<Map<number, Set<string>>>(new Map());
    let expandedPoolMonths = $state<Map<number, Map<string, Set<string>>>>(new Map());

    // Focus mode: filter to show only one participant's perspective
    let focusParticipantId = $state<number | null>(null);

    // Settlement mode: false = minimal (default), true = direct-only
    let useDirectSettlements = $state(false);

    function toggleBalanceRow(participantId: number) {
        const newSet = new Set(expandedBalanceRows);
        if (newSet.has(participantId)) {
            newSet.delete(participantId);
        } else {
            newSet.add(participantId);
        }
        expandedBalanceRows = newSet;
    }

    function togglePoolEntry(participantId: number) {
        const newSet = new Set(expandedPoolEntries);
        if (newSet.has(participantId)) {
            newSet.delete(participantId);
        } else {
            newSet.add(participantId);
        }
        expandedPoolEntries = newSet;
    }

    function isPoolEntryExpanded(participantId: number): boolean {
        return expandedPoolEntries.has(participantId);
    }

    function togglePoolYear(participantId: number, year: string) {
        const newMap = new Map(expandedPoolYears);
        if (!newMap.has(participantId)) {
            newMap.set(participantId, new Set());
        }
        const yearSet = newMap.get(participantId)!;
        if (yearSet.has(year)) {
            yearSet.delete(year);
        } else {
            yearSet.add(year);
        }
        expandedPoolYears = newMap;
    }

    function togglePoolMonth(participantId: number, year: string, monthKey: string) {
        const newMap = new Map(expandedPoolMonths);
        if (!newMap.has(participantId)) {
            newMap.set(participantId, new Map());
        }
        const yearMap = newMap.get(participantId)!;
        if (!yearMap.has(year)) {
            yearMap.set(year, new Set());
        }
        const monthSet = yearMap.get(year)!;
        if (monthSet.has(monthKey)) {
            monthSet.delete(monthKey);
        } else {
            monthSet.add(monthKey);
        }
        expandedPoolMonths = newMap;
    }

    // Track which pairwise rows are expanded
    let expandedPairwise = $state<Set<string>>(new Set());
    let expandedPairwiseYears = $state<Map<string, Set<string>>>(new Map());
    let expandedPairwiseMonths = $state<Map<string, Map<string, Set<string>>>>(new Map());

    function togglePairwiseRow(participantId: number, otherId: number) {
        const key = `${participantId}-${otherId}`;
        const newSet = new Set(expandedPairwise);
        if (newSet.has(key)) {
            newSet.delete(key);
        } else {
            newSet.add(key);
        }
        expandedPairwise = newSet;
    }

    function isPairwiseExpanded(participantId: number, otherId: number): boolean {
        return expandedPairwise.has(`${participantId}-${otherId}`);
    }

    function togglePairwiseYear(pairwiseKey: string, year: string) {
        const newMap = new Map(expandedPairwiseYears);
        if (!newMap.has(pairwiseKey)) {
            newMap.set(pairwiseKey, new Set());
        }
        const yearSet = newMap.get(pairwiseKey)!;
        if (yearSet.has(year)) {
            yearSet.delete(year);
        } else {
            yearSet.add(year);
        }
        expandedPairwiseYears = newMap;
    }

    function togglePairwiseMonth(pairwiseKey: string, year: string, monthKey: string) {
        const newMap = new Map(expandedPairwiseMonths);
        if (!newMap.has(pairwiseKey)) {
            newMap.set(pairwiseKey, new Map());
        }
        const yearMap = newMap.get(pairwiseKey)!;
        if (!yearMap.has(year)) {
            yearMap.set(year, new Set());
        }
        const monthSet = yearMap.get(year)!;
        if (monthSet.has(monthKey)) {
            monthSet.delete(monthKey);
        } else {
            monthSet.add(monthKey);
        }
        expandedPairwiseMonths = newMap;
    }

    // Group breakdown by year/month, newest first
    function groupBreakdownByYearMonth(breakdown: any[]) {
        const yearMap = new Map<string, any>();

        for (const item of breakdown) {
            const year = getYear(item.occurrence_date);
            const monthKey = getMonthKey(item.occurrence_date);

            if (!yearMap.has(year)) {
                yearMap.set(year, {
                    year,
                    total: 0,
                    months: new Map()
                });
            }

            const yearData = yearMap.get(year);
            if (!yearData.months.has(monthKey)) {
                yearData.months.set(monthKey, {
                    monthKey,
                    month: formatMonthYear(item.occurrence_date),
                    total: 0,
                    items: []
                });
            }

            const monthData = yearData.months.get(monthKey);
            monthData.items.push(item);
            monthData.total += item.amount;
            yearData.total += item.amount;
        }

        // Convert to array and sort by year (newest first)
        const sortedYears = [...yearMap.entries()].sort((a, b) => b[0].localeCompare(a[0]));

        // For each year, sort months (newest first) and items (newest first)
        const result = new Map<string, any>();
        for (const [year, yearData] of sortedYears) {
            const sortedMonths = [...yearData.months.entries()].sort((a, b) => b[0].localeCompare(a[0]));

            yearData.months = new Map();
            for (const [monthKey, monthData] of sortedMonths) {
                monthData.items.sort((a: any, b: any) => b.occurrence_date.localeCompare(a.occurrence_date));
                yearData.months.set(monthKey, monthData);
            }

            result.set(year, yearData);
        }

        return result;
    }

    // Get pairwise balances for a specific participant
    function getPairwiseForParticipant(participantId: number): PairwiseBalance[] {
        if (!debts?.pairwise_balances) return [];
        return debts.pairwise_balances.filter(p => p.participant_id === participantId);
    }

    // Check if a settlement involves the focused participant
    function settlementInvolvesFocus(s: { from_participant_id: number; to_participant_id: number }): boolean {
        if (focusParticipantId === null) return true;
        return s.from_participant_id === focusParticipantId || s.to_participant_id === focusParticipantId;
    }

    // Check if a payment involves the focused participant (as payer or contributor)
    function paymentInvolvesFocus(paymentId: number, payerId: number | null): boolean {
        if (focusParticipantId === null) return true;
        if (payerId === focusParticipantId) return true;

        const payment = paymentMap.get(paymentId);
        if (payment?.contributions) {
            return payment.contributions.some((c: { participant_id: number }) => c.participant_id === focusParticipantId);
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
        return activeSettlements.filter(s => settlementInvolvesFocus(s));
    });

    // Get filtered pool ownerships based on focus
    let filteredPoolOwnerships = $derived.by(() => {
        if (!debts?.pool_ownerships || debts.pool_ownerships.length === 0) return [];
        if (focusParticipantId === null) return debts.pool_ownerships;

        return debts.pool_ownerships.map(pool => {
            const filteredEntries = pool.entries.filter(
                e => e.participant_id === focusParticipantId
            );
            return {
                ...pool,
                entries: filteredEntries,
                total_balance: filteredEntries.reduce((sum, e) => sum + e.ownership, 0)
            };
        }).filter(pool => pool.entries.length > 0);
    });

    // Get pool IDs for filtering balances
    let poolIds = $derived.by(() => {
        if (!debts?.pool_ownerships) return new Set<number>();
        return new Set(debts.pool_ownerships.map(p => p.pool_id));
    });

    // Get non-pool balances for display and selector
    let nonPoolBalances = $derived.by(() => {
        if (!debts) return [];
        return debts.balances.filter(b => !poolIds.has(b.participant_id));
    });

    // Get filtered balances based on focus
    let filteredBalances = $derived.by(() => {
        if (!debts) return [];
        if (focusParticipantId === null) {
            return nonPoolBalances;
        }
        return nonPoolBalances.filter(b => b.participant_id === focusParticipantId);
    });

    // Date state - default to today (using local date)
    function getLocalDateString(date: Date = new Date()): string {
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        return `${year}-${month}-${day}`;
    }

    function parseLocalDate(dateStr: string): Date {
        const [year, month, day] = dateStr.split('-').map(Number);
        return new Date(year, month - 1, day);
    }

    let targetDate = $state(getLocalDateString());

    let projectId = $derived(parseInt($page.params.id ?? ''));

    // Get unique payment dates (from all payments, considering recurrence)
    // Always include today as a navigation point
    let paymentDates = $derived.by(() => {
        if (!debts?.occurrences) return [];
        const dates = new Set(debts.occurrences.map(o => o.occurrence_date));
        dates.add(todayStr); // Add today to the set
        return [...dates].sort();
    });

    // Find previous date with payments (from current occurrences)
    let previousPaymentDate = $derived.by(() => {
        const currentDates = paymentDates.filter(d => d < targetDate);
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
            return new Date(year, month + 1, 0).getDate();
        }

        // Handle enhanced weekly with weekdays pattern
        if (payment.recurrence_weekdays && recurrenceType === 'weekly') {
            try {
                const weekPatterns: number[][] = JSON.parse(payment.recurrence_weekdays);
                if (weekPatterns.length === 0) return null;

                // Start searching from afterDate
                const searchStart = new Date(afterDate);
                searchStart.setDate(searchStart.getDate() + 1); // Day after target

                // Calculate which week in the cycle we're in
                const startWeekday = startDate.getDay();
                const weekStart = new Date(startDate);
                weekStart.setDate(weekStart.getDate() - startWeekday);

                const msPerWeek = 7 * 24 * 60 * 60 * 1000;
                const cycleWeeks = interval <= 4 ? interval : interval;

                // Search forward from afterDate
                for (let daysAhead = 0; daysAhead < 365 * 2; daysAhead++) {
                    const checkDate = new Date(searchStart);
                    checkDate.setDate(checkDate.getDate() + daysAhead);

                    if (checkDate < startDate) continue;

                    // Check end date
                    if (payment.recurrence_end_date) {
                        const endDate = parseLocalDate(payment.recurrence_end_date);
                        if (checkDate > endDate) return null;
                    }

                    // Calculate week number from start
                    const checkWeekStart = new Date(checkDate);
                    checkWeekStart.setDate(checkWeekStart.getDate() - checkDate.getDay());
                    const weeksDiff = Math.floor((checkWeekStart.getTime() - weekStart.getTime()) / msPerWeek);
                    const cycleWeek = ((weeksDiff % weekPatterns.length) + weekPatterns.length) % weekPatterns.length;

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
                        const checkDate = new Date(year, month, actualDay);

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
                        const checkDate = new Date(year, jsMonth, actualDay);

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
                case 'weekly': dayInterval = Math.max(1, Math.floor(7 / timesPer)); break;
                case 'monthly': dayInterval = Math.max(1, Math.floor(30 / timesPer)); break;
                case 'yearly': dayInterval = Math.max(1, Math.floor(365 / timesPer)); break;
                default: dayInterval = 1;
            }
        } else {
            // Every X periods
            switch (recurrenceType) {
                case 'daily': dayInterval = interval; break;
                case 'weekly': dayInterval = interval * 7; break;
                case 'monthly': dayInterval = interval * 30; break;
                case 'yearly': dayInterval = interval * 365; break;
                default: dayInterval = 30;
            }
        }

        // Find next occurrence
        let current = new Date(startDate);
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
            // Load debts up to target date
            debts = await getDebts(projectId, targetDate);
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
                const newYears = new Set([year]);
                expandedYears = newYears;

                // Also expand first month
                const firstMonthEntry = [...yearData.months.entries()][0];
                if (firstMonthEntry) {
                    const [monthKey] = firstMonthEntry;
                    const newMonths = new Map();
                    newMonths.set(year, new Set([monthKey]));
                    expandedMonths = newMonths;
                }
                autoExpandedOnce = true;
            }
        }
    });

    function goToToday() {
        targetDate = getLocalDateString();
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
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        date.setHours(0, 0, 0, 0);
        return Math.round((date.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
    }

    // Get relative date label
    function getRelativeDateLabel(dateStr: string): string {
        const days = getDaysDiff(dateStr);
        if (days === 0) return $_('debts.today');
        if (days === 1) return $_('debts.tomorrow');
        if (days === -1) return $_('debts.yesterday');
        if (days > 0 && days <= 7) return $_('debts.inDays', { values: { days } });
        if (days < 0 && days >= -7) return $_('debts.daysAgo', { values: { days: Math.abs(days) } });
        return '';
    }

    // Check if date is in the future
    function isFutureDate(dateStr: string): boolean {
        return getDaysDiff(dateStr) > 0;
    }

    // Format recurrence info for display
    function formatRecurrence(payment: any): string {
        if (!payment.is_recurring) return '';
        const type = payment.recurrence_type || 'monthly';
        const interval = payment.recurrence_interval || 1;
        const timesPer = payment.recurrence_times_per;

        // Get translated recurrence type
        const typeKey = `payments.recurrence.${type}`;
        const translatedType = $_(typeKey);

        if (timesPer) {
            return `${timesPer}×/${translatedType}`;
        } else if (interval === 1) {
            return translatedType;
        } else {
            return `${$_('payments.recurrence.every')} ${interval} ${translatedType}`;
        }
    }

    let todayStr = $derived(getLocalDateString());
    let isToday = $derived(targetDate === todayStr);
    let daysDiff = $derived(getDaysDiff(targetDate));
    let isFuture = $derived(daysDiff > 0);
    let isPast = $derived(daysDiff < 0);
    let relativeDateLabel = $derived(getRelativeDateLabel(targetDate));

    // Map payment_id to full payment details for showing contributions
    let paymentMap = $derived.by(() => {
        const map = new Map();
        for (const payment of allPayments) {
            map.set(payment.id, payment);
        }
        return map;
    });

    // Track expanded years and months
    let expandedYears = $state(new Set<string>());
    let expandedMonths = $state(new Map<string, Set<string>>());

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
        if (!debts?.occurrences) return new Map();

        // First filter occurrences to only those involving the focused participant
        let occurrencesToShow = debts.occurrences;
        if (focusParticipantId !== null) {
            occurrencesToShow = debts.occurrences.filter(occ =>
                paymentInvolvesFocus(occ.payment_id, occ.payer_id)
            );
        }

        // Then reverse for newest first
        const reversed = [...occurrencesToShow].reverse();

        // Group by year > month > date
        const yearMap = new Map<string, any>();

        for (const occ of reversed) {
            const year = getYear(occ.occurrence_date);
            const monthKey = getMonthKey(occ.occurrence_date);
            const dateStr = occ.occurrence_date;

            if (!yearMap.has(year)) {
                yearMap.set(year, {
                    year,
                    expanded: expandedYears.has(year),
                    months: new Map()
                });
            }

            const yearData = yearMap.get(year);
            if (!yearData.months.has(monthKey)) {
                yearData.months.set(monthKey, {
                    monthKey,
                    month: formatMonthYear(dateStr),
                    expanded: expandedMonths.get(year)?.has(monthKey) ?? false,
                    totalAmount: 0,
                    dates: new Map(),
                    participantTotals: new Map<string, number>()
                });
            }

            const monthData = yearData.months.get(monthKey);
            if (!monthData.dates.has(dateStr)) {
                monthData.dates.set(dateStr, {
                    date: dateStr,
                    totalAmount: 0,
                    occurrences: []
                });
            }

            const dateData = monthData.dates.get(dateStr);
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
        const newSet = new Set(expandedYears);
        if (newSet.has(year)) {
            newSet.delete(year);
        } else {
            newSet.add(year);
        }
        expandedYears = newSet;
    }

    // Toggle month expanded state
    function toggleMonth(year: string, monthKey: string) {
        const newMap = new Map(expandedMonths);
        if (!newMap.has(year)) {
            newMap.set(year, new Set());
        }
        const monthSet = newMap.get(year)!;
        if (monthSet.has(monthKey)) {
            monthSet.delete(monthKey);
        } else {
            monthSet.add(monthKey);
        }
        expandedMonths = newMap;
    }

    // Get total for a year
    function getYearTotal(yearData: any): number {
        let total = 0;
        for (const monthData of yearData.months.values()) {
            total += monthData.totalAmount;
        }
        return total;
    }

    // Get month count for a year
    function getMonthCount(yearData: any): number {
        return yearData.months.size;
    }

    // Count total transactions in a month
    function getTransactionCount(monthData: any): number {
        let count = 0;
        for (const dateData of monthData.dates.values()) {
            count += dateData.occurrences.length;
        }
        return count;
    }

    // Count total transactions in a year
    function getYearTransactionCount(yearData: any): number {
        let count = 0;
        for (const monthData of yearData.months.values()) {
            count += getTransactionCount(monthData);
        }
        return count;
    }

    // Get per-participant totals for a year
    function getYearParticipantTotals(yearData: any): Map<string, number> {
        const totals = new Map<string, number>();
        for (const monthData of yearData.months.values()) {
            for (const [participantName, amount] of monthData.participantTotals.entries()) {
                const current = totals.get(participantName) || 0;
                totals.set(participantName, current + amount);
            }
        }
        return totals;
    }
</script>

<h2>{$_('debts.summary')}</h2>

<!-- Date selector -->
<div class="date-selector card">
    <div class="date-nav-row">
        <button
            class="nav-btn"
            onclick={goToPreviousPayment}
            disabled={!previousPaymentDate}
            title={previousPaymentDate ? `Go to ${formatDate(previousPaymentDate)}` : 'No earlier payments'}
        >⟨</button>

        <div class="date-display">
            <input
                type="date"
                bind:value={targetDate}
                class="date-input"
            />
            <span class="date-label">
                {formatDate(targetDate)}
                {#if relativeDateLabel && !isToday}
                    <span class="relative-label">({relativeDateLabel})</span>
                {/if}
                {#if isFuture}
                    <span class="badge future-badge">{$_('debts.future')}</span>
                {:else if isPast}
                    <span class="badge past-badge">{$_('debts.past')}</span>
                {/if}
            </span>
        </div>

        <button
            class="nav-btn"
            onclick={goToNextPayment}
            disabled={!nextPaymentDate}
            title={nextPaymentDate ? `Go to ${formatDate(nextPaymentDate)}` : 'No future payments'}
        >⟩</button>
    </div>

    <button
        class="today-btn"
        onclick={goToToday}
        disabled={isToday}
    >
        {$_('debts.today')}
    </button>
</div>

<!-- Focus mode selector -->
{#if debts}
    <div class="focus-selector">
        <label for="focus-participant">{$_('debts.focusOn')}</label>
        <select id="focus-participant" bind:value={focusParticipantId}>
            <option value={null}>{$_('debts.allParticipants')}</option>
            {#each nonPoolBalances as b}
                <option value={b.participant_id}>{b.participant_name}</option>
            {/each}
        </select>
        {#if focusParticipantId !== null}
            <button class="clear-focus" onclick={() => focusParticipantId = null}>{$_('common.clear')}</button>
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
    {#each filteredPoolOwnerships as poolOwnership}
        <section class="card">
            <h3>{poolOwnership.pool_name}</h3>
            {#if poolOwnership.entries.length === 0}
                <p class="no-ownership">{$_('debts.noPoolActivity')}</p>
            {:else}
                <table class="balance-table">
                    <thead>
                        <tr>
                            <th></th>
                            <th>{$_('debts.participant')}</th>
                            <th>{$_('debts.ownership')}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each poolOwnership.entries as entry}
                            {@const isExpanded = isPoolEntryExpanded(entry.participant_id)}
                            {@const poolKey = `${poolOwnership.pool_id}-${entry.participant_id}`}
                            <tr class="balance-row" class:expanded={isExpanded} class:focused={focusParticipantId === entry.participant_id} onclick={() => togglePoolEntry(entry.participant_id)}>
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
                                                    <div class="detail-header">{entry.participant_name} contributed {formatCurrency(entry.contributed)} to {poolOwnership.pool_name}</div>
                                                    {#if entry.contributed_breakdown && entry.contributed_breakdown.length > 0}
                                                        {@const groupedYears = groupBreakdownByYearMonth(entry.contributed_breakdown)}
                                                        <div class="breakdown-hierarchy">
                                                            {#each [...groupedYears.entries()] as [year, yearData]}
                                                                {@const yearExpanded = expandedPoolYears.get(entry.participant_id)?.has(year) ?? false}
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
                                                                        {#each [...yearData.months.entries()] as [monthKey, monthData]}
                                                                            {@const monthExpanded = expandedPoolMonths.get(entry.participant_id)?.get(year)?.has(monthKey) ?? false}
                                                                            <div class="breakdown-month">
                                                                                <button
                                                                                    class="breakdown-month-btn"
                                                                                    onclick={() => togglePoolMonth(entry.participant_id, year, monthKey)}
                                                                                >
                                                                                    <span class="expand-icon">{monthExpanded ? '▼' : '▶'}</span>
                                                                                    <span class="month-text">{monthData.month}</span>
                                                                                    <span class="month-total">{formatCurrency(monthData.total)}</span>
                                                                                </button>
                                                                                {#if monthExpanded}
                                                                                    <ul class="breakdown-list">
                                                                                        {#each monthData.items as item}
                                                                                            <li class="breakdown-item">
                                                                                                <span class="breakdown-desc">{item.description}</span>
                                                                                                <span class="breakdown-date">{item.occurrence_date}</span>
                                                                                                <span class="breakdown-amount">{formatCurrency(item.amount)}</span>
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
                                                    <div class="detail-header">{entry.participant_name} consumed {formatCurrency(entry.consumed)} from {poolOwnership.pool_name}</div>
                                                    {#if entry.consumed_breakdown && entry.consumed_breakdown.length > 0}
                                                        {@const groupedYears = groupBreakdownByYearMonth(entry.consumed_breakdown)}
                                                        <div class="breakdown-hierarchy">
                                                            {#each [...groupedYears.entries()] as [year, yearData]}
                                                                {@const yearExpanded = expandedPoolYears.get(entry.participant_id)?.has(`${year}-consumed`) ?? false}
                                                                <div class="breakdown-year">
                                                                    <button
                                                                        class="breakdown-year-btn"
                                                                        onclick={() => togglePoolYear(entry.participant_id, `${year}-consumed`)}
                                                                    >
                                                                        <span class="expand-icon">{yearExpanded ? '▼' : '▶'}</span>
                                                                        <span class="year-text">{year}</span>
                                                                        <span class="year-total">{formatCurrency(yearData.total)}</span>
                                                                    </button>
                                                                    {#if yearExpanded}
                                                                        {#each [...yearData.months.entries()] as [monthKey, monthData]}
                                                                            {@const monthExpanded = expandedPoolMonths.get(entry.participant_id)?.get(`${year}-consumed`)?.has(monthKey) ?? false}
                                                                            <div class="breakdown-month">
                                                                                <button
                                                                                    class="breakdown-month-btn"
                                                                                    onclick={() => togglePoolMonth(entry.participant_id, `${year}-consumed`, monthKey)}
                                                                                >
                                                                                    <span class="expand-icon">{monthExpanded ? '▼' : '▶'}</span>
                                                                                    <span class="month-text">{monthData.month}</span>
                                                                                    <span class="month-total">{formatCurrency(monthData.total)}</span>
                                                                                </button>
                                                                                {#if monthExpanded}
                                                                                    <ul class="breakdown-list">
                                                                                        {#each monthData.items as item}
                                                                                            <li class="breakdown-item">
                                                                                                <span class="breakdown-desc">{item.description}</span>
                                                                                                <span class="breakdown-date">{item.occurrence_date}</span>
                                                                                                <span class="breakdown-amount">{formatCurrency(item.amount)}</span>
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
                            <td class:positive={poolOwnership.total_balance > 0} class:negative={poolOwnership.total_balance < 0}>
                                <strong>{poolOwnership.total_balance >= 0 ? '+' : ''}{formatCurrency(poolOwnership.total_balance)}</strong>
                            </td>
                        </tr>
                    </tfoot>
                </table>
            {/if}
        </section>
    {/each}

    <section class="card">
        <h3>{$_('debts.balances')}</h3>
        <table class="balance-table">
            <thead>
                <tr>
                    <th></th>
                    <th>{$_('debts.participant')}</th>
                    <th>{$_('debts.balance')}</th>
                </tr>
            </thead>
            <tbody>
                {#each filteredBalances as b}
                    {@const isExpanded = expandedBalanceRows.has(b.participant_id)}
                    {@const pairwise = getPairwiseForParticipant(b.participant_id)}
                    <tr class="balance-row" class:expanded={isExpanded} class:focused={focusParticipantId === b.participant_id} onclick={() => toggleBalanceRow(b.participant_id)}>
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
                                        <p class="no-relationships">{$_('debts.noRelationships')}</p>
                                    {:else}
                                        <div class="pairwise-list">
                                            {#each pairwise as pw}
                                                {@const isExpanded = isPairwiseExpanded(b.participant_id, pw.other_participant_id)}
                                                {@const hasDetails = pw.amount_paid_for > 0.01 || pw.amount_owed_by > 0.01}
                                                <button
                                                    class="pairwise-item"
                                                    class:expanded={isExpanded}
                                                    onclick={() => togglePairwiseRow(b.participant_id, pw.other_participant_id)}
                                                >
                                                    <span class="pairwise-toggle">{isExpanded ? '▼' : '▶'}</span>
                                                    <span class="pairwise-name">{pw.other_participant_name}</span>
                                                    <span class="pairwise-net" class:positive={pw.net > 0} class:negative={pw.net < 0}>
                                                        {pw.net >= 0 ? '+' : ''}{formatCurrency(pw.net)}
                                                    </span>
                                                </button>
                                                {#if isExpanded}
                                                    {@const pairwiseKey = `${b.participant_id}-${pw.other_participant_id}`}
                                                    <div class="pairwise-details-expanded">
                                                        {#if pw.amount_paid_for > 0.01}
                                                            <div class="detail-section">
                                                                <div class="detail-header">{b.participant_name} paid {formatCurrency(pw.amount_paid_for)} for {pw.other_participant_name}</div>
                                                                {#if pw.paid_for_breakdown && pw.paid_for_breakdown.length > 0}
                                                                    {@const groupedYears = groupBreakdownByYearMonth(pw.paid_for_breakdown)}
                                                                    <div class="breakdown-hierarchy">
                                                                        {#each [...groupedYears.entries()] as [year, yearData]}
                                                                            {@const yearExpanded = expandedPairwiseYears.get(pairwiseKey)?.has(year) ?? false}
                                                                            <div class="breakdown-year">
                                                                                <button
                                                                                    class="breakdown-year-btn"
                                                                                    onclick={() => togglePairwiseYear(pairwiseKey, year)}
                                                                                >
                                                                                    <span class="expand-icon">{yearExpanded ? '▼' : '▶'}</span>
                                                                                    <span class="year-text">{year}</span>
                                                                                    <span class="year-total">{formatCurrency(yearData.total)}</span>
                                                                                </button>
                                                                                {#if yearExpanded}
                                                                                    {#each [...yearData.months.entries()] as [monthKey, monthData]}
                                                                                        {@const monthExpanded = expandedPairwiseMonths.get(pairwiseKey)?.get(year)?.has(monthKey) ?? false}
                                                                                        <div class="breakdown-month">
                                                                                            <button
                                                                                                class="breakdown-month-btn"
                                                                                                onclick={() => togglePairwiseMonth(pairwiseKey, year, monthKey)}
                                                                                            >
                                                                                                <span class="expand-icon">{monthExpanded ? '▼' : '▶'}</span>
                                                                                                <span class="month-text">{monthData.month}</span>
                                                                                                <span class="month-total">{formatCurrency(monthData.total)}</span>
                                                                                            </button>
                                                                                            {#if monthExpanded}
                                                                                                <ul class="breakdown-list">
                                                                                                    {#each monthData.items as item}
                                                                                                        <li class="breakdown-item">
                                                                                                            <span class="breakdown-desc">{item.description}</span>
                                                                                                            <span class="breakdown-date">{item.occurrence_date}</span>
                                                                                                            <span class="breakdown-amount">{formatCurrency(item.amount)}</span>
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
                                                                <div class="detail-header">{pw.other_participant_name} paid {formatCurrency(pw.amount_owed_by)} for {b.participant_name}</div>
                                                                {#if pw.owed_by_breakdown && pw.owed_by_breakdown.length > 0}
                                                                    {@const groupedYears = groupBreakdownByYearMonth(pw.owed_by_breakdown)}
                                                                    <div class="breakdown-hierarchy">
                                                                        {#each [...groupedYears.entries()] as [year, yearData]}
                                                                            {@const yearExpanded = expandedPairwiseYears.get(`${pairwiseKey}-owed`)?.has(year) ?? false}
                                                                            <div class="breakdown-year">
                                                                                <button
                                                                                    class="breakdown-year-btn"
                                                                                    onclick={() => togglePairwiseYear(`${pairwiseKey}-owed`, year)}
                                                                                >
                                                                                    <span class="expand-icon">{yearExpanded ? '▼' : '▶'}</span>
                                                                                    <span class="year-text">{year}</span>
                                                                                    <span class="year-total">{formatCurrency(yearData.total)}</span>
                                                                                </button>
                                                                                {#if yearExpanded}
                                                                                    {#each [...yearData.months.entries()] as [monthKey, monthData]}
                                                                                        {@const monthExpanded = expandedPairwiseMonths.get(`${pairwiseKey}-owed`)?.get(year)?.has(monthKey) ?? false}
                                                                                        <div class="breakdown-month">
                                                                                            <button
                                                                                                class="breakdown-month-btn"
                                                                                                onclick={() => togglePairwiseMonth(`${pairwiseKey}-owed`, year, monthKey)}
                                                                                            >
                                                                                                <span class="expand-icon">{monthExpanded ? '▼' : '▶'}</span>
                                                                                                <span class="month-text">{monthData.month}</span>
                                                                                                <span class="month-total">{formatCurrency(monthData.total)}</span>
                                                                                            </button>
                                                                                            {#if monthExpanded}
                                                                                                <ul class="breakdown-list">
                                                                                                    {#each monthData.items as item}
                                                                                                        <li class="breakdown-item">
                                                                                                            <span class="breakdown-desc">{item.description}</span>
                                                                                                            <span class="breakdown-date">{item.occurrence_date}</span>
                                                                                                            <span class="breakdown-amount">{formatCurrency(item.amount)}</span>
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
    </section>

    <section class="card">
        <div class="settlements-header">
            <h3>{$_('debts.settlements')}</h3>
            <label class="settlement-mode-toggle">
                <input type="checkbox" bind:checked={useDirectSettlements} />
                {$_('debts.directOnlyMode')}
            </label>
        </div>
        {#if filteredSettlements.length === 0}
            <p class="all-settled">{focusParticipantId !== null ? $_('debts.noSettlementsForParticipant') : $_('debts.allSettled')}</p>
        {:else}
            <ul class="settlements-list">
                {#each filteredSettlements as s}
                    <li class:focused={focusParticipantId !== null}>
                        <span class="from" class:highlight={s.from_participant_id === focusParticipantId}>{s.from_participant_name}</span>
                        <span class="arrow">&rarr;</span>
                        <span class="to" class:highlight={s.to_participant_id === focusParticipantId}>{s.to_participant_name}</span>
                        <span class="amount">{formatCurrency(s.amount)}</span>
                    </li>
                {/each}
            </ul>
        {/if}
    </section>

    <!-- All payment occurrences (expandable) -->
    {#if debts.occurrences && debts.occurrences.length > 0}
        {@const totalCount = focusParticipantId === null ? debts.occurrences.length : debts.occurrences.filter(occ => paymentInvolvesFocus(occ.payment_id, occ.payer_id)).length}
        <section class="card occurrences-card">
            <button
                class="expand-header"
                onclick={() => showOccurrences = !showOccurrences}
            >
                <span class="expand-icon">{showOccurrences ? '▼' : '▶'}</span>
                <h3>{$_('debts.paymentsIncluded')} ({totalCount})</h3>
            </button>

            {#if showOccurrences}
                <div class="hierarchy-container">
                    {#each [...groupedOccurrences.entries()] as [year, yearData]}
                        <!-- Year group -->
                        <div class="year-group">
                            <button
                                class="year-header"
                                onclick={() => toggleYear(year)}
                            >
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
                                            {getYearTransactionCount(yearData)} {getYearTransactionCount(yearData) === 1 ? $_('debts.transaction') : $_('debts.transactions')}
                                        </div>
                                        <div class="year-summary-participants">
                                            {#each [...getYearParticipantTotals(yearData).entries()].sort((a, b) => b[1] - a[1]) as [participantName, amount]}
                                                <span class="participant-chip">{participantName}: {formatCurrency(amount)}</span>
                                            {/each}
                                        </div>
                                    </div>
                                {/if}
                            </button>

                            {#if yearData.expanded}
                                {#each [...yearData.months.entries()] as [monthKey, monthData]}
                                    <!-- Month group -->
                                    <div class="month-group">
                                        <button
                                            class="month-header"
                                            onclick={() => toggleMonth(year, monthKey)}
                                        >
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
                                                        {getTransactionCount(monthData)} {getTransactionCount(monthData) === 1 ? $_('debts.transaction') : $_('debts.transactions')}
                                                    </div>
                                                    <div class="month-summary-participants">
                                                        {#each [...monthData.participantTotals.entries()].sort((a, b) => b[1] - a[1]) as [participantName, amount]}
                                                            <span class="participant-chip">{participantName}: {formatCurrency(amount)}</span>
                                                        {/each}
                                                    </div>
                                                </div>
                                            {/if}
                                        </button>

                                        {#if monthData.expanded}
                                            {#each [...monthData.dates.entries()] as [dateKey, dateData]}
                                                <!-- Date group -->
                                                <div class="date-group">
                                                    <div class="date-label">
                                                        {formatDate(dateData.date)}
                                                        <span class="date-total">{formatCurrency(dateData.totalAmount)}</span>
                                                    </div>

                                                    <!-- Individual transactions -->
                                                    <div class="transactions">
                                                        {#each dateData.occurrences as occ}
                                                            {@const payment = paymentMap.get(occ.payment_id)}
                                                            {@const occRelative = getRelativeDateLabel(occ.occurrence_date)}
                                                            <div class="transaction">
                                                                <div class="trans-header">
                                                                    <span class="trans-desc">
                                                                        {occ.description}
                                                                        {#if occ.is_recurring}
                                                                            <span class="occ-tag recurring-tag">{$_('debts.recurring')}</span>
                                                                        {/if}
                                                                    </span>
                                                                    <span class="trans-amount">{formatCurrency(occ.amount)}</span>
                                                                </div>
                                                                <div class="trans-meta">
                                                                    {#if payment}
                                                                        {$_('debts.paidBy')} {payment.payer_name ?? $_('common.unknown')}
                                                                        {#if payment.is_recurring && payment.recurrence_end_date}
                                                                            {$_('debts.from')} {formatDate(payment.payment_date)} {$_('debts.to')} {formatDate(payment.recurrence_end_date)}
                                                                        {:else}
                                                                            {isFutureDate(occ.occurrence_date) ? $_('debts.from') : $_('debts.on')} {formatDate(occ.occurrence_date)}
                                                                        {/if}
                                                                        {#if payment.is_recurring}
                                                                            <span class="recurrence-badge">{formatRecurrence(payment)}</span>
                                                                        {/if}
                                                                    {:else}
                                                                        {$_('debts.on')} {formatDate(occ.occurrence_date)}
                                                                    {/if}
                                                                    {#if occRelative}
                                                                        <span class="trans-relative">({occRelative})</span>
                                                                    {/if}
                                                                </div>
                                                                {#if payment && payment.contributions && payment.contributions.length > 0}
                                                                    <div class="trans-splits">
                                                                        {#each payment.contributions as c}
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
        transition: background 0.2s, opacity 0.2s;
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

    .badge {
        padding: 0.15rem 0.5rem;
        border-radius: 12px;
        font-size: 0.7rem;
        font-weight: 600;
        text-transform: uppercase;
    }

    .future-badge {
        background: #ff9f43;
        color: white;
    }

    .past-badge {
        background: #6c757d;
        color: white;
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
    }

    .detail-section:last-child {
        margin-bottom: 0;
    }

    .detail-header {
        font-size: 1.05rem;
        color: #222;
        font-weight: 700;
        padding: 0.75rem 0;
        margin-bottom: 0.5rem;
        border-bottom: 2px solid #d0d0d0;
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

    .year-text, .month-text {
        flex: 1;
        font-weight: 500;
        color: #333;
    }

    .year-total, .month-total {
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

    .settlement-mode-toggle input[type="checkbox"] {
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
