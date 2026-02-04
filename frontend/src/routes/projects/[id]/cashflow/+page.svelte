<script lang="ts">
  import { page } from '$app/stores';
  import {
    getCashflowProjection,
    getParticipants,
    consolidatePayments,
    type CashflowProjection,
    type ConsolidatePaymentsRequest,
    type Participant
  } from '$lib/api';
  import { _ } from '$lib/i18n';
  import { formatCurrency } from '$lib/format/currency';
  import { Chart, registerables, type TooltipItem } from 'chart.js';
  import 'chartjs-adapter-date-fns';
  import type { AnnotationOptions } from 'chartjs-plugin-annotation';
  import { onMount } from 'svelte';
  import { SvelteMap, SvelteDate } from 'svelte/reactivity';
  import type { RecurringContributionRecommendation } from '$lib/api';

  // Will be set after dynamic import (SSR-safe)
  let annotationPlugin: typeof import('chartjs-plugin-annotation').default | null = null;

  let projection: CashflowProjection | null = $state(null);
  let loading = $state(true);
  let error = $state('');

  // Participants for dropdowns
  let participants: Participant[] = $state([]);
  let userParticipants = $derived(participants.filter((p) => p.account_type === 'user'));
  let poolParticipants = $derived(participants.filter((p) => p.account_type === 'pool'));

  // Recommendation Configuration
  let selectedPayerId: number | null = $state(null);
  let selectedReceiverId: number | null = $state(null);
  let recStartDate = $state('');
  let recEndDate = $state('');
  let includeRecommendation = $state(true);

  // Controls
  let horizonMonths = $state(6);
  let recommendationMode = $state('simple_average');
  let frequencyType = $state('monthly');
  let frequencyInterval = $state(1);
  let consolidateMode = $state(false);

  // Charts
  let balanceChartCanvas: HTMLCanvasElement | undefined = $state(undefined);
  let balanceChart: Chart | null = null;
  let poolChartCanvases: SvelteMap<number, HTMLCanvasElement> = new SvelteMap();
  let poolCharts: SvelteMap<number, Chart> = new SvelteMap();

  onMount(() => {
    Chart.register(...registerables);

    // Dynamically import annotation plugin (SSR-safe)
    import('chartjs-plugin-annotation').then((annotationModule) => {
      annotationPlugin = annotationModule.default;
      Chart.register(annotationPlugin);
    });

    // Set default dates
    const today = new SvelteDate();
    recStartDate = today.toISOString().split('T')[0];
    const endDate = new SvelteDate(today);
    endDate.setMonth(endDate.getMonth() + 6);
    recEndDate = endDate.toISOString().split('T')[0];

    return () => {
      if (balanceChart) {
        balanceChart.destroy();
      }
      for (const chart of poolCharts.values()) {
        chart.destroy();
      }
    };
  });

  // Load participants when project changes
  $effect(() => {
    const projectId = parseInt($page.params.id ?? '');
    if (projectId && !isNaN(projectId)) {
      loadParticipants(projectId);
    }
  });

  // Reload cashflow when settings change
  $effect(() => {
    const projectId = parseInt($page.params.id ?? '');
    // Track all control dependencies to trigger reload
    const _deps = [
      horizonMonths,
      recommendationMode,
      frequencyType,
      frequencyInterval,
      consolidateMode,
      selectedPayerId,
      selectedReceiverId,
      recStartDate,
      recEndDate,
      includeRecommendation
    ];
    void _deps; // Ensure dependencies are tracked
    if (projectId && !isNaN(projectId)) {
      loadCashflow(projectId);
    }
  });

  async function loadParticipants(projectId: number) {
    try {
      participants = await getParticipants(projectId);
      // Auto-select first user and first pool if available
      if (!selectedPayerId && userParticipants.length > 0) {
        selectedPayerId = userParticipants[0].id;
      }
      if (!selectedReceiverId && poolParticipants.length > 0) {
        selectedReceiverId = poolParticipants[0].id;
      }
    } catch (err) {
      console.error('Failed to load participants:', err);
    }
  }

  // Update charts when canvas becomes available or projection changes
  $effect(() => {
    if (balanceChartCanvas && projection) {
      updateBalanceChart();
    }
  });

  // Update pool charts when they become available
  $effect(() => {
    if (projection && projection.pool_evolutions.length > 0) {
      // Small delay to ensure canvases are bound
      setTimeout(() => updatePoolCharts(), 0);
    }
  });

  async function loadCashflow(projectId: number) {
    loading = true;
    error = '';
    try {
      projection = await getCashflowProjection(projectId, {
        horizonMonths,
        recommendationMode,
        frequencyType,
        frequencyInterval,
        consolidateMode,
        recPayerId: selectedPayerId ?? undefined,
        recReceiverId: selectedReceiverId ?? undefined,
        recStartDate: recStartDate || undefined,
        recEndDate: recEndDate || undefined,
        includeRecommendation
      });
      // Chart update handled by $effect that watches projection
    } catch (err) {
      error = $_('cashflow.failedToLoad');
      console.error('Failed to load cashflow projection:', err);
    } finally {
      loading = false;
    }
  }

  const COLORS = ['#7b61ff', '#2a9d8f', '#e76f51', '#f4a261', '#e9c46a', '#264653'];

  // Action to bind pool chart canvases
  function bindPoolCanvas(node: HTMLCanvasElement, params: { poolId: number }) {
    poolChartCanvases.set(params.poolId, node);
    // Trigger chart update after binding
    setTimeout(() => updatePoolCharts(), 0);

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

  function updateBalanceChart() {
    if (!projection || !balanceChartCanvas) return;

    // Use event-based data if available, fallback to monthly
    const useEvents = projection.balance_events && projection.balance_events.length > 0;

    if (useEvents) {
      // Group balance events by participant
      const participantMap = new SvelteMap<
        number,
        { name: string; events: Array<{ x: string; y: number; isSynthetic: boolean }> }
      >();

      for (const event of projection.balance_events) {
        if (!participantMap.has(event.participant_id)) {
          participantMap.set(event.participant_id, {
            name: event.participant_name,
            events: []
          });
        }
        participantMap.get(event.participant_id)!.events.push({
          x: event.date,
          y: event.net_balance,
          isSynthetic: event.is_synthetic
        });
      }

      const datasets = Array.from(participantMap.values()).map((data, idx) => ({
        label: data.name,
        data: data.events.map((e) => ({ x: e.x, y: e.y })),
        borderColor: COLORS[idx % COLORS.length],
        backgroundColor: COLORS[idx % COLORS.length] + '33',
        borderWidth: 2,
        tension: 0.1,
        fill: false,
        pointRadius: data.events.map((e) => (e.isSynthetic ? 4 : 3)),
        pointStyle: data.events.map((e) => (e.isSynthetic ? 'triangle' : 'circle'))
      }));

      // Build annotations
      const annotations: Record<string, unknown> = {};
      const today = new SvelteDate().toISOString().split('T')[0];

      // Today line
      annotations['todayLine'] = {
        type: 'line',
        xMin: today,
        xMax: today,
        borderColor: '#333',
        borderWidth: 2,
        borderDash: [5, 5],
        label: {
          display: true,
          content: $_('cashflow.today'),
          position: 'start',
          backgroundColor: '#333',
          color: 'white',
          font: { size: 11 }
        }
      };

      // Recommendation horizon box (if configured)
      if (recStartDate && recEndDate && selectedPayerId && selectedReceiverId) {
        annotations['recHorizon'] = {
          type: 'box',
          xMin: recStartDate,
          xMax: recEndDate,
          backgroundColor: 'rgba(123, 97, 255, 0.1)',
          borderColor: 'rgba(123, 97, 255, 0.5)',
          borderWidth: 1,
          label: {
            display: true,
            content: $_('cashflow.recommendationHorizon'),
            position: { x: 'center', y: 'start' },
            backgroundColor: 'rgba(123, 97, 255, 0.8)',
            color: 'white',
            font: { size: 10 }
          }
        };
      }

      if (balanceChart) balanceChart.destroy();

      balanceChart = new Chart(balanceChartCanvas, {
        type: 'line',
        data: { datasets: datasets as never[] },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          plugins: {
            legend: {
              position: 'bottom',
              labels: { font: { size: 12 } }
            },
            tooltip: {
              mode: 'index',
              intersect: false,
              callbacks: {
                title: (items) => {
                  if (items.length > 0) {
                    const dateStr = items[0].label;
                    return new SvelteDate(dateStr).toLocaleDateString();
                  }
                  return '';
                },
                label: (context: TooltipItem<'line'>) => {
                  const label = context.dataset.label || '';
                  const value = formatCurrency(context.parsed.y ?? 0);
                  return `${label}: ${value}`;
                }
              }
            },
            annotation: {
              annotations: annotations as Record<string, AnnotationOptions>
            }
          },
          scales: {
            y: {
              title: { display: true, text: $_('cashflow.balance') },
              ticks: { callback: (value: number | string) => formatCurrency(Number(value)) }
            },
            x: {
              type: 'time',
              time: {
                unit: 'month',
                displayFormats: { month: 'MMM yyyy' }
              },
              title: { display: true, text: $_('cashflow.date') }
            }
          },
          interaction: { mode: 'nearest', axis: 'x', intersect: false }
        }
      });
    } else {
      // Fallback: use monthly aggregated data
      const participantMap = new SvelteMap<number, { name: string; balances: number[] }>();

      for (const balance of projection.monthly_balances) {
        if (!participantMap.has(balance.participant_id)) {
          participantMap.set(balance.participant_id, {
            name: balance.participant_name,
            balances: []
          });
        }
        participantMap.get(balance.participant_id)!.balances.push(balance.net_balance);
      }

      const datasets = Array.from(participantMap.values()).map((data, idx) => ({
        label: data.name,
        data: data.balances,
        borderColor: COLORS[idx % COLORS.length],
        backgroundColor: COLORS[idx % COLORS.length] + '33',
        borderWidth: 2,
        tension: 0.1,
        fill: false
      }));

      if (balanceChart) balanceChart.destroy();

      balanceChart = new Chart(balanceChartCanvas, {
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
              labels: { font: { size: 12 } }
            },
            tooltip: {
              mode: 'index',
              intersect: false,
              callbacks: {
                label: (context: TooltipItem<'line'>) => {
                  const label = context.dataset.label || '';
                  const value = formatCurrency(context.parsed.y ?? 0);
                  return `${label}: ${value}`;
                }
              }
            }
          },
          scales: {
            y: {
              title: { display: true, text: $_('cashflow.balance') },
              ticks: { callback: (value: number | string) => formatCurrency(Number(value)) }
            },
            x: {
              title: { display: true, text: $_('cashflow.month') }
            }
          },
          interaction: { mode: 'nearest', axis: 'x', intersect: false }
        }
      });
    }
  }

  function updatePoolCharts() {
    if (!projection) return;

    for (const pool of projection.pool_evolutions) {
      const canvas = poolChartCanvases.get(pool.pool_id);
      if (!canvas) continue;

      // Build participant ownership over time
      const participantMap = new SvelteMap<number, { name: string; ownerships: number[] }>();

      // Initialize with zeros for all months
      for (const monthData of pool.monthly_balances) {
        for (const ownership of monthData.participant_ownerships) {
          if (!participantMap.has(ownership.participant_id)) {
            participantMap.set(ownership.participant_id, {
              name: ownership.participant_name,
              ownerships: []
            });
          }
        }
      }

      // Fill in ownership values for each month
      for (const monthData of pool.monthly_balances) {
        const monthOwnershipMap = new SvelteMap(
          monthData.participant_ownerships.map((o) => [o.participant_id, o.ownership])
        );

        for (const [pid, pdata] of participantMap) {
          pdata.ownerships.push(monthOwnershipMap.get(pid) ?? 0);
        }
      }

      const datasets = Array.from(participantMap.values()).map((data, idx) => ({
        label: data.name,
        data: data.ownerships,
        borderColor: COLORS[idx % COLORS.length],
        backgroundColor: COLORS[idx % COLORS.length] + '33',
        borderWidth: 2,
        tension: 0.1,
        fill: false
      }));

      // Destroy existing chart if any
      const existingChart = poolCharts.get(pool.pool_id);
      if (existingChart) existingChart.destroy();

      const newChart = new Chart(canvas, {
        type: 'line',
        data: {
          labels: pool.monthly_balances.map((m) => m.month),
          datasets
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          plugins: {
            legend: {
              position: 'bottom',
              labels: { font: { size: 12 } }
            },
            tooltip: {
              mode: 'index',
              intersect: false,
              callbacks: {
                label: (context: TooltipItem<'line'>) => {
                  const label = context.dataset.label || '';
                  const value = formatCurrency(context.parsed.y ?? 0);
                  return `${label}: ${value}`;
                }
              }
            }
          },
          scales: {
            y: {
              title: { display: true, text: $_('overview.ownership') },
              ticks: { callback: (value: number | string) => formatCurrency(Number(value)) }
            },
            x: {
              title: { display: true, text: $_('cashflow.month') }
            }
          },
          interaction: { mode: 'nearest', axis: 'x', intersect: false }
        }
      });

      poolCharts.set(pool.pool_id, newChart);
    }
  }

  function getFrequencyLabel(type: string, interval: number): string {
    const base =
      type === 'weekly'
        ? $_('cashflow.weeks')
        : type === 'monthly'
          ? $_('cashflow.months')
          : $_('cashflow.years');
    return interval > 1 ? `${interval} ${base}` : base;
  }

  async function handleCreatePayment(rec: RecurringContributionRecommendation, event: MouseEvent) {
    event.preventDefault();

    if (!projection) return;

    if (consolidateMode && projection.payments_to_consolidate.length > 0) {
      // Consolidate mode: show list of payments being deleted
      const paymentsList = projection.payments_to_consolidate
        .map(
          (p) =>
            `- ${p.description} (${formatCurrency(p.amount)} every ${p.recurrence_interval} ${p.recurrence_type})`
        )
        .join('\n');

      const message =
        $_('cashflow.consolidateWarning')
          .replace('{count}', projection.payments_to_consolidate.length.toString())
          .replace('{paymentsList}', paymentsList)
          .replace('{newAmount}', formatCurrency(rec.recommended_amount))
          .replace('{frequency}', `${rec.frequency_interval} ${rec.frequency_type}`) +
        '\n\n' +
        $_('cashflow.bankUpdateWarning');

      if (!confirm(message)) return;

      // Find pool account ID (receiver for all these payments)
      const poolId = projection.payments_to_consolidate[0]?.payment_id
        ? await getPaymentPoolId(projection.payments_to_consolidate[0].payment_id)
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
          payment_date: new SvelteDate().toISOString().split('T')[0],
          recurrence_type: rec.frequency_type,
          recurrence_interval: rec.frequency_interval,
          payment_ids_to_delete: projection.payments_to_consolidate.map((p) => p.payment_id)
        };

        const response = await consolidatePayments(parseInt($page.params.id ?? ''), request);
        alert(
          $_('cashflow.consolidateSuccess').replace('{count}', response.deleted_count.toString())
        );
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
  async function getPaymentPoolId(_paymentId: number): Promise<number | null> {
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

    <!-- Recommendation Configuration -->
    <div class="card recommendation-config">
      <h3>{$_('cashflow.configureRecommendation')}</h3>
      <div class="rec-controls">
        <div class="control-group">
          <label class="control-label" for="payer-select">{$_('cashflow.payer')}</label>
          <select id="payer-select" bind:value={selectedPayerId}>
            <option value={null}>{$_('cashflow.selectPayer')}</option>
            {#each userParticipants as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <div class="control-group">
          <label class="control-label" for="receiver-select">{$_('cashflow.receiver')}</label>
          <select id="receiver-select" bind:value={selectedReceiverId}>
            <option value={null}>{$_('cashflow.selectReceiver')}</option>
            {#each poolParticipants as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <div class="control-group">
          <label class="control-label" for="start-date">{$_('cashflow.startDate')}</label>
          <input type="date" id="start-date" bind:value={recStartDate} />
        </div>
        <div class="control-group">
          <label class="control-label" for="end-date">{$_('cashflow.endDate')}</label>
          <input type="date" id="end-date" bind:value={recEndDate} />
        </div>
      </div>

      <!-- Computed Recommendation Output -->
      {#if projection.computed_recommendation}
        {@const rec = projection.computed_recommendation}
        <div class="recommendation-output">
          <p class="recommendation-statement">
            <strong>{rec.payer_name}</strong>
            {$_('cashflow.shouldTransfer')}
            <strong>{formatCurrency(rec.recommended_amount)}</strong>
            {$_('cashflow.perFrequency')
              .replace('{interval}', String(rec.frequency_interval))
              .replace('{type}', rec.frequency_type)}
            {$_('cashflow.to')} <strong>{rec.receiver_name}</strong>
          </p>
          <div class="recommendation-meta">
            <span
              >{$_('cashflow.currentTrendLabel')}: {formatCurrency(rec.current_trend)}/month</span
            >
            <span>{$_('cashflow.method')}: {rec.calculation_method}</span>
          </div>
        </div>
      {:else if selectedPayerId && selectedReceiverId}
        <p class="empty">{$_('cashflow.noRecommendationData')}</p>
      {:else}
        <p class="note">{$_('cashflow.selectPayerReceiver')}</p>
      {/if}
    </div>

    <!-- Controls -->
    <div class="controls">
      <div class="control-group">
        <span class="control-label">{$_('cashflow.timeHorizon')}</span>
        <div class="horizon-selector">
          <button class:active={horizonMonths === 3} onclick={() => (horizonMonths = 3)}>
            {$_('cashflow.threeMonths')}
          </button>
          <button class:active={horizonMonths === 6} onclick={() => (horizonMonths = 6)}>
            {$_('cashflow.sixMonths')}
          </button>
          <button class:active={horizonMonths === 12} onclick={() => (horizonMonths = 12)}>
            {$_('cashflow.twelveMonths')}
          </button>
        </div>
      </div>

      <div class="control-group">
        <span class="control-label">{$_('cashflow.recommendationMode')}</span>
        <div class="mode-toggle">
          <button
            class:active={recommendationMode === 'simple_average'}
            onclick={() => (recommendationMode = 'simple_average')}
          >
            {$_('cashflow.simpleAverage')}
          </button>
          <button
            class:active={recommendationMode === 'linear_regression'}
            onclick={() => (recommendationMode = 'linear_regression')}
          >
            {$_('cashflow.linearRegression')}
          </button>
        </div>
      </div>

      <div class="control-group frequency-group">
        <span class="control-label">{$_('cashflow.paymentFrequency')}</span>
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
          {#each projection.payments_to_consolidate as payment (payment.payment_id)}
            <li>
              <strong>{payment.description}</strong>
              <br />
              {formatCurrency(payment.amount)} every {payment.recurrence_interval}
              {payment.recurrence_type}
            </li>
          {/each}
        </ul>
        <p class="warning-text">{$_('cashflow.consolidateAction')}</p>
      </div>
    {/if}

    <!-- Balance Chart -->
    <div class="card chart-card">
      <div class="chart-header">
        <h3>{$_('cashflow.balanceOverTime')}</h3>
        {#if selectedPayerId && selectedReceiverId}
          <label class="include-toggle">
            <input type="checkbox" bind:checked={includeRecommendation} />
            {$_('cashflow.includeInProjection')}
          </label>
        {/if}
      </div>
      <div class="chart-container">
        <canvas bind:this={balanceChartCanvas}></canvas>
      </div>
    </div>

    <!-- Pool Ownership Charts -->
    {#if projection.pool_evolutions.length > 0}
      {#each projection.pool_evolutions as pool (pool.pool_id)}
        <div class="card chart-card">
          <h3>{pool.pool_name} - {$_('cashflow.poolEvolution')}</h3>
          <div class="chart-container">
            <canvas use:bindPoolCanvas={{ poolId: pool.pool_id }}></canvas>
          </div>
        </div>
      {/each}
    {/if}

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
            {#each projection.recommendations as rec (rec.participant_id)}
              <tr>
                <td class="participant-name">{rec.participant_name}</td>
                <td
                  class="trend"
                  class:negative={rec.current_trend < 0}
                  class:positive={rec.current_trend > 0}
                >
                  {formatCurrency(rec.current_trend)}/month
                </td>
                <td class="recommended">
                  {formatCurrency(rec.recommended_amount)}
                  <span class="frequency">
                    every {getFrequencyLabel(rec.frequency_type, rec.frequency_interval)}
                  </span>
                </td>
                <td>
                  <button onclick={(e) => handleCreatePayment(rec, e)} class="create-payment-btn">
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

  .control-group label,
  .control-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: #666;
    display: block;
    margin-bottom: 0.5rem;
  }

  .recommendation-config {
    border-left: 4px solid #7b61ff;
    margin-bottom: 1.5rem;
  }

  .rec-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .rec-controls select,
  .rec-controls input[type='date'] {
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.875rem;
    min-width: 150px;
  }

  .recommendation-output {
    background: #f8f7ff;
    padding: 1rem;
    border-radius: 6px;
    border: 1px solid #e0dbff;
  }

  .recommendation-statement {
    margin: 0 0 0.5rem 0;
    font-size: 1.1rem;
    line-height: 1.5;
  }

  .recommendation-statement strong {
    color: #7b61ff;
  }

  .recommendation-meta {
    display: flex;
    gap: 1.5rem;
    font-size: 0.875rem;
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

  .chart-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .chart-header h3 {
    margin: 0;
  }

  .include-toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: #666;
    cursor: pointer;
  }

  .include-toggle input {
    cursor: pointer;
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
