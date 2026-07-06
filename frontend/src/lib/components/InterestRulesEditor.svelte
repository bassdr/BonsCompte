<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Plus from '@lucide/svelte/icons/plus';
  import { _ } from '$lib/i18n';
  import {
    getInterestRules,
    createInterestRule,
    updateInterestRule,
    deleteInterestRule,
    type AccountInterestRule,
    type InterestRuleInput
  } from '$lib/api';
  import { getErrorKey } from '$lib/errors';
  import { getLocalDateString } from '$lib/format/date';
  import DateInput from '$lib/components/DateInput.svelte';

  let {
    projectId,
    poolId,
    canEdit
  }: {
    projectId: number;
    poolId: number;
    canEdit: boolean;
  } = $props();

  let rules = $state<AccountInterestRule[]>([]);
  let loading = $state(true);
  let errorKey = $state('');

  // Form state (add or edit)
  let showForm = $state(false);
  let editingRuleId = $state<number | null>(null);
  let formStartDate = $state(getLocalDateString());
  let formEndDate = $state('');
  let formRatePercent = $state('');
  let formPeriod = $state<'monthly' | 'semiannual' | 'annual'>('annual');
  let formOnlyWhenNegative = $state(false);
  let formMonthlyFee = $state('');
  let formFeeThreshold = $state('');
  let submitting = $state(false);

  $effect(() => {
    if (projectId && poolId) {
      loadRules();
    }
  });

  async function loadRules() {
    loading = true;
    errorKey = '';
    try {
      rules = await getInterestRules(projectId, poolId);
    } catch (e) {
      errorKey = getErrorKey(e, 'interestRules.failedToLoad');
    } finally {
      loading = false;
    }
  }

  function resetForm() {
    showForm = false;
    editingRuleId = null;
    formStartDate = getLocalDateString();
    formEndDate = '';
    formRatePercent = '';
    formPeriod = 'annual';
    formOnlyWhenNegative = false;
    formMonthlyFee = '';
    formFeeThreshold = '';
  }

  function startEdit(rule: AccountInterestRule) {
    editingRuleId = rule.id;
    formStartDate = rule.start_date;
    formEndDate = rule.end_date ?? '';
    formRatePercent = String(rule.annual_rate * 100);
    formPeriod = rule.period;
    formOnlyWhenNegative = rule.only_when_negative;
    formMonthlyFee = rule.monthly_fee !== null ? String(rule.monthly_fee) : '';
    formFeeThreshold = rule.fee_threshold !== null ? String(rule.fee_threshold) : '';
    showForm = true;
  }

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    const ratePercent = parseFloat(formRatePercent);
    if (isNaN(ratePercent)) {
      errorKey = 'interestRules.invalidRate';
      return;
    }
    const data: InterestRuleInput = {
      start_date: formStartDate,
      end_date: formEndDate || null,
      annual_rate: ratePercent / 100,
      period: formPeriod,
      only_when_negative: formOnlyWhenNegative,
      monthly_fee: formMonthlyFee ? parseFloat(formMonthlyFee) : null,
      fee_threshold: formFeeThreshold ? parseFloat(formFeeThreshold) : null
    };

    submitting = true;
    errorKey = '';
    try {
      if (editingRuleId !== null) {
        await updateInterestRule(projectId, poolId, editingRuleId, data);
      } else {
        await createInterestRule(projectId, poolId, data);
      }
      resetForm();
      await loadRules();
    } catch (e) {
      errorKey = getErrorKey(e, 'interestRules.failedToSave');
    } finally {
      submitting = false;
    }
  }

  async function handleDelete(ruleId: number) {
    if (!confirm($_('interestRules.confirmDelete'))) return;
    errorKey = '';
    try {
      await deleteInterestRule(projectId, poolId, ruleId);
      await loadRules();
    } catch (e) {
      errorKey = getErrorKey(e, 'interestRules.failedToDelete');
    }
  }

  function formatRate(rule: AccountInterestRule): string {
    const pct = (rule.annual_rate * 100).toFixed(2).replace(/\.?0+$/, '');
    return `${pct}%`;
  }
</script>

<div class="interest-rules">
  <h4>{$_('interestRules.title')}</h4>
  <p class="section-desc">{$_('interestRules.description')}</p>

  {#if errorKey}
    <div class="error">{$_(errorKey)}</div>
  {/if}

  {#if loading}
    <p class="muted">{$_('common.loading')}</p>
  {:else if rules.length === 0 && !showForm}
    <p class="muted">{$_('interestRules.noRules')}</p>
  {:else}
    <ul class="rules-list">
      {#each rules as rule (rule.id)}
        <li>
          <div class="rule-summary">
            <strong>{formatRate(rule)}</strong>
            <span>{$_(`interestRules.period.${rule.period}`)}</span>
            <span class="rule-dates">
              {rule.start_date}{rule.end_date ? ` → ${rule.end_date}` : ' →'}
            </span>
            {#if rule.only_when_negative}
              <span class="rule-badge">{$_('interestRules.onlyWhenNegative')}</span>
            {/if}
            {#if rule.monthly_fee !== null}
              <span class="rule-badge">
                {$_('interestRules.feeBadge', { values: { fee: rule.monthly_fee } })}
              </span>
            {/if}
          </div>
          {#if canEdit}
            <div class="rule-actions">
              <button class="icon-btn" onclick={() => startEdit(rule)} title={$_('common.edit')}>
                <Pencil size={16} />
              </button>
              <button
                class="icon-btn delete-btn"
                onclick={() => handleDelete(rule.id)}
                title={$_('common.delete')}
              >
                <Trash2 size={16} />
              </button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if canEdit && !showForm}
    <button class="btn-secondary add-rule-btn" onclick={() => (showForm = true)}>
      <Plus size={16} />
      {$_('interestRules.addRule')}
    </button>
  {/if}

  {#if showForm}
    <form class="rule-form" onsubmit={handleSubmit}>
      <div class="form-grid">
        <div class="field">
          <label for="rule-rate-{poolId}">{$_('interestRules.annualRate')}</label>
          <input
            id="rule-rate-{poolId}"
            type="number"
            step="any"
            required
            bind:value={formRatePercent}
            placeholder="5.25"
          />
        </div>
        <div class="field">
          <label for="rule-period-{poolId}">{$_('interestRules.periodLabel')}</label>
          <select id="rule-period-{poolId}" bind:value={formPeriod}>
            <option value="monthly">{$_('interestRules.period.monthly')}</option>
            <option value="semiannual">{$_('interestRules.period.semiannual')}</option>
            <option value="annual">{$_('interestRules.period.annual')}</option>
          </select>
        </div>
        <div class="field">
          <label for="rule-start-{poolId}">{$_('interestRules.startDate')}</label>
          <DateInput id="rule-start-{poolId}" bind:value={formStartDate} buttons={['today']} />
        </div>
        <div class="field">
          <label for="rule-end-{poolId}">{$_('interestRules.endDate')}</label>
          <DateInput id="rule-end-{poolId}" bind:value={formEndDate} buttons={['clear']} />
        </div>
        <div class="field">
          <label for="rule-fee-{poolId}">{$_('interestRules.monthlyFee')}</label>
          <input
            id="rule-fee-{poolId}"
            type="number"
            step="any"
            min="0"
            bind:value={formMonthlyFee}
            placeholder={$_('interestRules.optional')}
          />
        </div>
        <div class="field">
          <label for="rule-threshold-{poolId}">{$_('interestRules.feeThreshold')}</label>
          <input
            id="rule-threshold-{poolId}"
            type="number"
            step="any"
            bind:value={formFeeThreshold}
            placeholder={$_('interestRules.optional')}
          />
        </div>
      </div>
      <label class="checkbox-field">
        <input type="checkbox" bind:checked={formOnlyWhenNegative} />
        {$_('interestRules.onlyWhenNegativeLabel')}
      </label>
      <div class="form-actions">
        <button type="submit" class="btn-primary" disabled={submitting}>
          {submitting ? $_('common.saving') : $_('common.save')}
        </button>
        <button type="button" class="btn-secondary" onclick={resetForm}>
          {$_('common.cancel')}
        </button>
      </div>
    </form>
  {/if}
</div>

<style>
  .interest-rules {
    margin-top: 1.5rem;
    padding-top: 1rem;
    border-top: 1px solid #eee;
  }

  h4 {
    margin: 0 0 0.25rem 0;
    color: var(--accent, #7b61ff);
  }

  .section-desc {
    font-size: 0.85rem;
    color: #666;
    margin: 0 0 1rem 0;
  }

  .error {
    background: #fee;
    color: #c00;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .muted {
    color: #666;
    font-style: italic;
    font-size: 0.9rem;
  }

  .rules-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1rem 0;
  }

  .rules-list li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid #f0f0f0;
  }

  .rule-summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .rule-dates {
    color: #666;
    font-size: 0.85rem;
  }

  .rule-badge {
    background: #f0edff;
    color: var(--accent, #7b61ff);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    font-size: 0.75rem;
  }

  .rule-actions {
    display: flex;
    gap: 0.25rem;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.3rem;
    border-radius: 6px;
    color: #666;
  }

  .icon-btn:hover {
    background: #f0f0f0;
  }

  .delete-btn:hover {
    color: #dc3545;
  }

  .add-rule-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .rule-form {
    margin-top: 1rem;
    padding: 1rem;
    background: #fafafa;
    border-radius: 8px;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 0.75rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .field label {
    font-size: 0.85rem;
    font-weight: 500;
    color: #666;
  }

  .field input,
  .field select {
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.9rem;
  }

  .checkbox-field {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
    font-size: 0.9rem;
    color: #444;
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .btn-primary {
    background: var(--accent, #7b61ff);
    color: white;
    border: none;
    border-radius: 6px;
    padding: 0.5rem 1rem;
    cursor: pointer;
  }

  .btn-primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .btn-secondary {
    background: #f0f0f0;
    color: #444;
    border: none;
    border-radius: 6px;
    padding: 0.5rem 1rem;
    cursor: pointer;
  }
</style>
