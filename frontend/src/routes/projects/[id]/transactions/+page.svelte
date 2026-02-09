<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { getPayments, deletePayment, type PaymentWithContributions } from '$lib/api';
  import { participants, canEdit } from '$lib/stores/project';
  import { _ } from '$lib/i18n';
  import { getLocalDateString, parseLocalDate } from '$lib/format/date';
  import { getErrorKey } from '$lib/errors';
  import DateInput from '$lib/components/DateInput.svelte';
  import TransactionCard from '$lib/components/TransactionCard.svelte';

  let transactions: PaymentWithContributions[] = $state([]);
  let loading = $state(true);
  let errorKey = $state('');

  // Filter state
  let searchText = $state('');
  let filterPayerId = $state<number | null>(null);
  let filterContributorId = $state<number | null>(null);
  let filterTransactionType = $state<string>('');
  let filterRecurring = $state<string>('');
  let filterStatus = $state<string>('');
  let filterDateFrom = $state('');
  let filterDateTo = $state('');

  // Filter date buttons - use disabledButtons for Today when appropriate
  let filterFromDisabledButtons = $derived.by((): 'today'[] => {
    const today = getLocalDateString();
    return filterDateFrom === today ? ['today'] : [];
  });

  let filterFromDisabledReasons = $derived.by((): Record<string, string> => {
    const today = getLocalDateString();
    if (filterDateFrom === today) {
      return { today: $_('datePicker.alreadyToday', { default: 'Already set to today' }) };
    }
    return {};
  });

  let filterToDisabledButtons = $derived.by((): 'today'[] => {
    const today = getLocalDateString();
    // Disable Today if already today OR if From date is today or future
    if (filterDateTo === today) return ['today'];
    if (filterDateFrom) {
      const fromDate = parseLocalDate(filterDateFrom);
      const todayDate = parseLocalDate(today);
      if (fromDate >= todayDate) return ['today'];
    }
    return [];
  });

  let filterToDisabledReasons = $derived.by((): Record<string, string> => {
    const today = getLocalDateString();
    if (filterDateTo === today) {
      return { today: $_('datePicker.alreadyToday', { default: 'Already set to today' }) };
    }
    if (filterDateFrom) {
      const fromDate = parseLocalDate(filterDateFrom);
      const todayDate = parseLocalDate(today);
      if (fromDate >= todayDate) {
        return {
          today: $_('datePicker.fromDateInFuture', { default: 'From date is today or later' })
        };
      }
    }
    return {};
  });

  // Pagination state
  let transactionsToShow = $state(30);
  const PAYMENTS_PER_PAGE = 30;

  // Image modal state
  let showImageModal = $state(false);
  let modalImage = $state<string | null>(null);

  // Load transactions
  $effect(() => {
    const projectId = parseInt($page.params.id ?? '');
    if (!isNaN(projectId)) {
      loadTransactions(projectId);
    }
  });

  async function loadTransactions(projectId: number) {
    loading = true;
    errorKey = '';
    try {
      transactions = await getPayments(projectId);
    } catch (e) {
      errorKey = getErrorKey(e, 'transactions.failedToLoad');
    } finally {
      loading = false;
    }
  }

  // Apply all filters
  let filteredTransactions = $derived.by(() => {
    let result = transactions;

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

    // Filter by transaction type
    // Note: Rule check comes first since rules can look like inflows (payer_id=null, receiver_account_id!=null)
    if (filterTransactionType) {
      if (filterTransactionType === 'rule') {
        result = result.filter((p) => p.affects_balance === false);
      } else if (filterTransactionType === 'expense') {
        result = result.filter(
          (p) =>
            p.affects_balance !== false && p.payer_id !== null && p.receiver_account_id === null
        );
      } else if (filterTransactionType === 'transfer') {
        result = result.filter(
          (p) =>
            p.affects_balance !== false && p.payer_id !== null && p.receiver_account_id !== null
        );
      } else if (filterTransactionType === 'inflow') {
        result = result.filter(
          (p) =>
            p.affects_balance !== false && p.payer_id === null && p.receiver_account_id !== null
        );
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

    // Filter by status (is_final boolean)
    if (filterStatus) {
      const isFinal = filterStatus === 'final';
      result = result.filter((p) => p.is_final === isFinal);
    }

    // Filter by date range
    if (filterDateFrom) {
      result = result.filter((p) => {
        const transactionDate = p.payment_date.split('T')[0];
        return transactionDate >= filterDateFrom;
      });
    }
    if (filterDateTo) {
      result = result.filter((p) => {
        const transactionDate = p.payment_date.split('T')[0];
        return transactionDate <= filterDateTo;
      });
    }

    return result;
  });

  // Active filter count
  let activeFilterCount = $derived(
    (searchText.trim() ? 1 : 0) +
      (filterPayerId !== null ? 1 : 0) +
      (filterContributorId !== null ? 1 : 0) +
      (filterTransactionType !== '' ? 1 : 0) +
      (filterRecurring !== '' ? 1 : 0) +
      (filterStatus !== '' ? 1 : 0) +
      (filterDateFrom !== '' ? 1 : 0) +
      (filterDateTo !== '' ? 1 : 0)
  );

  // Pagination
  let visibleTransactions = $derived(filteredTransactions.slice(0, transactionsToShow));
  let hasMoreTransactions = $derived(filteredTransactions.length > transactionsToShow);

  function clearFilters() {
    searchText = '';
    filterPayerId = null;
    filterContributorId = null;
    filterTransactionType = '';
    filterRecurring = '';
    filterStatus = '';
    filterDateFrom = '';
    filterDateTo = '';
  }

  function loadMoreTransactions() {
    transactionsToShow += PAYMENTS_PER_PAGE;
  }

  // Get transaction mode for routing to edit page
  function getTransactionMode(p: PaymentWithContributions): string {
    // Rules are identified by affects_balance = false
    if (p.affects_balance === false) return 'rule';
    if (p.payer_id === null && p.receiver_account_id !== null) return 'incoming';
    if (p.receiver_account_id !== null) return 'internal';
    return 'outgoing';
  }

  function editTransaction(transaction: PaymentWithContributions) {
    const mode = getTransactionMode(transaction);
    goto(`/projects/${$page.params.id}/transactions/${mode}?edit=${transaction.id}`);
  }

  async function handleDelete(transactionId: number) {
    if (!confirm($_('transactions.deleteTransaction') + '?')) return;

    const projectId = parseInt($page.params.id ?? '');
    try {
      await deletePayment(projectId, transactionId);
      transactions = transactions.filter((p) => p.id !== transactionId);
    } catch (e) {
      errorKey = getErrorKey(e, 'transactions.failedToDelete');
    }
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

{#if errorKey}
  <div class="error">{$_(errorKey)}</div>
{/if}

<h2>{$_('transactions.title')}</h2>

<!-- Filters Section -->
<section class="card filters">
  <div class="filters-header">
    <h3>{$_('transactions.filters')}</h3>
    {#if activeFilterCount > 0}
      <button type="button" class="clear-filters-btn" onclick={clearFilters}>
        {$_('common.clear')} ({activeFilterCount})
      </button>
    {/if}
  </div>

  <div class="filters-grid">
    <!-- Text Search -->
    <div class="filter-field">
      <label for="search-text">{$_('transactions.searchDescription')}</label>
      <input
        id="search-text"
        type="text"
        bind:value={searchText}
        placeholder={$_('transactions.searchPlaceholder')}
      />
    </div>

    <!-- Payer Filter -->
    <div class="filter-field">
      <label for="filter-payer">{$_('transactions.filterByPayer')}</label>
      <select id="filter-payer" bind:value={filterPayerId}>
        <option value={null}>{$_('transactions.allPayers')}</option>
        {#each $participants as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    <!-- Contributor Filter -->
    <div class="filter-field">
      <label for="filter-contributor">{$_('transactions.filterByContributor')}</label>
      <select id="filter-contributor" bind:value={filterContributorId}>
        <option value={null}>{$_('transactions.allContributors')}</option>
        {#each $participants as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    <!-- Transaction Type Filter -->
    <div class="filter-field">
      <label for="filter-type">{$_('transactions.filterByType')}</label>
      <select id="filter-type" bind:value={filterTransactionType}>
        <option value="">{$_('transactions.allTypes')}</option>
        <option value="expense">{$_('transactions.typeExpense')}</option>
        <option value="transfer">{$_('transactions.typeTransfer')}</option>
        <option value="inflow">{$_('transactions.typeInflow')}</option>
        <option value="rule">{$_('transactions.typeRule')}</option>
      </select>
    </div>

    <!-- Recurring Filter -->
    <div class="filter-field">
      <label for="filter-recurring">{$_('transactions.filterByRecurring')}</label>
      <select id="filter-recurring" bind:value={filterRecurring}>
        <option value="">{$_('transactions.allRecurrence')}</option>
        <option value="yes">{$_('transactions.recurringOnly')}</option>
        <option value="no">{$_('transactions.oneTimeOnly')}</option>
      </select>
    </div>

    <!-- Status Filter -->
    <div class="filter-field">
      <label for="filter-status">{$_('transactions.filterByStatus')}</label>
      <select id="filter-status" bind:value={filterStatus}>
        <option value="">{$_('transactions.allStatuses')}</option>
        <option value="final">{$_('transactions.statusFinal')}</option>
        <option value="draft">{$_('transactions.statusDraft')}</option>
      </select>
    </div>

    <!-- Date From -->
    <div class="filter-field">
      <label for="filter-date-from">{$_('transactions.dateFrom')}</label>
      <DateInput
        id="filter-date-from"
        bind:value={filterDateFrom}
        buttons={['clear', 'today']}
        disabledButtons={filterFromDisabledButtons}
        disabledReasons={filterFromDisabledReasons}
      />
    </div>

    <!-- Date To -->
    <div class="filter-field">
      <label for="filter-date-to">{$_('transactions.dateTo')}</label>
      <DateInput
        id="filter-date-to"
        bind:value={filterDateTo}
        buttons={['clear', 'today']}
        disabledButtons={filterToDisabledButtons}
        disabledReasons={filterToDisabledReasons}
      />
    </div>
  </div>

  {#if activeFilterCount > 0}
    <div class="filter-summary">
      {$_('transactions.showingFiltered')}
      {filteredTransactions.length} / {transactions.length}
    </div>
  {/if}
</section>

<!-- Transactions List -->
<section class="card">
  <h3>{$_('transactions.recentTransactions')}</h3>

  {#if loading}
    <p>{$_('common.loading')}</p>
  {:else if transactions.length === 0}
    <p class="empty">{$_('transactions.noTransactionsYet')}</p>
  {:else if filteredTransactions.length === 0}
    <div class="empty-state">
      <p class="empty">{$_('transactions.noMatchingTransactions')}</p>
      <button type="button" class="clear-filters-btn" onclick={clearFilters}>
        {$_('common.clear')}
      </button>
    </div>
  {:else}
    <div class="transactions-list">
      {#each visibleTransactions as p (p.id)}
        {@const receiver = p.receiver_account_id
          ? $participants.find((pr) => pr.id === p.receiver_account_id)
          : null}
        <TransactionCard
          description={p.description}
          amount={p.amount}
          date={p.payment_date}
          payerName={p.payer_name}
          payerId={p.payer_id}
          receiverName={receiver?.name}
          receiverAccountId={p.receiver_account_id}
          isFinal={p.is_final}
          affectsBalance={p.affects_balance}
          affectsPayerExpectation={p.affects_payer_expectation}
          affectsReceiverExpectation={p.affects_receiver_expectation}
          isRecurring={p.is_recurring}
          recurrenceType={p.recurrence_type}
          recurrenceInterval={p.recurrence_interval}
          recurrenceEndDate={p.recurrence_end_date}
          contributions={p.contributions}
          receiptImage={p.receipt_image}
          onEdit={$canEdit ? () => editTransaction(p) : undefined}
          onDelete={$canEdit ? () => handleDelete(p.id) : undefined}
          onViewReceipt={p.receipt_image ? () => openImageModal(p.receipt_image!) : undefined}
        />
      {/each}
    </div>

    <!-- Load More Button -->
    {#if hasMoreTransactions}
      <div class="load-more-section">
        <button type="button" class="load-more-btn" onclick={loadMoreTransactions}>
          {$_('transactions.loadMore')}
          ({transactionsToShow} / {filteredTransactions.length})
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

  /* Filters - needs position and z-index to create stacking context above other cards */
  .filters {
    margin-bottom: 1.5rem;
    overflow: visible;
    position: relative;
    z-index: 10;
  }

  .filters-grid {
    overflow: visible;
  }

  .filter-field {
    overflow: visible;
  }

  /* Ensure date picker dropdowns appear above other content */
  .filter-field :global(.wx-dropdown) {
    z-index: 100;
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

  /* Transactions List */
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

  .transactions-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
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
  }
</style>
