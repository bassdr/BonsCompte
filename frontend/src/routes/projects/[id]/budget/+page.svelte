<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import Loader from '@lucide/svelte/icons/loader';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Pencil from '@lucide/svelte/icons/pencil';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import {
    getBudget,
    getBudgetOverrides,
    createBudgetOverride,
    deleteBudgetOverride,
    updatePreferences,
    type BudgetResponse,
    type BudgetLineItem,
    type BudgetOverride,
    type CreateBudgetOverride
  } from '$lib/api';
  import { _ } from '$lib/i18n';
  import { participants } from '$lib/stores/project';
  import { preferences } from '$lib/stores/preferences';
  import { formatCurrency } from '$lib/format/currency';
  import { getErrorKey } from '$lib/errors';
  import { auth } from '$lib/auth';
  import type { UserPreferences } from '$lib/auth';
  import { SvelteSet } from 'svelte/reactivity';

  let budget: BudgetResponse | null = $state(null);
  let overrides: BudgetOverride[] = $state([]);
  let loading = $state(true);
  let errorKey = $state('');

  // Configuration
  let selectedParticipantId = $state<number | undefined>(undefined);
  let viewMode = $state<'budgeted' | 'observed' | 'comparison'>('budgeted');

  // Budget preferences (local state synced from user prefs)
  let payFrequency = $state<UserPreferences['budget_pay_frequency']>('biweekly');
  let hoursPerWeek = $state<number>(40);

  // Override form
  let showOverrideForm = $state(false);
  let overrideForm = $state<CreateBudgetOverride>({
    name: '',
    yearly_amount: 0,
    override_type: 'add',
    tag: null,
    participant_id: null,
    linked_payment_id: null
  });
  let savingOverride = $state(false);

  // Expanded sections
  let expandedSections = new SvelteSet<string>();

  function toggleSection(key: string) {
    if (expandedSections.has(key)) {
      expandedSections.delete(key);
    } else {
      expandedSections.add(key);
    }
  }

  // Time column calculations
  function getPaychecksPerYear(freq: string): number {
    switch (freq) {
      case 'weekly':
        return 52;
      case 'biweekly':
        return 26;
      case 'semimonthly':
        return 24;
      case 'monthly':
        return 12;
      default:
        return 26;
    }
  }

  function calculateTimeAmount(yearlyAmount: number, column: string): number {
    const paychecksPerYear = getPaychecksPerYear(payFrequency);
    switch (column) {
      case 'year':
        return yearlyAmount;
      case 'semester':
        return yearlyAmount / 2;
      case 'month':
        return yearlyAmount / 12;
      case 'paycheck':
        return yearlyAmount / paychecksPerYear;
      case 'week':
        return yearlyAmount / 52;
      case 'hour':
        return yearlyAmount / (52 * hoursPerWeek);
      default:
        return yearlyAmount;
    }
  }

  // Time columns to display
  const timeColumns = ['year', 'month', 'paycheck', 'week', 'hour'] as const;

  function getColumnLabel(col: string): string {
    return $_(`budget.${col}`);
  }

  // Group items by tag
  function groupByTag(items: BudgetLineItem[]): Map<string, BudgetLineItem[]> {
    const groups = new Map<string, BudgetLineItem[]>();
    for (const item of items) {
      const tag = item.tag || $_('budget.uncategorized');
      if (!groups.has(tag)) {
        groups.set(tag, []);
      }
      groups.get(tag)!.push(item);
    }
    return groups;
  }

  // Derived data
  let currentSummary = $derived.by(() => {
    const b = budget;
    if (!b) return null;
    return viewMode === 'observed' ? b.observed : b.budgeted;
  });

  let incomeItems = $derived(
    currentSummary?.items.filter((i: BudgetLineItem) => i.is_income) ?? []
  );
  let expenseItems = $derived(
    currentSummary?.items.filter((i: BudgetLineItem) => !i.is_income) ?? []
  );
  let incomeByTag = $derived(groupByTag(incomeItems));
  let expenseByTag = $derived(groupByTag(expenseItems));

  // Comparison data
  let comparisonData = $derived.by(() => {
    if (!budget) return [];

    const budgetedByTag = new Map<string, number>();
    const observedByTag = new Map<string, number>();

    for (const item of budget.budgeted.items) {
      const tag = item.tag || $_('budget.uncategorized');
      const current = budgetedByTag.get(tag) || 0;
      budgetedByTag.set(tag, current + (item.is_income ? item.yearly_amount : -item.yearly_amount));
    }

    for (const item of budget.observed.items) {
      const tag = item.tag || $_('budget.uncategorized');
      const current = observedByTag.get(tag) || 0;
      observedByTag.set(tag, current + (item.is_income ? item.yearly_amount : -item.yearly_amount));
    }

    const allTags = new Set([...budgetedByTag.keys(), ...observedByTag.keys()]);
    return Array.from(allTags)
      .map((tag) => ({
        tag,
        budgeted: budgetedByTag.get(tag) || 0,
        observed: observedByTag.get(tag) || 0,
        delta: (observedByTag.get(tag) || 0) - (budgetedByTag.get(tag) || 0)
      }))
      .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta));
  });

  // User participants (non-pool)
  let userParticipants = $derived($participants.filter((p) => p.account_type === 'user'));

  const projectId = $derived(parseInt($page.params.id ?? ''));

  async function loadData() {
    loading = true;
    errorKey = '';
    try {
      const [budgetData, overridesData] = await Promise.all([
        getBudget(projectId, selectedParticipantId),
        getBudgetOverrides(projectId)
      ]);
      budget = budgetData;
      overrides = overridesData;
    } catch (e) {
      errorKey = getErrorKey(e, 'common.error');
    } finally {
      loading = false;
    }
  }

  // Sync preferences
  $effect(() => {
    const prefs = $preferences;
    payFrequency = (prefs as UserPreferences).budget_pay_frequency || 'biweekly';
    hoursPerWeek = (prefs as UserPreferences).budget_hours_per_week || 40;
  });

  // Reload when participant filter changes
  $effect(() => {
    if (projectId) {
      // Access selectedParticipantId to track it
      const _pid = selectedParticipantId;
      loadData();
    }
  });

  async function savePreferences() {
    try {
      const updated = await updatePreferences({
        budget_pay_frequency: payFrequency,
        budget_hours_per_week: hoursPerWeek
      });
      preferences.setAll(updated);
    } catch {
      // Silently fail - preferences are local anyway
    }
  }

  async function handleAddOverride() {
    if (!overrideForm.name.trim()) return;
    savingOverride = true;
    try {
      await createBudgetOverride(projectId, overrideForm);
      showOverrideForm = false;
      overrideForm = {
        name: '',
        yearly_amount: 0,
        override_type: 'add',
        tag: null,
        participant_id: null,
        linked_payment_id: null
      };
      await loadData();
    } catch (e) {
      errorKey = getErrorKey(e, 'common.error');
    } finally {
      savingOverride = false;
    }
  }

  async function handleDeleteOverride(overrideId: number) {
    try {
      await deleteBudgetOverride(projectId, overrideId);
      await loadData();
    } catch (e) {
      errorKey = getErrorKey(e, 'common.error');
    }
  }

  onMount(() => {
    loadData();
  });
</script>

<div class="budget-page">
  <!-- Configuration Bar -->
  <div class="config-bar">
    <div class="config-row">
      <div class="config-group">
        <label for="scope-select">{$_('budget.scope')}</label>
        <select id="scope-select" bind:value={selectedParticipantId} onchange={() => loadData()}>
          <option value={undefined}>{$_('budget.allAccounts')}</option>
          {#each userParticipants as participant}
            <option value={participant.id}>{participant.name}</option>
          {/each}
        </select>
      </div>

      <div class="config-group">
        <label for="view-select">{$_('budget.view')}</label>
        <div class="view-toggle">
          <button class:active={viewMode === 'budgeted'} onclick={() => (viewMode = 'budgeted')}>
            {$_('budget.budgeted')}
          </button>
          <button class:active={viewMode === 'observed'} onclick={() => (viewMode = 'observed')}>
            {$_('budget.observed')}
          </button>
          <button
            class:active={viewMode === 'comparison'}
            onclick={() => (viewMode = 'comparison')}
          >
            {$_('budget.delta')}
          </button>
        </div>
      </div>
    </div>

    <div class="config-row">
      <div class="config-group">
        <label for="pay-freq">{$_('budget.payFrequency')}</label>
        <select id="pay-freq" bind:value={payFrequency} onchange={() => savePreferences()}>
          <option value="weekly">{$_('budget.weekly')}</option>
          <option value="biweekly">{$_('budget.biweekly')}</option>
          <option value="semimonthly">{$_('budget.semimonthly')}</option>
          <option value="monthly">{$_('budget.monthly')}</option>
        </select>
      </div>

      <div class="config-group">
        <label for="hours-week">{$_('budget.hoursPerWeek')}</label>
        <input
          id="hours-week"
          type="number"
          bind:value={hoursPerWeek}
          min="1"
          max="168"
          step="0.5"
          onchange={() => savePreferences()}
          class="hours-input"
        />
      </div>
    </div>
  </div>

  {#if loading}
    <div class="loading">
      <Loader size={24} class="spin-animation" />
      <span>{$_('common.loading')}</span>
    </div>
  {:else if errorKey}
    <div class="error">
      <p>{$_(errorKey)}</p>
    </div>
  {:else if !budget}
    <div class="empty">
      <p>{$_('budget.noData')}</p>
    </div>
  {:else if viewMode === 'comparison'}
    <!-- Comparison View -->
    <div class="budget-table comparison-table">
      <div class="table-header">
        <div class="col-tag">{$_('budget.overrideTag')}</div>
        <div class="col-amount">{$_('budget.budgetedAmount')}</div>
        <div class="col-amount">{$_('budget.observedAmount')}</div>
        <div class="col-amount">{$_('budget.difference')}</div>
      </div>

      {#each comparisonData as row}
        <div
          class="table-row"
          class:over-budget={row.delta < -1}
          class:under-budget={row.delta > 1}
        >
          <div class="col-tag">{row.tag}</div>
          <div class="col-amount">{formatCurrency(calculateTimeAmount(row.budgeted, 'month'))}</div>
          <div class="col-amount">{formatCurrency(calculateTimeAmount(row.observed, 'month'))}</div>
          <div
            class="col-amount delta"
            class:negative={row.delta < 0}
            class:positive={row.delta > 0}
          >
            {formatCurrency(calculateTimeAmount(row.delta, 'month'))}
            {#if Math.abs(row.delta) > 1}
              <span class="delta-label">
                {row.delta < 0 ? $_('budget.overBudget') : $_('budget.underBudget')}
              </span>
            {/if}
          </div>
        </div>
      {/each}

      <!-- Totals -->
      <div class="table-row totals-row">
        <div class="col-tag"><strong>{$_('budget.net')}</strong></div>
        <div class="col-amount">
          <strong>{formatCurrency(calculateTimeAmount(budget.budgeted.net_yearly, 'month'))}</strong
          >
        </div>
        <div class="col-amount">
          <strong>{formatCurrency(calculateTimeAmount(budget.observed.net_yearly, 'month'))}</strong
          >
        </div>
        <div
          class="col-amount delta"
          class:negative={budget.observed.net_yearly - budget.budgeted.net_yearly < 0}
          class:positive={budget.observed.net_yearly - budget.budgeted.net_yearly > 0}
        >
          <strong
            >{formatCurrency(
              calculateTimeAmount(budget.observed.net_yearly - budget.budgeted.net_yearly, 'month')
            )}</strong
          >
        </div>
      </div>
    </div>
  {:else}
    <!-- Budget Table (Budgeted or Observed) -->
    <div class="budget-table">
      <div class="table-header">
        <div class="col-category">&nbsp;</div>
        {#each timeColumns as col}
          <div class="col-amount">{getColumnLabel(col)}</div>
        {/each}
      </div>

      <!-- Income Section -->
      {#if incomeItems.length > 0}
        <div
          class="section-header"
          onclick={() => toggleSection('income')}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === 'Enter' && toggleSection('income')}
        >
          <div class="col-category">
            {#if expandedSections.has('income')}
              <ChevronDown size={16} />
            {:else}
              <ChevronRight size={16} />
            {/if}
            <strong>{$_('budget.income')}</strong>
          </div>
          {#each timeColumns as col}
            <div class="col-amount income-amount">
              {formatCurrency(calculateTimeAmount(currentSummary?.total_income_yearly ?? 0, col))}
            </div>
          {/each}
        </div>

        {#if expandedSections.has('income')}
          {#each [...incomeByTag] as [tag, items]}
            {#each items as item}
              <div class="table-row item-row">
                <div class="col-category item-name">
                  {#if item.tag}
                    <span class="tag-badge">{item.tag}</span>
                  {/if}
                  {item.name}
                </div>
                {#each timeColumns as col}
                  <div class="col-amount">
                    {formatCurrency(calculateTimeAmount(item.yearly_amount, col))}
                  </div>
                {/each}
              </div>
            {/each}
          {/each}
        {/if}
      {/if}

      <!-- Expenses Section -->
      {#if expenseItems.length > 0}
        <div
          class="section-header"
          onclick={() => toggleSection('expenses')}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === 'Enter' && toggleSection('expenses')}
        >
          <div class="col-category">
            {#if expandedSections.has('expenses')}
              <ChevronDown size={16} />
            {:else}
              <ChevronRight size={16} />
            {/if}
            <strong>{$_('budget.expenses')}</strong>
          </div>
          {#each timeColumns as col}
            <div class="col-amount expense-amount">
              {formatCurrency(calculateTimeAmount(currentSummary?.total_expenses_yearly ?? 0, col))}
            </div>
          {/each}
        </div>

        {#if expandedSections.has('expenses')}
          {#each [...expenseByTag] as [tag, items]}
            {#each items as item}
              <div class="table-row item-row">
                <div class="col-category item-name">
                  {#if item.tag}
                    <span class="tag-badge">{item.tag}</span>
                  {/if}
                  {item.name}
                </div>
                {#each timeColumns as col}
                  <div class="col-amount">
                    {formatCurrency(calculateTimeAmount(item.yearly_amount, col))}
                  </div>
                {/each}
              </div>
            {/each}
          {/each}
        {/if}
      {/if}

      <!-- Net Row -->
      <div class="table-row net-row">
        <div class="col-category"><strong>{$_('budget.net')}</strong></div>
        {#each timeColumns as col}
          <div
            class="col-amount"
            class:positive={(currentSummary?.net_yearly ?? 0) > 0}
            class:negative={(currentSummary?.net_yearly ?? 0) < 0}
          >
            <strong
              >{formatCurrency(calculateTimeAmount(currentSummary?.net_yearly ?? 0, col))}</strong
            >
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Budget Overrides Section -->
  <div class="overrides-section">
    <div class="overrides-header">
      <h3>{$_('budget.overrides')}</h3>
      <button class="btn-add" onclick={() => (showOverrideForm = !showOverrideForm)}>
        <Plus size={16} />
        {$_('budget.addOverride')}
      </button>
    </div>

    {#if showOverrideForm}
      <form
        class="override-form"
        onsubmit={(e) => {
          e.preventDefault();
          handleAddOverride();
        }}
      >
        <div class="form-row">
          <div class="form-group">
            <label for="override-name">{$_('budget.overrideName')}</label>
            <input id="override-name" type="text" bind:value={overrideForm.name} required />
          </div>
          <div class="form-group">
            <label for="override-type">{$_('budget.overrideType')}</label>
            <select id="override-type" bind:value={overrideForm.override_type}>
              <option value="add">{$_('budget.overrideAdd')}</option>
              <option value="adjust">{$_('budget.overrideAdjust')}</option>
              <option value="exclude">{$_('budget.overrideExclude')}</option>
            </select>
          </div>
        </div>
        <div class="form-row">
          <div class="form-group">
            <label for="override-amount">{$_('budget.overrideAmount')}</label>
            <input
              id="override-amount"
              type="number"
              step="0.01"
              bind:value={overrideForm.yearly_amount}
            />
          </div>
          <div class="form-group">
            <label for="override-tag">{$_('budget.overrideTag')}</label>
            <input
              id="override-tag"
              type="text"
              bind:value={overrideForm.tag}
              list="available-tags"
            />
            {#if budget?.available_tags.length}
              <datalist id="available-tags">
                {#each budget.available_tags as tag}
                  <option value={tag}></option>
                {/each}
              </datalist>
            {/if}
          </div>
        </div>
        <div class="form-actions">
          <button type="submit" class="btn-primary" disabled={savingOverride}>
            {savingOverride ? $_('common.saving') : $_('common.save')}
          </button>
          <button type="button" class="btn-secondary" onclick={() => (showOverrideForm = false)}>
            {$_('common.cancel')}
          </button>
        </div>
      </form>
    {/if}

    {#if overrides.length === 0}
      <p class="empty-text">{$_('budget.noOverrides')}</p>
    {:else}
      <div class="overrides-list">
        {#each overrides as override}
          <div class="override-item">
            <div class="override-info">
              <span
                class="override-type-badge"
                class:add={override.override_type === 'add'}
                class:adjust={override.override_type === 'adjust'}
                class:exclude={override.override_type === 'exclude'}
              >
                {$_(
                  `budget.override${override.override_type.charAt(0).toUpperCase() + override.override_type.slice(1)}`
                )}
              </span>
              <span class="override-name">{override.name}</span>
              {#if override.tag}
                <span class="tag-badge">{override.tag}</span>
              {/if}
            </div>
            <div class="override-amount">
              {formatCurrency(override.yearly_amount)}/yr
            </div>
            <button
              class="btn-icon"
              onclick={() => handleDeleteOverride(override.id)}
              title={$_('common.delete')}
            >
              <Trash2 size={16} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .budget-page {
    max-width: 100%;
    overflow-x: auto;
  }

  /* Configuration Bar */
  .config-bar {
    background: rgba(255, 255, 255, 0.8);
    border-radius: 12px;
    padding: 1rem;
    margin-bottom: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .config-row {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
    align-items: end;
  }

  .config-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .config-group label {
    font-size: 0.75rem;
    color: #666;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .config-group select,
  .config-group input {
    padding: 0.4rem 0.6rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.85rem;
    background: white;
  }

  .hours-input {
    width: 80px;
  }

  .view-toggle {
    display: flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid #ddd;
  }

  .view-toggle button {
    padding: 0.4rem 0.8rem;
    border: none;
    background: white;
    font-size: 0.85rem;
    cursor: pointer;
    color: #666;
    transition: all 0.15s;
  }

  .view-toggle button:not(:last-child) {
    border-right: 1px solid #ddd;
  }

  .view-toggle button.active {
    background: var(--accent, #7b61ff);
    color: white;
  }

  /* Loading/Error/Empty States */
  .loading {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 0.5rem;
    min-height: 200px;
    color: #666;
  }

  .error {
    text-align: center;
    padding: 2rem;
    color: #c00;
  }

  .empty {
    text-align: center;
    padding: 3rem;
    color: #999;
  }

  /* Budget Table */
  .budget-table {
    background: rgba(255, 255, 255, 0.8);
    border-radius: 12px;
    overflow: hidden;
    margin-bottom: 1.5rem;
  }

  .table-header {
    display: grid;
    grid-template-columns: 1.5fr repeat(5, 1fr);
    padding: 0.6rem 1rem;
    background: rgba(0, 0, 0, 0.03);
    border-bottom: 1px solid #eee;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    color: #888;
    letter-spacing: 0.05em;
  }

  .section-header {
    display: grid;
    grid-template-columns: 1.5fr repeat(5, 1fr);
    padding: 0.6rem 1rem;
    cursor: pointer;
    border-bottom: 1px solid #f0f0f0;
    transition: background 0.15s;
  }

  .section-header:hover {
    background: rgba(0, 0, 0, 0.02);
  }

  .table-row {
    display: grid;
    grid-template-columns: 1.5fr repeat(5, 1fr);
    padding: 0.5rem 1rem;
    border-bottom: 1px solid #f5f5f5;
  }

  .item-row {
    padding-left: 2rem;
  }

  .net-row {
    border-top: 2px solid #ddd;
    background: rgba(0, 0, 0, 0.02);
  }

  .col-category {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .col-amount {
    text-align: right;
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }

  .income-amount {
    color: #2a9d8f;
  }

  .expense-amount {
    color: #e76f51;
  }

  .positive {
    color: #2a9d8f;
  }

  .negative {
    color: #e76f51;
  }

  .item-name {
    font-size: 0.85rem;
    color: #555;
  }

  .tag-badge {
    display: inline-block;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: var(--accent, #7b61ff);
    color: white;
    font-size: 0.7rem;
    font-weight: 500;
  }

  /* Comparison Table */
  .comparison-table .table-header {
    grid-template-columns: 1.5fr repeat(3, 1fr);
  }

  .comparison-table .table-row {
    grid-template-columns: 1.5fr repeat(3, 1fr);
  }

  .comparison-table .totals-row {
    border-top: 2px solid #ddd;
    background: rgba(0, 0, 0, 0.02);
  }

  .col-tag {
    font-size: 0.9rem;
  }

  .delta {
    font-weight: 500;
  }

  .delta-label {
    display: block;
    font-size: 0.7rem;
    font-weight: 400;
  }

  .over-budget {
    background: rgba(231, 111, 81, 0.05);
  }

  .under-budget {
    background: rgba(42, 157, 143, 0.05);
  }

  /* Overrides Section */
  .overrides-section {
    background: rgba(255, 255, 255, 0.8);
    border-radius: 12px;
    padding: 1rem;
  }

  .overrides-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .overrides-header h3 {
    margin: 0;
    font-size: 1rem;
  }

  .btn-add {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.4rem 0.8rem;
    border: 1px solid var(--accent, #7b61ff);
    background: transparent;
    color: var(--accent, #7b61ff);
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-add:hover {
    background: var(--accent, #7b61ff);
    color: white;
  }

  /* Override Form */
  .override-form {
    background: #f9f9fc;
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
    border: 1px solid #eee;
  }

  .form-row {
    display: flex;
    gap: 1rem;
    margin-bottom: 0.75rem;
    flex-wrap: wrap;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    flex: 1;
    min-width: 150px;
  }

  .form-group label {
    font-size: 0.75rem;
    color: #666;
    font-weight: 500;
  }

  .form-group input,
  .form-group select {
    padding: 0.4rem 0.6rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.85rem;
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .btn-primary {
    padding: 0.4rem 1rem;
    background: var(--accent, #7b61ff);
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
  }

  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-secondary {
    padding: 0.4rem 1rem;
    background: transparent;
    color: #666;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
  }

  /* Overrides List */
  .overrides-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .override-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0.75rem;
    background: #f9f9fc;
    border-radius: 6px;
    border: 1px solid #eee;
  }

  .override-info {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .override-type-badge {
    display: inline-block;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .override-type-badge.add {
    background: #d4edda;
    color: #155724;
  }

  .override-type-badge.adjust {
    background: #fff3cd;
    color: #856404;
  }

  .override-type-badge.exclude {
    background: #f8d7da;
    color: #721c24;
  }

  .override-name {
    font-size: 0.9rem;
  }

  .override-amount {
    font-size: 0.85rem;
    color: #666;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .btn-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    background: transparent;
    color: #999;
    cursor: pointer;
    border-radius: 6px;
    transition: all 0.15s;
  }

  .btn-icon:hover {
    background: #fee;
    color: #c00;
  }

  .empty-text {
    color: #999;
    font-size: 0.85rem;
    text-align: center;
    padding: 1rem;
  }

  /* Spin animation for loader */
  :global(.spin-animation) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* Responsive */
  @media (max-width: 768px) {
    .table-header,
    .section-header,
    .table-row {
      grid-template-columns: 1fr repeat(3, 1fr);
    }

    /* Hide semester and hour columns on mobile */
    .table-header > :nth-child(3),
    .section-header > :nth-child(3),
    .table-row > :nth-child(3),
    .table-header > :nth-child(6),
    .section-header > :nth-child(6),
    .table-row > :nth-child(6) {
      display: none;
    }

    .config-row {
      flex-direction: column;
      gap: 0.75rem;
    }
  }
</style>
