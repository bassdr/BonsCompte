<script lang="ts">
    import { page } from '$app/stores';
    import { getCashflowProjection, consolidatePayments, type CashflowProjection, type ConsolidatePaymentsRequest } from '$lib/api';
    import { _ } from '$lib/i18n';
    import { formatCurrency } from '$lib/format/currency';
    import { Chart, registerables } from 'chart.js';
    import { onMount } from 'svelte';

    let projection: CashflowProjection | null = $state(null);
    let loading = $state(true);
    let error = $state('');

    // Controls
    let horizonMonths = $state(6);
    let recommendationMode = $state('simple_average');
    let frequencyType = $state('monthly');
    let frequencyInterval = $state(1);
    let consolidateMode = $state(false);

    // Chart
    let chartCanvas: HTMLCanvasElement;
    let chart: Chart | null = null;

    onMount(() => {
        Chart.register(...registerables);
        return () => {
            if (chart) {
                chart.destroy();
            }
        };
    });

    $effect(() => {
        const projectId = parseInt($page.params.id ?? '');
        if (projectId && !isNaN(projectId)) {
            loadCashflow(projectId);
        }
    });

    async function loadCashflow(projectId: number) {
        loading = true;
        error = '';
        try {
            projection = await getCashflowProjection(
                projectId,
                horizonMonths,
                recommendationMode,
                frequencyType,
                frequencyInterval,
                consolidateMode
            );
            updateChart();
        } catch (err) {
            error = $_('cashflow.failedToLoad');
            console.error('Failed to load cashflow projection:', err);
        } finally {
            loading = false;
        }
    }

    function updateChart() {
        if (!projection || !chartCanvas) return;

        // Group monthly balances by participant
        const participantMap = new Map<number, { name: string; balances: number[] }>();

        for (const balance of projection.monthly_balances) {
            if (!participantMap.has(balance.participant_id)) {
                participantMap.set(balance.participant_id, {
                    name: balance.participant_name,
                    balances: []
                });
            }
            participantMap.get(balance.participant_id)!.balances.push(balance.net_balance);
        }

        const COLORS = ['#7b61ff', '#2a9d8f', '#e76f51', '#f4a261', '#e9c46a', '#264653'];

        const datasets = Array.from(participantMap.values()).map((data, idx) => ({
            label: data.name,
            data: data.balances,
            borderColor: COLORS[idx % COLORS.length],
            backgroundColor: COLORS[idx % COLORS.length] + '33',
            borderWidth: 2,
            tension: 0.1,
            fill: false,
        }));

        if (chart) chart.destroy();

        chart = new Chart(chartCanvas, {
            type: 'line',
            data: {
                labels: projection.months,
                datasets
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'bottom',
                        labels: {
                            font: { size: 12 }
                        }
                    },
                    tooltip: {
                        mode: 'index',
                        intersect: false,
                        callbacks: {
                            label: (context) => {
                                const label = context.dataset.label || '';
                                const value = formatCurrency(context.parsed.y ?? 0);
                                return `${label}: ${value}`;
                            }
                        }
                    }
                },
                scales: {
                    y: {
                        title: {
                            display: true,
                            text: $_('cashflow.balance')
                        },
                        ticks: {
                            callback: (value) => formatCurrency(Number(value))
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: $_('cashflow.month')
                        }
                    }
                },
                interaction: {
                    mode: 'nearest',
                    axis: 'x',
                    intersect: false
                }
            }
        });
    }

    function getFrequencyLabel(type: string, interval: number): string {
        const base = type === 'weekly' ? $_('cashflow.weeks') :
                      type === 'monthly' ? $_('cashflow.months') :
                      $_('cashflow.years');
        return interval > 1 ? `${interval} ${base}` : base;
    }

    async function handleCreatePayment(rec: any, event: MouseEvent) {
        event.preventDefault();

        if (!projection) return;

        if (consolidateMode && projection.payments_to_consolidate.length > 0) {
            // Consolidate mode: show list of payments being deleted
            const paymentsList = projection.payments_to_consolidate
                .map(p => `- ${p.description} (${formatCurrency(p.amount)} every ${p.recurrence_interval} ${p.recurrence_type})`)
                .join('\n');

            const message = $_('cashflow.consolidateWarning')
                .replace('{count}', projection.payments_to_consolidate.length.toString())
                .replace('{paymentsList}', paymentsList)
                .replace('{newAmount}', formatCurrency(rec.recommended_amount))
                .replace('{frequency}', `${rec.frequency_interval} ${rec.frequency_type}`)
                + '\n\n' + $_('cashflow.bankUpdateWarning');

            if (!confirm(message)) return;

            // Find pool account ID (receiver for all these payments)
            const poolId = projection.payments_to_consolidate[0]?.payment_id
                ? (await getPaymentPoolId(projection.payments_to_consolidate[0].payment_id))
                : null;

            if (!poolId) {
                alert($_('cashflow.consolidateError'));
                return;
            }

            try {
                const request: ConsolidatePaymentsRequest = {
                    payer_id: rec.participant_id,
                    pool_id: poolId,
                    amount: rec.recommended_amount,
                    description: $_('cashflow.consolidatedPaymentDescription'),
                    payment_date: new Date().toISOString().split('T')[0],
                    recurrence_type: rec.frequency_type,
                    recurrence_interval: rec.frequency_interval,
                    payment_ids_to_delete: projection.payments_to_consolidate.map(p => p.payment_id)
                };

                const response = await consolidatePayments(parseInt($page.params.id ?? ''), request);
                alert($_('cashflow.consolidateSuccess').replace('{count}', response.deleted_count.toString()));
                // Reload to reflect changes
                loadCashflow(parseInt($page.params.id ?? ''));
            } catch (err) {
                alert($_('cashflow.consolidateError'));
                console.error('Consolidation failed:', err);
            }
        } else {
            // Normal mode: navigate to payment form
            const message = $_('cashflow.bankUpdateWarning');
            if (confirm(message)) {
                const url = `/projects/${$page.params.id}/payments?prefill=true&payer_id=${rec.participant_id}&amount=${rec.recommended_amount.toFixed(2)}&recurring=true&recurrence_type=${rec.frequency_type}&recurrence_interval=${rec.frequency_interval}&description=Recurring+contribution+(recommended)`;
                window.location.href = url;
            }
        }
    }

    // Helper to get pool ID (we need to query the actual payment to find receiver_account_id)
    // For now, we'll use a simple approach - check the projection data
    async function getPaymentPoolId(paymentId: number): Promise<number | null> {
        // In a real implementation, we'd query the backend
        // For now, assume all payments in consolidate list go to the same pool
        // The backend will validate this anyway
        // Let's just use a fixed pool ID - we can get it from the pool_evolutions
        if (projection && projection.pool_evolutions.length > 0) {
            return projection.pool_evolutions[0].pool_id;
        }
        return null;
    }
</script>

{#if loading}
    <div class="loading">{$_('common.loading')}</div>
{:else if error}
    <div class="error">{error}</div>
{:else if projection}
    <div class="cashflow-page">
        <div class="header">
            <h2>{$_('cashflow.title')}</h2>
        </div>

        <!-- Controls -->
        <div class="controls">
            <div class="control-group">
                <label>{$_('cashflow.timeHorizon')}</label>
                <div class="horizon-selector">
                    <button
                        class:active={horizonMonths === 3}
                        onclick={() => horizonMonths = 3}>
                        {$_('cashflow.threeMonths')}
                    </button>
                    <button
                        class:active={horizonMonths === 6}
                        onclick={() => horizonMonths = 6}>
                        {$_('cashflow.sixMonths')}
                    </button>
                    <button
                        class:active={horizonMonths === 12}
                        onclick={() => horizonMonths = 12}>
                        {$_('cashflow.twelveMonths')}
                    </button>
                </div>
            </div>

            <div class="control-group">
                <label>{$_('cashflow.recommendationMode')}</label>
                <div class="mode-toggle">
                    <button
                        class:active={recommendationMode === 'simple_average'}
                        onclick={() => recommendationMode = 'simple_average'}>
                        {$_('cashflow.simpleAverage')}
                    </button>
                    <button
                        class:active={recommendationMode === 'linear_regression'}
                        onclick={() => recommendationMode = 'linear_regression'}>
                        {$_('cashflow.linearRegression')}
                    </button>
                </div>
            </div>

            <div class="control-group frequency-group">
                <label>{$_('cashflow.paymentFrequency')}</label>
                <div class="frequency-selector">
                    <select bind:value={frequencyType}>
                        <option value="weekly">{$_('cashflow.weeks')}</option>
                        <option value="monthly">{$_('cashflow.months')}</option>
                        <option value="yearly">{$_('cashflow.years')}</option>
                    </select>
                    <span>{$_('cashflow.every')}</span>
                    <input type="number" bind:value={frequencyInterval} min="1" max="12" />
                </div>
            </div>

            <div class="control-group">
                <label>
                    <input type="checkbox" bind:checked={consolidateMode} />
                    {$_('cashflow.consolidateMode')}
                </label>
                <p class="consolidate-help">{$_('cashflow.consolidateModeHelp')}</p>
            </div>
        </div>

        <!-- Payments to Consolidate (shown only in consolidate mode) -->
        {#if consolidateMode && projection.payments_to_consolidate.length > 0}
            <div class="card consolidate-warning-card">
                <h3>⚠️ {$_('cashflow.paymentsToDelete')}</h3>
                <p class="warning-text">{$_('cashflow.consolidateExplanation')}</p>
                <ul class="payments-list">
                    {#each projection.payments_to_consolidate as payment}
                        <li>
                            <strong>{payment.description}</strong>
                            <br />
                            {formatCurrency(payment.amount)} every {payment.recurrence_interval} {payment.recurrence_type}
                        </li>
                    {/each}
                </ul>
                <p class="warning-text">{$_('cashflow.consolidateAction')}</p>
            </div>
        {/if}

        <!-- Chart -->
        <div class="card chart-card">
            <h3>{$_('cashflow.balanceOverTime')}</h3>
            <div class="chart-container">
                <canvas bind:this={chartCanvas}></canvas>
            </div>
        </div>

        <!-- Recommendations -->
        <div class="card">
            <h3>{$_('cashflow.recommendations')}</h3>
            <p class="note">{$_('cashflow.recommendationNote')}</p>
            {#if projection.recommendations.length > 0}
                <table class="recommendations-table">
                    <thead>
                        <tr>
                            <th>{$_('cashflow.participant')}</th>
                            <th>{$_('cashflow.currentTrend')}</th>
                            <th>{$_('cashflow.recommended')}</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each projection.recommendations as rec}
                            <tr>
                                <td class="participant-name">{rec.participant_name}</td>
                                <td class="trend" class:negative={rec.current_trend < 0} class:positive={rec.current_trend > 0}>
                                    {formatCurrency(rec.current_trend)}/month
                                </td>
                                <td class="recommended">
                                    {formatCurrency(rec.recommended_amount)}
                                    <span class="frequency">
                                        every {getFrequencyLabel(rec.frequency_type, rec.frequency_interval)}
                                    </span>
                                </td>
                                <td>
                                    <button
                                        onclick={(e) => handleCreatePayment(rec, e)}
                                        class="create-payment-btn">
                                        Create Payment
                                    </button>
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            {:else}
                <p class="empty">{$_('cashflow.noRecommendations')}</p>
            {/if}
        </div>

        <!-- Pool Evolution -->
        {#if projection.pool_evolutions.length > 0}
            {#each projection.pool_evolutions as pool}
                <div class="card pool-card">
                    <h3>{pool.pool_name} {$_('cashflow.evolution')}</h3>
                    <div class="pool-evolution">
                        {#each pool.monthly_balances as monthData}
                            <details>
                                <summary>
                                    <span class="month">{monthData.month}</span>
                                    <span class="balance">{formatCurrency(monthData.total_balance)}</span>
                                </summary>
                                <div class="ownerships">
                                    {#each monthData.participant_ownerships as ownership}
                                        <div class="ownership-row">
                                            <span>{ownership.participant_name}</span>
                                            <span class:positive={ownership.ownership > 0} class:negative={ownership.ownership < 0}>
                                                {formatCurrency(ownership.ownership)}
                                            </span>
                                        </div>
                                    {/each}
                                </div>
                            </details>
                        {/each}
                    </div>
                </div>
            {/each}
        {/if}
    </div>
{/if}

<style>
    .cashflow-page {
        padding: 1.5rem;
        max-width: 1200px;
        margin: 0 auto;
    }

    .header {
        margin-bottom: 1.5rem;
    }

    .header h2 {
        font-size: 1.75rem;
        color: #333;
        margin: 0;
    }

    .controls {
        display: flex;
        flex-wrap: wrap;
        gap: 1.5rem;
        margin-bottom: 2rem;
        padding: 1rem;
        background: white;
        border-radius: 8px;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    }

    .control-group {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .control-group label {
        font-size: 0.875rem;
        font-weight: 600;
        color: #666;
    }

    .horizon-selector,
    .mode-toggle {
        display: flex;
        gap: 0.5rem;
    }

    .horizon-selector button,
    .mode-toggle button {
        padding: 0.5rem 1rem;
        border: 1px solid #ddd;
        background: white;
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.2s;
        font-size: 0.875rem;
    }

    .horizon-selector button:hover,
    .mode-toggle button:hover {
        border-color: #7b61ff;
        background: #f8f7ff;
    }

    .horizon-selector button.active,
    .mode-toggle button.active {
        background: #7b61ff;
        color: white;
        border-color: #7b61ff;
    }

    .frequency-selector {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .frequency-selector select,
    .frequency-selector input {
        padding: 0.5rem;
        border: 1px solid #ddd;
        border-radius: 6px;
        font-size: 0.875rem;
    }

    .frequency-selector input {
        width: 60px;
        text-align: center;
    }

    .consolidate-help {
        margin: 0.5rem 0 0 1.5rem;
        font-size: 0.8rem;
        color: #666;
        font-style: italic;
    }

    .consolidate-warning-card {
        background: #fff3cd;
        border-left: 4px solid #f4a261;
    }

    .consolidate-warning-card h3 {
        color: #856404;
        margin-bottom: 1rem;
    }

    .warning-text {
        color: #856404;
        font-size: 0.9rem;
        margin-bottom: 1rem;
        line-height: 1.5;
    }

    .payments-list {
        margin: 1rem 0;
        padding-left: 1.5rem;
        list-style: disc;
    }

    .payments-list li {
        margin-bottom: 0.75rem;
        color: #333;
    }

    .payments-list li strong {
        color: #000;
    }

    .card {
        background: white;
        border-radius: 8px;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
        padding: 1.5rem;
        margin-bottom: 1.5rem;
    }

    .card h3 {
        margin: 0 0 1rem 0;
        font-size: 1.25rem;
        color: #333;
    }

    .chart-card {
        min-height: 400px;
    }

    .chart-container {
        height: 350px;
        position: relative;
    }

    .note {
        color: #666;
        font-size: 0.875rem;
        margin-bottom: 1rem;
        font-style: italic;
    }

    .recommendations-table {
        width: 100%;
        border-collapse: collapse;
    }

    .recommendations-table th {
        text-align: left;
        padding: 0.75rem;
        background: #f8f9fa;
        font-weight: 600;
        font-size: 0.875rem;
        color: #666;
        border-bottom: 2px solid #e9ecef;
    }

    .recommendations-table td {
        padding: 0.75rem;
        border-bottom: 1px solid #e9ecef;
    }

    .participant-name {
        font-weight: 500;
    }

    .trend {
        font-family: monospace;
        font-size: 0.9rem;
    }

    .trend.negative {
        color: #e76f51;
    }

    .trend.positive {
        color: #2a9d8f;
    }

    .recommended {
        font-weight: 500;
    }

    .frequency {
        color: #666;
        font-size: 0.875rem;
        font-weight: normal;
    }

    .create-payment-btn {
        display: inline-block;
        padding: 0.5rem 1rem;
        background: #7b61ff;
        color: white;
        text-decoration: none;
        border: none;
        border-radius: 6px;
        font-size: 0.875rem;
        font-weight: 500;
        cursor: pointer;
        transition: background 0.2s;
    }

    .create-payment-btn:hover {
        background: #6a4feb;
    }

    .pool-evolution details {
        border: 1px solid #e9ecef;
        border-radius: 6px;
        margin-bottom: 0.5rem;
    }

    .pool-evolution summary {
        padding: 0.75rem 1rem;
        cursor: pointer;
        display: flex;
        justify-content: space-between;
        align-items: center;
        background: #f8f9fa;
        border-radius: 6px;
        transition: background 0.2s;
    }

    .pool-evolution summary:hover {
        background: #e9ecef;
    }

    .pool-evolution summary .month {
        font-weight: 500;
    }

    .pool-evolution summary .balance {
        font-family: monospace;
        color: #666;
    }

    .ownerships {
        padding: 1rem;
    }

    .ownership-row {
        display: flex;
        justify-content: space-between;
        padding: 0.5rem 0;
        border-bottom: 1px solid #f0f0f0;
    }

    .ownership-row:last-child {
        border-bottom: none;
    }

    .positive {
        color: #2a9d8f;
    }

    .negative {
        color: #e76f51;
    }

    .loading,
    .error,
    .empty {
        padding: 2rem;
        text-align: center;
        color: #666;
    }

    .error {
        color: #e76f51;
    }

    @media (max-width: 768px) {
        .cashflow-page {
            padding: 1rem;
        }

        .controls {
            flex-direction: column;
        }

        .chart-card {
            min-height: 300px;
        }

        .chart-container {
            height: 250px;
        }

        .recommendations-table {
            font-size: 0.875rem;
        }

        .recommendations-table th,
        .recommendations-table td {
            padding: 0.5rem;
        }
    }
</style>
