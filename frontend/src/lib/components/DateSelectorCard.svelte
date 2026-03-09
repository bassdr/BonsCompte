<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import { _ } from '$lib/i18n';
  import DateInput from '$lib/components/DateInput.svelte';
  import type { OverviewState } from '$lib/date-projection.svelte';
  import { getLocalDateString } from '$lib/date-projection';
  let { projection }: { projection: OverviewState } = $props();

  let expanded = $state(false);

  // Summary helpers
  const isToday = $derived(projection.targetDate === getLocalDateString());

  const horizonLabel = $derived.by(() => {
    switch (projection.selectedHorizon) {
      case 'thisMonth':
        return $_('overview.thisMonth');
      case 'nextMonth':
        return $_('overview.nextMonth');
      case '3months':
        return $_('overview.threeMonths');
      case '6months':
        return $_('overview.sixMonths');
      case '12months':
        return $_('overview.twelveMonths');
      case 'custom':
        return projection.endDate ? projection.formatDate(projection.endDate) : '';
      default:
        return '';
    }
  });
</script>

<div class="date-selector card" class:range-mode={projection.isRangeMode}>
  <!-- Compact summary bar (always visible) -->
  <button class="summary-bar" onclick={() => (expanded = !expanded)}>
    <div class="summary-content">
      <SlidersHorizontal size={16} />
      <span class="summary-date">
        {#if isToday}
          {$_('overview.today')}
        {:else}
          {projection.formatDate(projection.targetDate)}
        {/if}
      </span>
      {#if horizonLabel}
        <span class="summary-badge horizon-badge">
          {$_('overview.horizon')}: {horizonLabel}
        </span>
      {/if}
    </div>
    <span class="summary-toggle">
      {#if expanded}
        <ChevronUp size={18} />
      {:else}
        <ChevronDown size={18} />
      {/if}
    </span>
  </button>

  <!-- Full controls (expandable) -->
  {#if expanded}
    <div class="controls">
      <!-- Start Date Row -->
      <div class="date-row">
        <span class="date-row-label">{$_('overview.from')}</span>
        <div class="date-row-center">
          <div class="date-nav-row">
            <button
              class="nav-btn"
              onclick={projection.goToPreviousPayment}
              disabled={!projection.previousPaymentDate}
              title={projection.previousPaymentDate
                ? $_('overview.goToDate', {
                    values: { date: projection.formatDate(projection.previousPaymentDate) }
                  })
                : $_('overview.noPreviousPayment')}><ChevronLeft size={20} /></button
            >

            <div class="date-display">
              <DateInput
                bind:value={projection.targetDate}
                class="date-input"
                buttons={['today']}
                disabledButtons={projection.fromDateDisabledButtons}
                disabledReasons={projection.fromDateDisabledReasons}
              />
            </div>

            <button
              class="nav-btn"
              onclick={projection.goToNextPayment}
              disabled={!projection.nextPaymentDate}
              title={projection.nextPaymentDate
                ? $_('overview.goToDate', {
                    values: { date: projection.formatDate(projection.nextPaymentDate) }
                  })
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
            class:active={projection.selectedHorizon === 'none'}
            onclick={() => projection.setHorizon('none')}
          >
            {$_('overview.horizonNone')}
          </button>
          <button
            class="horizon-btn"
            class:active={projection.selectedHorizon === 'thisMonth'}
            onclick={() => projection.setHorizon('thisMonth')}
          >
            {$_('overview.thisMonth')}
          </button>
          <button
            class="horizon-btn"
            class:active={projection.selectedHorizon === 'nextMonth'}
            onclick={() => projection.setHorizon('nextMonth')}
          >
            {$_('overview.nextMonth')}
          </button>
          <button
            class="horizon-btn"
            class:active={projection.selectedHorizon === '3months'}
            onclick={() => projection.setHorizon('3months')}
          >
            {$_('overview.threeMonths')}
          </button>
          <button
            class="horizon-btn"
            class:active={projection.selectedHorizon === '6months'}
            onclick={() => projection.setHorizon('6months')}
          >
            {$_('overview.sixMonths')}
          </button>
          <button
            class="horizon-btn"
            class:active={projection.selectedHorizon === '12months'}
            onclick={() => projection.setHorizon('12months')}
          >
            {$_('overview.twelveMonths')}
          </button>
        </div>
      </div>

      <!-- End Date Row (only shown when horizon is set) -->
      {#if projection.isRangeMode}
        <div class="date-row end-date-row">
          <span class="date-row-label">{$_('overview.to')}</span>
          <div class="date-nav-row">
            <button
              class="nav-btn"
              onclick={projection.goToPreviousEndPayment}
              disabled={!projection.previousEndPaymentDate}
              title={projection.previousEndPaymentDate
                ? $_('overview.goToDate', {
                    values: { date: projection.formatDate(projection.previousEndPaymentDate) }
                  })
                : $_('overview.noPreviousPayment')}><ChevronLeft size={20} /></button
            >

            <div class="date-display">
              <DateInput
                bind:value={projection.endDate}
                min={projection.targetDate}
                class="date-input"
                buttons={['clear', 'today']}
                disabledButtons={projection.toDateDisabledButtons}
                disabledReasons={projection.toDateDisabledReasons}
              />
            </div>

            <button
              class="nav-btn"
              onclick={projection.goToNextEndPayment}
              disabled={!projection.nextEndPaymentDate}
              title={projection.nextEndPaymentDate
                ? $_('overview.goToDate', {
                    values: { date: projection.formatDate(projection.nextEndPaymentDate) }
                  })
                : $_('overview.noNextPayment')}><ChevronRight size={20} /></button
            >
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 0;
    margin-bottom: 1.5rem;
  }

  .date-selector {
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 10;
    overflow: visible;
  }

  /* ─── Summary bar ──────────────────────────────────────────────────── */

  .summary-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.75rem 1.25rem;
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    border-radius: 16px;
    transition: background 0.15s;
  }

  .summary-bar:hover {
    background: rgba(0, 0, 0, 0.02);
  }

  .summary-content {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    color: #555;
    min-width: 0;
  }

  .summary-content :global(svg) {
    flex-shrink: 0;
    color: var(--accent, #7b61ff);
  }

  .summary-date {
    font-weight: 600;
    color: #333;
    font-size: 0.95rem;
  }

  .summary-badge {
    display: inline-flex;
    align-items: center;
    padding: 0.15rem 0.5rem;
    border-radius: 10px;
    font-size: 0.75rem;
    font-weight: 500;
    white-space: nowrap;
  }

  .horizon-badge {
    background: rgba(123, 97, 255, 0.1);
    color: var(--accent, #7b61ff);
  }

  .summary-toggle {
    flex-shrink: 0;
    color: #999;
    display: flex;
    align-items: center;
  }

  /* ─── Expanded controls ────────────────────────────────────────────── */

  .controls {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 0 1.5rem 1.5rem;
    border-top: 1px solid #eee;
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

  .date-row {
    display: grid;
    grid-template-columns: 5rem 1fr 6.5rem;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    padding-top: 0.75rem;
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

  @media (max-width: 600px) {
    .date-selector {
      overflow-x: hidden;
    }

    .controls {
      padding: 0 1rem 1rem;
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
      gap: 0.5rem;
    }

    .date-display {
      flex: 1;
      min-width: 0;
      max-width: 100%;
    }

    .date-display :global(.wx-datepicker) {
      width: 100%;
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

    .summary-content {
      gap: 0.4rem;
    }

    .summary-badge {
      font-size: 0.7rem;
      padding: 0.1rem 0.4rem;
    }
  }
</style>
