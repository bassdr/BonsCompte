<script lang="ts">
    import { page } from '$app/stores';
    import { getDebts, getPayments, type DebtSummary, type PaymentWithContributions } from "$lib/api";

    let debts: DebtSummary | null = $state(null);
    let allPayments: PaymentWithContributions[] = $state([]);
    let loading = $state(true);
    let error = $state('');
    let showOccurrences = $state(false);

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
    let paymentDates = $derived.by(() => {
        if (!debts?.occurrences) return [];
        const dates = [...new Set(debts.occurrences.map(o => o.occurrence_date))];
        return dates.sort();
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
        const date = parseLocalDate(dateStr);
        return date.toLocaleDateString('en-US', {
            weekday: 'short',
            year: 'numeric',
            month: 'short',
            day: 'numeric'
        });
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
        if (days === 0) return 'Today';
        if (days === 1) return 'Tomorrow';
        if (days === -1) return 'Yesterday';
        if (days > 0 && days <= 7) return `in ${days} days`;
        if (days < 0 && days >= -7) return `${Math.abs(days)} days ago`;
        return '';
    }

    let todayStr = $derived(getLocalDateString());
    let isToday = $derived(targetDate === todayStr);
    let daysDiff = $derived(getDaysDiff(targetDate));
    let isFuture = $derived(daysDiff > 0);
    let isPast = $derived(daysDiff < 0);
    let relativeDateLabel = $derived(getRelativeDateLabel(targetDate));
</script>

<h2>Debts Summary</h2>

<!-- Date selector -->
<div class="date-selector card">
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
                <span class="badge future-badge">Future</span>
            {:else if isPast}
                <span class="badge past-badge">Past</span>
            {/if}
        </span>
    </div>

    <button
        class="nav-btn"
        onclick={goToNextPayment}
        disabled={!nextPaymentDate}
        title={nextPaymentDate ? `Go to ${formatDate(nextPaymentDate)}` : 'No future payments'}
    >⟩</button>

    <button
        class="today-btn"
        onclick={goToToday}
        disabled={isToday}
    >
        Today
    </button>
</div>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if loading}
    <p>Loading...</p>
{:else if debts}
    <section class="card">
        <h3>Balances</h3>
        <table class="balance-table">
            <thead>
                <tr>
                    <th>Participant</th>
                    <th>Paid</th>
                    <th>Owes</th>
                    <th>Balance</th>
                </tr>
            </thead>
            <tbody>
                {#each debts.balances as b}
                    <tr>
                        <td>{b.participant_name}</td>
                        <td>${b.total_paid.toFixed(2)}</td>
                        <td>${b.total_owed.toFixed(2)}</td>
                        <td class:positive={b.net_balance > 0} class:negative={b.net_balance < 0}>
                            {b.net_balance >= 0 ? '+' : ''}${b.net_balance.toFixed(2)}
                        </td>
                    </tr>
                {/each}
            </tbody>
        </table>
    </section>

    <section class="card">
        <h3>Settlements</h3>
        {#if debts.settlements.length === 0}
            <p class="all-settled">All settled up!</p>
        {:else}
            <ul class="settlements-list">
                {#each debts.settlements as s}
                    <li>
                        <span class="from">{s.from_participant_name}</span>
                        <span class="arrow">&rarr;</span>
                        <span class="to">{s.to_participant_name}</span>
                        <span class="amount">${s.amount.toFixed(2)}</span>
                    </li>
                {/each}
            </ul>
        {/if}
    </section>

    <!-- All payment occurrences (expandable) -->
    {#if debts.occurrences && debts.occurrences.length > 0}
        <section class="card occurrences-card">
            <button
                class="expand-header"
                onclick={() => showOccurrences = !showOccurrences}
            >
                <span class="expand-icon">{showOccurrences ? '▼' : '▶'}</span>
                <h3>Payments Included ({debts.occurrences.length})</h3>
            </button>

            {#if showOccurrences}
                <ul class="occurrences-list">
                    {#each debts.occurrences as occ}
                        {@const occRelative = getRelativeDateLabel(occ.occurrence_date)}
                        <li>
                            <span class="occ-date">
                                {formatDate(occ.occurrence_date)}
                                {#if occRelative}
                                    <span class="occ-relative">({occRelative})</span>
                                {/if}
                            </span>
                            <span class="occ-desc">
                                {occ.description}
                                {#if occ.is_recurring}
                                    <span class="occ-tag recurring-tag">recurring</span>
                                {:else}
                                    <span class="occ-tag single-tag">single</span>
                                {/if}
                            </span>
                            <span class="occ-amount">${occ.amount.toFixed(2)}</span>
                        </li>
                    {/each}
                </ul>
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
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
        flex-wrap: wrap;
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
        padding: 0 1rem;
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
        margin-left: 0.5rem;
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
        font-size: 0.8rem;
    }

    .occurrences-list {
        list-style: none;
        padding: 0 1.5rem 1.5rem;
        margin: 0;
    }

    .occurrences-list li {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 0.75rem 0;
        border-bottom: 1px solid #eee;
    }

    .occurrences-list li:last-child {
        border-bottom: none;
    }

    .occ-date {
        font-size: 0.85rem;
        color: #666;
        min-width: 180px;
    }

    .occ-relative {
        color: #888;
        font-style: italic;
        font-size: 0.8rem;
    }

    .occ-desc {
        flex: 1;
        font-weight: 500;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
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

    .single-tag {
        background: #e0e0e0;
        color: #666;
    }

    .occ-amount {
        font-weight: 600;
        color: var(--accent, #7b61ff);
    }
</style>
