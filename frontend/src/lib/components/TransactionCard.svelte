<script lang="ts">
  import { _ } from '$lib/i18n';
  import { formatCurrency } from '$lib/format/currency';
  import { formatDate } from '$lib/format/date';
  import { preferences } from '$lib/stores/preferences';
  import { SvelteDate } from 'svelte/reactivity';
  import Image from '@lucide/svelte/icons/image';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import CirclePlus from '@lucide/svelte/icons/circle-plus';

  interface Contribution {
    participant_id: number;
    participant_name: string;
    amount: number;
  }

  interface Props {
    // Required fields
    description: string;
    amount: number;
    date: string; // ISO format YYYY-MM-DD or datetime

    // Optional transaction metadata
    payerName?: string | null;
    payerId?: number | null;
    receiverName?: string | null;
    receiverAccountId?: number | null;

    // Status flags
    isFinal?: boolean;
    affectsBalance?: boolean;
    affectsPayerExpectation?: boolean;
    affectsReceiverExpectation?: boolean;

    // Recurrence
    isRecurring?: boolean;
    recurrenceType?: string | null;
    recurrenceInterval?: number | null;
    recurrenceEndDate?: string | null;

    // Contributions/splits
    contributions?: Contribution[];

    // Receipt image
    receiptImage?: string | null;

    // Tags for categorization
    tags?: string[] | null;

    // Optional action handlers (if provided, buttons will show)
    onEdit?: () => void;
    onDelete?: () => void;
    onViewReceipt?: () => void;
    onPromoteToBudget?: () => void;

    // Styling
    class?: string;
  }

  let {
    description,
    amount,
    date,
    payerName = null,
    payerId = null,
    receiverName = null,
    receiverAccountId = null,
    isFinal = true,
    affectsBalance = true,
    affectsPayerExpectation = false,
    affectsReceiverExpectation = false,
    isRecurring = false,
    recurrenceType = null,
    recurrenceInterval = null,
    recurrenceEndDate = null,
    contributions = [],
    receiptImage = null,
    tags = null,
    onEdit,
    onDelete,
    onViewReceipt,
    onPromoteToBudget,
    class: className = ''
  }: Props = $props();

  // Determine transaction type
  let transactionType = $derived.by(() => {
    if (!affectsBalance) return 'rule';
    if (payerId === null && receiverAccountId !== null) return 'inflow';
    if (receiverAccountId !== null) return 'transfer';
    return 'expense';
  });

  // Format date (strip time portion if present)
  // Include $preferences in the expression to make it reactive to preference changes
  let displayDate = $derived.by(() => {
    // Access preferences to create reactive dependency
    const _ = $preferences.date_format;
    return formatDate(date.split('T')[0]);
  });

  // Format recurrence end date reactively
  let displayRecurrenceEndDate = $derived.by(() => {
    if (!recurrenceEndDate) return '';
    // Access preferences to create reactive dependency
    const _ = $preferences.date_format;
    return formatDate(recurrenceEndDate.split('T')[0]);
  });

  // Format recurrence info
  function formatRecurrence(): string {
    if (!isRecurring || !recurrenceType) return '';

    const interval = recurrenceInterval || 1;
    let typeKey = '';

    switch (recurrenceType) {
      case 'daily':
        typeKey =
          interval === 1 ? 'transactions.recurrence.daily' : 'transactions.recurrence.everyNDays';
        break;
      case 'weekly':
        typeKey =
          interval === 1 ? 'transactions.recurrence.weekly' : 'transactions.recurrence.everyNWeeks';
        break;
      case 'monthly':
        typeKey =
          interval === 1
            ? 'transactions.recurrence.monthly'
            : 'transactions.recurrence.everyNMonths';
        break;
      case 'yearly':
        typeKey =
          interval === 1 ? 'transactions.recurrence.yearly' : 'transactions.recurrence.everyNYears';
        break;
      default:
        return recurrenceType;
    }

    return $_(`${typeKey}`, { values: { n: interval } });
  }

  // Check if date is in the future
  function isFutureDate(dateStr: string): boolean {
    const today = new SvelteDate();
    today.setHours(0, 0, 0, 0);
    const checkDate = new SvelteDate(dateStr.split('T')[0]);
    return checkDate > today;
  }
</script>

<div class="transaction-card {className}">
  <!-- Row 1: Description + badges | Amount + actions -->
  <div class="card-header">
    <div class="card-title">
      <span class="description">{description}</span>
      <span class="badges">
        {#if !affectsBalance}
          <span class="badge rule">{$_('transactions.typeRule')}</span>
        {:else}
          {#if affectsPayerExpectation}
            <span class="badge approved">{$_('transactions.statusApproved')}</span>
          {/if}
          {#if affectsReceiverExpectation}
            <span class="badge earmarked">{$_('transactions.statusEarmarked')}</span>
          {/if}
        {/if}
        {#if !isFinal}
          <span class="badge draft">{$_('transactions.statusDraft')}</span>
        {/if}
      </span>
    </div>
    <div class="card-actions">
      <span class="amount">{formatCurrency(amount)}</span>
      {#if receiptImage && onViewReceipt}
        <button class="icon-btn" onclick={onViewReceipt} title={$_('transactions.viewReceipt')}>
          <Image size={18} />
        </button>
      {/if}
      {#if onEdit}
        <button class="icon-btn edit-btn" onclick={onEdit} title={$_('common.edit')}>
          <Pencil size={18} />
        </button>
      {/if}
      {#if onDelete}
        <button class="icon-btn delete-btn" onclick={onDelete} title={$_('common.delete')}>
          <Trash2 size={18} />
        </button>
      {/if}
      <!-- TODO: untested and put there to silent eslint errors -->
      {#if tags && onPromoteToBudget}
        <button
          class="icon-btn promote-btn"
          onclick={onPromoteToBudget}
          title={$_('budget.promoteToBudget')}
        >
          <CirclePlus size={18} />
        </button>
      {/if}
    </div>
  </div>

  <!-- Row 2: Transaction type, payer/receiver, date, recurrence -->
  <div class="card-meta">
    {#if transactionType === 'rule'}
      <span class="type-badge rule-badge">{$_('transactions.typeRule')}</span>
      {#if receiverName}
        {$_('transactions.appliesTo')} {receiverName}
      {/if}
    {:else if transactionType === 'inflow'}
      <span class="type-badge inflow-badge">{$_('transactions.externalInflow')}</span>
      {$_('transactions.externalIndicator')} &rarr; {receiverName ?? $_('common.unknown')}
    {:else if transactionType === 'transfer'}
      <span class="type-badge transfer-badge">{$_('transactions.transfer')}</span>
      {payerName ?? $_('common.unknown')} &rarr; {receiverName ?? $_('common.unknown')}
    {:else}
      {$_('transactions.paidBy')} {payerName ?? $_('common.unknown')}
    {/if}

    <span class="separator">&bull;</span>

    {#if isRecurring && recurrenceEndDate}
      {$_('transactions.dateRangeFromTo', {
        values: { startDate: displayDate, endDate: displayRecurrenceEndDate }
      })}
    {:else if isFutureDate(date)}
      {$_('transactions.startingFrom', { values: { date: displayDate } })}
    {:else}
      {$_('transactions.occurringOn', { values: { date: displayDate } })}
    {/if}

    {#if isRecurring}
      <span class="recurrence-badge">{formatRecurrence()}</span>
    {/if}
  </div>

  <!-- Row 3: Contributions/splits -->
  {#if contributions.length > 0}
    <div class="card-splits">
      {#each contributions as c (c.participant_id)}
        <span class="chip">
          {c.participant_name}: {formatCurrency(c.amount)}
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .transaction-card {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.75rem;
    background: #fafafa;
    border-radius: 6px;
    border-left: 3px solid var(--accent, #7b61ff);
  }

  /* Row 1: Header */
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }

  .card-title {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .description {
    font-weight: 500;
    font-size: 0.95rem;
  }

  .badges {
    display: inline-flex;
    gap: 0.25rem;
  }

  .badge {
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge.draft {
    background: #f0ad4e;
    color: white;
  }

  .badge.approved {
    background: #28a745;
    color: white;
  }

  .badge.rule {
    background: var(--accent, #7b61ff);
    color: white;
  }

  .badge.earmarked {
    background: #17a2b8;
    color: white;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .amount {
    font-weight: 600;
    color: var(--accent, #7b61ff);
    font-size: 0.95rem;
    white-space: nowrap;
  }

  .icon-btn {
    padding: 0.25rem;
    background: transparent;
    border: none;
    cursor: pointer;
    color: #666;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .icon-btn:hover {
    background: #e0e0e0;
  }

  .edit-btn:hover {
    color: var(--accent, #7b61ff);
  }

  .delete-btn:hover {
    color: #dc3545;
  }

  /* Row 2: Meta */
  .card-meta {
    font-size: 0.85rem;
    color: #666;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .separator {
    color: #ccc;
  }

  .type-badge {
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    color: white;
  }

  .transfer-badge {
    background: linear-gradient(135deg, #4caf50, #2e7d32);
  }

  .inflow-badge {
    background: linear-gradient(135deg, #1976d2, #0d47a1);
  }

  .rule-badge {
    background: var(--accent, #7b61ff);
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

  /* Row 3: Splits */
  .card-splits {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding-top: 0.25rem;
  }

  .chip {
    background: #f0f0f0;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.8rem;
    color: #555;
  }

  /* Responsive */
  @media (max-width: 480px) {
    .card-header {
      flex-direction: column;
      gap: 0.5rem;
    }

    .card-actions {
      width: 100%;
      justify-content: space-between;
    }

    .card-meta {
      font-size: 0.8rem;
    }
  }
</style>
