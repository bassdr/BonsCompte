<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { getPayments, deletePayment, type PaymentWithContributions } from '$lib/api';
  import { participants, canEdit } from '$lib/stores/project';
  import { _ } from '$lib/i18n';
  import { formatCurrency } from '$lib/format/currency';
  import { formatDate } from '$lib/format/date';
  import { SvelteDate } from 'svelte/reactivity';

  let payments: PaymentWithContributions[] = $state([]);
  let loading = $state(true);
  let error = $state('');

  // Filter state
  let searchText = $state('');
  let filterPayerId = $state<number | null>(null);
  let filterContributorId = $state<number | null>(null);
  let filterPaymentType = $state<string>('');
  let filterRecurring = $state<string>('');
  let filterStatus = $state<string>('');
  let filterDateFrom = $state('');
  let filterDateTo = $state('');

  // Pagination state
  let paymentsToShow = $state(30);
  const PAYMENTS_PER_PAGE = 30;

  // Image modal state
  let showImageModal = $state(false);
  let modalImage = $state<string | null>(null);

  // Load payments
  $effect(() => {
    const projectId = parseInt($page.params.id ?? '');
    if (!isNaN(projectId)) {
      loadPayments(projectId);
    }
  });

  async function loadPayments(projectId: number) {
    loading = true;
    error = '';
    try {
      payments = await getPayments(projectId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load payments';
    } finally {
      loading = false;
    }
  }

  // Apply all filters
  let filteredPayments = $derived.by(() => {
    let result = payments;

    // Text search
    if (searchText.trim()) {
      const search = searchText.toLowerCase();
      result = result.filter((p) => p.description.toLowerCase().includes(search));
    }

    // Filter by payer
    if (filterPayerId !== null) {
      result = result.filter((p) => p.payer_id === filterPayerId);
    }

    // Filter by contributor
    if (filterContributorId !== null) {
      result = result.filter((p) =>
        p.contributions.some((c) => c.participant_id === filterContributorId)
      );
    }

    // Filter by payment type
    if (filterPaymentType) {
      if (filterPaymentType === 'expense') {
        result = result.filter((p) => p.payer_id !== null && p.receiver_account_id === null);
      } else if (filterPaymentType === 'transfer') {
        result = result.filter((p) => p.payer_id !== null && p.receiver_account_id !== null);
      } else if (filterPaymentType === 'inflow') {
        result = result.filter((p) => p.payer_id === null && p.receiver_account_id !== null);
      }
    }

    // Filter by recurring
    if (filterRecurring) {
      if (filterRecurring === 'yes') {
        result = result.filter((p) => p.is_recurring);
      } else if (filterRecurring === 'no') {
        result = result.filter((p) => !p.is_recurring);
      }
    }

    // Filter by status
    if (filterStatus) {
      result = result.filter((p) => p.status === filterStatus);
    }

    // Filter by date range
    if (filterDateFrom) {
      result = result.filter((p) => {
        const paymentDate = p.payment_date.split('T')[0];
        return paymentDate >= filterDateFrom;
      });
    }
    if (filterDateTo) {
      result = result.filter((p) => {
        const paymentDate = p.payment_date.split('T')[0];
        return paymentDate <= filterDateTo;
      });
    }

    return result;
  });

  // Active filter count
  let activeFilterCount = $derived(
    (searchText.trim() ? 1 : 0) +
      (filterPayerId !== null ? 1 : 0) +
      (filterContributorId !== null ? 1 : 0) +
      (filterPaymentType !== '' ? 1 : 0) +
      (filterRecurring !== '' ? 1 : 0) +
      (filterStatus !== '' ? 1 : 0) +
      (filterDateFrom !== '' ? 1 : 0) +
      (filterDateTo !== '' ? 1 : 0)
  );

  // Pagination
  let visiblePayments = $derived(filteredPayments.slice(0, paymentsToShow));
  let hasMorePayments = $derived(filteredPayments.length > paymentsToShow);

  function clearFilters() {
    searchText = '';
    filterPayerId = null;
    filterContributorId = null;
    filterPaymentType = '';
    filterRecurring = '';
    filterStatus = '';
    filterDateFrom = '';
    filterDateTo = '';
  }

  function loadMorePayments() {
    paymentsToShow += PAYMENTS_PER_PAGE;
  }

  // Get transaction mode for routing to edit page
  function getTransactionMode(p: PaymentWithContributions): string {
    if (p.payer_id === null && p.receiver_account_id !== null) return 'incoming';
    if (p.receiver_account_id !== null) return 'internal';
    return 'outgoing';
  }

  function editPayment(payment: PaymentWithContributions) {
    const mode = getTransactionMode(payment);
    goto(`/projects/${$page.params.id}/transactions/${mode}?edit=${payment.id}`);
  }

  async function handleDelete(paymentId: number) {
    if (!confirm($_('payments.deletePayment') + '?')) return;

    const projectId = parseInt($page.params.id ?? '');
    try {
      await deletePayment(projectId, paymentId);
      payments = payments.filter((p) => p.id !== paymentId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete payment';
    }
  }

  function isFutureDate(dateStr: string): boolean {
    const today = new SvelteDate();
    today.setHours(0, 0, 0, 0);
    const date = new SvelteDate(dateStr);
    return date > today;
  }

  function formatRecurrence(p: PaymentWithContributions): string {
    if (!p.is_recurring || !p.recurrence_type) return '';

    const interval = p.recurrence_interval ?? 1;
    const type = p.recurrence_type;

    if (interval === 1) {
      return $_(`payments.recurrence.${type}`);
    }

    // Map recurrence type to plural noun form for translation key
    const typeToNoun: Record<string, string> = {
      daily: 'Days',
      weekly: 'Weeks',
      monthly: 'Months',
      yearly: 'Years'
    };

    return $_(`payments.recurrence.everyN${typeToNoun[type] ?? type}`, {
      values: { n: interval }
    });
  }

  function openImageModal(image: string) {
    modalImage = image;
    showImageModal = true;
  }

  function closeImageModal() {
    showImageModal = false;
    modalImage = null;
  }
</script>

{#if error}
  <div class="error">{error}</div>
{/if}

<!-- Filters Section -->
<section class="card filters">
  <div class="filters-header">
    <h3>{$_('payments.filters')}</h3>
    {#if activeFilterCount > 0}
      <button type="button" class="clear-filters-btn" onclick={clearFilters}>
        {$_('common.clear')} ({activeFilterCount})
      </button>
    {/if}
  </div>

  <div class="filters-grid">
    <!-- Text Search -->
    <div class="filter-field">
      <label for="search-text">{$_('payments.searchDescription')}</label>
      <input
        id="search-text"
        type="text"
        bind:value={searchText}
        placeholder={$_('payments.searchPlaceholder')}
      />
    </div>

    <!-- Payer Filter -->
    <div class="filter-field">
      <label for="filter-payer">{$_('payments.filterByPayer')}</label>
      <select id="filter-payer" bind:value={filterPayerId}>
        <option value={null}>{$_('payments.allPayers')}</option>
        {#each $participants as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    <!-- Contributor Filter -->
    <div class="filter-field">
      <label for="filter-contributor">{$_('payments.filterByContributor')}</label>
      <select id="filter-contributor" bind:value={filterContributorId}>
        <option value={null}>{$_('payments.allContributors')}</option>
        {#each $participants as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    <!-- Payment Type Filter -->
    <div class="filter-field">
      <label for="filter-type">{$_('payments.filterByType')}</label>
      <select id="filter-type" bind:value={filterPaymentType}>
        <option value="">{$_('payments.allTypes')}</option>
        <option value="expense">{$_('payments.typeExpense')}</option>
        <option value="transfer">{$_('payments.typeTransfer')}</option>
        <option value="inflow">{$_('payments.typeInflow')}</option>
      </select>
    </div>

    <!-- Recurring Filter -->
    <div class="filter-field">
      <label for="filter-recurring">{$_('payments.filterByRecurring')}</label>
      <select id="filter-recurring" bind:value={filterRecurring}>
        <option value="">{$_('payments.allRecurrence')}</option>
        <option value="yes">{$_('payments.recurringOnly')}</option>
        <option value="no">{$_('payments.oneTimeOnly')}</option>
      </select>
    </div>

    <!-- Status Filter -->
    <div class="filter-field">
      <label for="filter-status">{$_('payments.filterByStatus')}</label>
      <select id="filter-status" bind:value={filterStatus}>
        <option value="">{$_('payments.allStatuses')}</option>
        <option value="final">{$_('payments.statusFinal')}</option>
        <option value="draft">{$_('payments.statusDraft')}</option>
      </select>
    </div>

    <!-- Date From -->
    <div class="filter-field">
      <label for="filter-date-from">{$_('payments.dateFrom')}</label>
      <input id="filter-date-from" type="date" bind:value={filterDateFrom} />
    </div>

    <!-- Date To -->
    <div class="filter-field">
      <label for="filter-date-to">{$_('payments.dateTo')}</label>
      <input id="filter-date-to" type="date" bind:value={filterDateTo} />
    </div>
  </div>

  {#if activeFilterCount > 0}
    <div class="filter-summary">
      {$_('payments.showingFiltered')}
      {filteredPayments.length} / {payments.length}
    </div>
  {/if}
</section>

<!-- Payments List -->
<section class="card">
  <h3>{$_('payments.recentPayments')}</h3>

  {#if loading}
    <p>{$_('common.loading')}</p>
  {:else if payments.length === 0}
    <p class="empty">{$_('payments.noPaymentsYet')}</p>
  {:else if filteredPayments.length === 0}
    <div class="empty-state">
      <p class="empty">{$_('payments.noMatchingPayments')}</p>
      <button type="button" class="clear-filters-btn" onclick={clearFilters}>
        {$_('common.clear')}
      </button>
    </div>
  {:else}
    <ul class="payments-list">
      {#each visiblePayments as p (p.id)}
        <li>
          <div class="payment-header">
            <div class="payment-title">
              <strong>{p.description}</strong>
              <div class="payment-icons">
                {#if p.status === 'draft'}
                  <span class="badge draft">{$_('payments.statusDraft')}</span>
                {/if}
                {#if p.is_recurring}
                  <span class="icon recurring" title={formatRecurrence(p)}>&#x27F3;</span>
                {/if}
                {#if p.receipt_image}
                  <button
                    class="icon-btn"
                    title={$_('payments.viewReceipt')}
                    onclick={() => openImageModal(p.receipt_image!)}>&#x1F9FE;</button
                  >
                {/if}
              </div>
            </div>
            <span class="amount-group">
              <span class="amount">{formatCurrency(p.amount)}</span>
              {#if $canEdit}
                <button
                  class="edit-btn"
                  onclick={() => editPayment(p)}
                  title={$_('payments.editPayment')}>&#x2699;</button
                >
                <button
                  class="delete-btn"
                  onclick={() => handleDelete(p.id)}
                  title={$_('payments.deletePayment')}
                >
                  &times;
                </button>
              {/if}
            </span>
          </div>
          <div class="payment-meta">
            {#if p.payer_id === null && p.receiver_account_id !== null}
              <!-- External inflow -->
              {@const receiver = $participants.find((pr) => pr.id === p.receiver_account_id)}
              <span class="inflow-badge">{$_('payments.externalInflow')}</span>
              {$_('payments.externalIndicator')} → {receiver?.name ?? $_('common.unknown')}
            {:else if p.receiver_account_id !== null}
              <!-- Internal transfer -->
              {@const receiver = $participants.find((pr) => pr.id === p.receiver_account_id)}
              <span class="transfer-badge">{$_('payments.transfer')}</span>
              {p.payer_name ?? $_('common.unknown')} → {receiver?.name ?? $_('common.unknown')}
            {:else}
              <!-- External expense -->
              {$_('payments.paidBy')}
              {p.payer_name ?? $_('common.unknown')}
            {/if}
            {#if p.is_recurring && p.recurrence_end_date}
              {$_('payments.dateRangeFromTo', {
                values: {
                  startDate: formatDate(p.payment_date),
                  endDate: formatDate(p.recurrence_end_date)
                }
              })}
            {:else if isFutureDate(p.payment_date)}
              {$_('payments.startingFrom', { values: { date: formatDate(p.payment_date) } })}
            {:else}
              {$_('payments.occurringOn', { values: { date: formatDate(p.payment_date) } })}
            {/if}
            {#if p.is_recurring}
              <span class="recurrence-badge">{formatRecurrence(p)}</span>
            {/if}
          </div>
          <div class="payment-splits">
            {#each p.contributions as c (c.participant_id)}
              <span class="chip">
                {c.participant_name}: {formatCurrency(c.amount)}
              </span>
            {/each}
          </div>
        </li>
      {/each}
    </ul>

    <!-- Load More Button -->
    {#if hasMorePayments}
      <div class="load-more-section">
        <button type="button" class="load-more-btn" onclick={loadMorePayments}>
          {$_('payments.loadMore')}
          ({paymentsToShow} / {filteredPayments.length})
        </button>
      </div>
    {/if}
  {/if}
</section>

<!-- Image Modal -->
{#if showImageModal && modalImage}
  <div
    class="modal-overlay"
    role="button"
    tabindex="0"
    onclick={closeImageModal}
    onkeydown={(e) => e.key === 'Escape' && closeImageModal()}
  >
    <div
      class="modal-content"
      role="button"
      tabindex="0"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <button class="modal-close" onclick={closeImageModal}>&times;</button>
      <img src={modalImage} alt="Receipt" />
    </div>
  </div>
{/if}

<style>
  .error {
    background: #fee;
    color: #c00;
    padding: 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .card {
    background: rgba(255, 255, 255, 0.9);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }

  h3 {
    margin: 0 0 1rem 0;
    color: var(--accent, #7b61ff);
  }

  /* Filters */
  .filters {
    margin-bottom: 1.5rem;
  }

  .filters-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .filters-header h3 {
    margin: 0;
  }

  .clear-filters-btn {
    background: #f0f0f0;
    border: none;
    border-radius: 6px;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 0.9rem;
    color: #666;
  }

  .clear-filters-btn:hover {
    background: #e0e0e0;
  }

  .filters-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
  }

  .filter-field {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .filter-field label {
    font-size: 0.85rem;
    font-weight: 500;
    color: #666;
  }

  .filter-field input,
  .filter-field select {
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 0.9rem;
  }

  .filter-summary {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #eee;
    font-size: 0.9rem;
    color: #666;
  }

  /* Payments List */
  .empty {
    color: #666;
    font-style: italic;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .payments-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .payments-list li {
    padding: 1rem;
    border-bottom: 1px solid #eee;
  }

  .payments-list li:last-child {
    border-bottom: none;
  }

  .payment-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 0.5rem;
  }

  .payment-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .payment-icons {
    display: flex;
    gap: 0.25rem;
  }

  .icon {
    font-size: 1rem;
    cursor: default;
  }

  .icon.recurring {
    color: var(--accent, #7b61ff);
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

  .icon-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    font-size: 1rem;
  }

  .amount-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .amount {
    font-weight: 600;
    color: var(--accent, #7b61ff);
    font-size: 1.1rem;
  }

  .edit-btn,
  .delete-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 1rem;
  }

  .edit-btn {
    color: #666;
  }

  .edit-btn:hover:not(:disabled) {
    background: #f0f0f0;
    color: var(--accent, #7b61ff);
  }

  .edit-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .delete-btn {
    color: #c00;
  }

  .delete-btn:hover {
    background: #fee;
  }

  .payment-meta {
    font-size: 0.85rem;
    color: #666;
    margin-bottom: 0.5rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
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

  .recurrence-badge {
    background: linear-gradient(135deg, #7b61ff 0%, #5a45cc 100%);
    color: white;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .payment-splits {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .chip {
    background: #f0f0f0;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.8rem;
  }

  /* Load More */
  .load-more-section {
    margin-top: 1rem;
    text-align: center;
  }

  .load-more-btn {
    padding: 0.75rem 1.5rem;
    background: #f0f0f0;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.9rem;
    color: #666;
  }

  .load-more-btn:hover {
    background: #e0e0e0;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-content {
    position: relative;
    max-width: 90vw;
    max-height: 90vh;
  }

  .modal-content img {
    max-width: 100%;
    max-height: 90vh;
    border-radius: 8px;
  }

  .modal-close {
    position: absolute;
    top: -40px;
    right: 0;
    background: none;
    border: none;
    color: white;
    font-size: 2rem;
    cursor: pointer;
  }

  /* Mobile responsive */
  @media (max-width: 768px) {
    .card {
      padding: 1rem;
      border-radius: 12px;
    }

    .filters-grid {
      grid-template-columns: 1fr;
    }

    .payment-header {
      flex-direction: column;
      gap: 0.5rem;
    }

    .amount-group {
      align-self: flex-start;
    }
  }
</style>
