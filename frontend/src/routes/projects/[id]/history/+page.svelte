<script lang="ts">
  import { page } from '$app/stores';
  import {
    getProjectHistory,
    undoHistoryEntry,
    verifyHistoryChain,
    type HistoryEntry,
    type ChainVerification
  } from '$lib/api';
  import { isAdmin } from '$lib/stores/project';
  import { _ } from '$lib/i18n';
  import { formatCurrency } from '$lib/format/currency';
  import { formatDate, getDaysDiff, type DateFormatType } from '$lib/format/date';
  import { preferences } from '$lib/stores/preferences';
  import { SvelteSet } from 'svelte/reactivity';
  import { getErrorKey } from '$lib/errors';

  let entries: HistoryEntry[] = $state([]);
  let loading = $state(true);
  let errorKey = $state('');
  let undoingId = $state<number | null>(null);
  let undoErrorKey = $state('');
  let undoSuccess = $state('');

  // Filters
  let filterEntityType = $state<string>('');
  let filterAction = $state<string>('');

  // Chain verification
  let verifying = $state(false);
  let verification = $state<ChainVerification | null>(null);

  // Undo confirmation dialog
  let showUndoDialog = $state(false);
  let entryToUndo = $state<HistoryEntry | null>(null);
  let undoReason = $state('');

  // Expanded entries for viewing payload details
  let expandedEntries = new SvelteSet<number>();

  let projectId = $derived(parseInt($page.params.id ?? ''));

  // Entity type options
  let entityTypes = $derived([
    { value: '', label: $_('history.entities.all') },
    { value: 'payment', label: $_('history.entities.payment') },
    { value: 'participant', label: $_('history.entities.participant') },
    { value: 'project_member', label: $_('history.entities.project_member') },
    { value: 'project', label: $_('history.entities.project') }
  ]);

  // Action type options
  let actionTypes = $derived([
    { value: '', label: $_('history.actions.all') },
    { value: 'CREATE', label: $_('history.actions.CREATE') },
    { value: 'UPDATE', label: $_('history.actions.UPDATE') },
    { value: 'DELETE', label: $_('history.actions.DELETE') },
    { value: 'UNDO', label: $_('history.actions.UNDO') }
  ]);

  // Filtered entries
  let filteredEntries = $derived.by(() => {
    let result = entries;
    if (filterEntityType) {
      result = result.filter((e) => e.entity_type === filterEntityType);
    }
    if (filterAction) {
      result = result.filter((e) => e.action === filterAction);
    }
    return result;
  });

  async function loadHistory() {
    loading = true;
    errorKey = '';
    try {
      entries = await getProjectHistory(projectId, { limit: 200 });
    } catch (e) {
      errorKey = getErrorKey(e, 'history.failedToLoad');
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (projectId) {
      loadHistory();
    }
  });

  function toggleExpanded(entryId: number) {
    if (expandedEntries.has(entryId)) {
      expandedEntries.delete(entryId);
    } else {
      expandedEntries.add(entryId);
    }
  }

  function openUndoDialog(entry: HistoryEntry) {
    entryToUndo = entry;
    undoReason = '';
    showUndoDialog = true;
  }

  function closeUndoDialog() {
    showUndoDialog = false;
    entryToUndo = null;
    undoReason = '';
  }

  async function confirmUndo() {
    if (!entryToUndo) return;

    undoingId = entryToUndo.id;
    undoErrorKey = '';
    undoSuccess = '';

    try {
      await undoHistoryEntry(projectId, entryToUndo.id, undoReason || undefined);
      undoSuccess = $_('history.undoSuccess');
      closeUndoDialog();
      // Reload history to show the new UNDO entry
      await loadHistory();
    } catch (e) {
      undoErrorKey = getErrorKey(e, 'history.failedToUndo');
    } finally {
      undoingId = null;
    }
  }

  async function runVerification() {
    verifying = true;
    verification = null;
    try {
      verification = await verifyHistoryChain(projectId);
    } catch {
      verification = {
        is_valid: false,
        total_entries: 0,
        first_broken_id: null,
        message: $_('history.verificationFailed')
      };
    } finally {
      verifying = false;
    }
  }

  // Reactive date format from user preferences
  let dateFormat = $derived($preferences.date_format as DateFormatType);

  function formatHistoryDate(dateStr: string): string {
    return formatDate(dateStr, dateFormat);
  }

  function formatRelativeLabel(dateStr: string): string {
    const days = getDaysDiff(dateStr);

    if (days === 0) return $_('dates.today');
    if (days === 1) return $_('dates.tomorrow');
    if (days === -1) return $_('dates.yesterday');
    if (days > 1 && days <= 7) return $_('dates.inDays', { values: { days } });
    if (days < -1 && days >= -7) return $_('dates.daysAgo', { values: { days: Math.abs(days) } });
    return '';
  }

  function formatHistoryDateTime(dateStr: string): string {
    const date = formatHistoryDate(dateStr);
    const relative = formatRelativeLabel(dateStr);
    return relative ? `${date} (${relative})` : date;
  }

  function getActionColor(action: string): string {
    switch (action) {
      case 'CREATE':
        return '#2a9d8f';
      case 'UPDATE':
        return '#e9c46a';
      case 'DELETE':
        return '#e76f51';
      case 'UNDO':
        return '#7b61ff';
      default:
        return '#666';
    }
  }

  function getEntityTypeLabel(type: string): string {
    const key = `history.entities.${type}`;
    const translated = $_(key);
    // If translation key not found, return the type as-is
    return translated === key ? type : translated;
  }

  function getActionLabel(action: string): string {
    const key = `history.actions.${action}`;
    const translated = $_(key);
    // If translation key not found, return the action as-is
    return translated === key ? action : translated;
  }

  function canUndo(entry: HistoryEntry): boolean {
    // Can't undo UNDO actions
    if (entry.action === 'UNDO') return false;
    // Can't undo already undone entries
    if (entry.is_undone) return false;
    return true;
  }

  function formatPayload(payload: unknown): string {
    if (!payload) return 'N/A';
    try {
      return JSON.stringify(payload, null, 2);
    } catch {
      return String(payload);
    }
  }

  function getPayloadSummary(entry: HistoryEntry): string {
    const payload = entry.action === 'DELETE' ? entry.payload_before : entry.payload_after;
    if (!payload) return '';

    // Try to extract useful summary based on entity type
    const data = payload as Record<string, unknown>;

    if (entry.entity_type === 'payment') {
      const p = data.payment as Record<string, unknown> | undefined;
      if (p) {
        return `${formatCurrency(Number(p.amount))} - ${p.description || 'No description'}`;
      }
      if (data.amount !== undefined) {
        return `${formatCurrency(Number(data.amount))} - ${data.description || 'No description'}`;
      }
    }

    if (entry.entity_type === 'participant') {
      return (data.name as string) || '';
    }

    if (entry.entity_type === 'project_member') {
      return `${data.username || 'Unknown'} (${data.role || 'editor'})`;
    }

    if (entry.entity_type === 'project') {
      return (data.name as string) || '';
    }

    return '';
  }
</script>

<h2>{$_('history.title')}</h2>

{#if undoSuccess}
  <div class="success-message">{undoSuccess}</div>
{/if}

{#if undoErrorKey}
  <div class="error">{$_(undoErrorKey)}</div>
{/if}

{#if errorKey}
  <div class="error">{$_(errorKey)}</div>
{/if}

<!-- Filters -->
<div class="filters card">
  <div class="filter-row">
    <label>
      <span>{$_('history.type')}:</span>
      <select bind:value={filterEntityType}>
        {#each entityTypes as opt (opt.value)}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{$_('history.action')}:</span>
      <select bind:value={filterAction}>
        {#each actionTypes as opt (opt.value)}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
    </label>
    {#if $isAdmin}
      <button class="verify-btn" onclick={runVerification} disabled={verifying}>
        {verifying ? $_('history.verifying') : $_('history.verifyChain')}
      </button>
    {/if}
  </div>
  {#if verification}
    <div
      class="verification-result"
      class:valid={verification.is_valid}
      class:invalid={!verification.is_valid}
    >
      <span class="verification-icon">{verification.is_valid ? '[OK]' : '[X]'}</span>
      <span>
        {#if verification.total_entries === 0}
          {$_('history.emptyLog')}
        {:else if verification.is_valid}
          {$_('history.chainValid')}
        {:else}
          {$_('history.chainInvalid')}
        {/if}
      </span>
      <span class="verification-count">({verification.total_entries} {$_('history.entries')})</span>
    </div>
  {/if}
</div>

{#if loading}
  <p class="loading">{$_('history.loadingHistory')}</p>
{:else if filteredEntries.length === 0}
  <div class="card empty-state">
    <p>{$_('history.noEntriesFound')}</p>
    {#if filterEntityType || filterAction}
      <button
        class="clear-filters-btn"
        onclick={() => {
          filterEntityType = '';
          filterAction = '';
        }}
      >
        {$_('history.clearFilters')}
      </button>
    {/if}
  </div>
{:else}
  <div class="history-list">
    {#each filteredEntries as entry (entry.id)}
      {@const isExpanded = expandedEntries.has(entry.id)}
      {@const summary = getPayloadSummary(entry)}
      <div
        class="history-entry card"
        class:undone={entry.is_undone}
        class:is-undo={entry.action === 'UNDO'}
      >
        <div
          class="entry-header"
          role="button"
          tabindex="0"
          onclick={() => toggleExpanded(entry.id)}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && toggleExpanded(entry.id)}
        >
          <span class="expand-icon">{isExpanded ? '▼' : '▶'}</span>
          <span class="action-badge" style="background: {getActionColor(entry.action)}">
            {getActionLabel(entry.action)}
          </span>
          <span class="entity-type">{getEntityTypeLabel(entry.entity_type)}</span>
          {#if entry.entity_id}
            <span class="entity-id">#{entry.entity_id}</span>
          {/if}
          {#if summary}
            <span class="entry-summary">{summary}</span>
          {/if}
          <span class="entry-time">
            {formatHistoryDateTime(entry.created_at)}
          </span>
        </div>

        <div class="entry-meta">
          <span class="actor">
            {$_('history.by')}
            {entry.actor_name || $_('history.system')}
          </span>
          {#if entry.is_undone}
            <span class="undone-badge">{$_('history.undone')}</span>
          {/if}
          {#if entry.undoes_history_id}
            <span class="undoes-badge">{$_('history.undoes')} #{entry.undoes_history_id}</span>
          {/if}
          {#if entry.reason}
            <span class="reason">{$_('history.reason')}: {entry.reason}</span>
          {/if}
        </div>

        {#if isExpanded}
          <div class="entry-details">
            {#if entry.payload_before}
              <div class="payload-section">
                <h4>{$_('history.before')}:</h4>
                <pre>{formatPayload(entry.payload_before)}</pre>
              </div>
            {/if}
            {#if entry.payload_after}
              <div class="payload-section">
                <h4>{$_('history.after')}:</h4>
                <pre>{formatPayload(entry.payload_after)}</pre>
              </div>
            {/if}
            <div class="entry-footer">
              <span class="correlation"
                >{$_('history.correlation')}: {entry.correlation_id.substring(0, 8)}...</span
              >
              <span class="full-time">{formatHistoryDateTime(entry.created_at)}</span>
            </div>
          </div>
        {/if}

        {#if $isAdmin && canUndo(entry) && !isExpanded}
          <button
            class="undo-btn"
            onclick={(e) => {
              e.stopPropagation();
              openUndoDialog(entry);
            }}
            disabled={undoingId === entry.id}
          >
            {undoingId === entry.id ? $_('history.undoing') : $_('history.undo')}
          </button>
        {/if}

        {#if isExpanded && $isAdmin && canUndo(entry)}
          <div class="expanded-actions">
            <button
              class="undo-btn"
              onclick={() => openUndoDialog(entry)}
              disabled={undoingId === entry.id}
            >
              {undoingId === entry.id ? $_('history.undoing') : $_('history.undoThisAction')}
            </button>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<!-- Undo Confirmation Dialog -->
{#if showUndoDialog && entryToUndo}
  <div
    class="dialog-overlay"
    role="presentation"
    onclick={closeUndoDialog}
    onkeydown={(e) => e.key === 'Escape' && closeUndoDialog()}
  >
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="undo-dialog-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h3 id="undo-dialog-title">{$_('history.confirmUndo')}</h3>
      <p>
        {$_('history.confirmUndoQuestion')}
      </p>
      <div class="dialog-summary">
        <span class="action-badge" style="background: {getActionColor(entryToUndo.action)}">
          {getActionLabel(entryToUndo.action)}
        </span>
        <span>{getEntityTypeLabel(entryToUndo.entity_type)}</span>
        {#if entryToUndo.entity_id}
          <span>#{entryToUndo.entity_id}</span>
        {/if}
      </div>
      <label class="reason-label">
        <span>{$_('history.reasonOptional')}:</span>
        <input type="text" bind:value={undoReason} placeholder={$_('history.reasonPlaceholder')} />
      </label>
      <div class="dialog-actions">
        <button class="cancel-btn" onclick={closeUndoDialog}>{$_('common.cancel')}</button>
        <button class="confirm-btn" onclick={confirmUndo} disabled={undoingId !== null}>
          {undoingId !== null ? $_('history.undoing') : $_('history.confirmUndo')}
        </button>
      </div>
    </div>
  </div>
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
    margin-bottom: 1rem;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
  }

  .error {
    background: #fee;
    color: #c00;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .success-message {
    background: #d4edda;
    color: #155724;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .loading {
    text-align: center;
    color: #666;
    padding: 2rem;
  }

  /* Filters */
  .filters {
    padding: 1rem 1.5rem;
  }

  .filter-row {
    display: flex;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .filter-row label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .filter-row label span {
    font-weight: 600;
    color: #666;
    font-size: 0.9rem;
  }

  .filter-row select {
    padding: 0.5rem 1rem;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 0.9rem;
    background: white;
  }

  .filter-row select:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .verify-btn {
    margin-left: auto;
    padding: 0.5rem 1rem;
    background: #6c757d;
    color: white;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .verify-btn:hover:not(:disabled) {
    background: #5a6268;
  }

  .verify-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .verification-result {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 1rem;
    padding: 0.75rem;
    border-radius: 8px;
    font-size: 0.9rem;
  }

  .verification-result.valid {
    background: #d4edda;
    color: #155724;
  }

  .verification-result.invalid {
    background: #f8d7da;
    color: #721c24;
  }

  .verification-icon {
    font-size: 1.2rem;
    font-weight: bold;
  }

  .verification-count {
    color: #666;
    margin-left: auto;
  }

  /* Empty state */
  .empty-state {
    text-align: center;
    padding: 3rem;
    color: #666;
  }

  .clear-filters-btn {
    margin-top: 1rem;
    padding: 0.5rem 1rem;
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 8px;
    cursor: pointer;
  }

  .clear-filters-btn:hover {
    background: #eee;
  }

  /* History list */
  .history-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .history-entry {
    padding: 1rem 1.5rem;
    transition: all 0.2s;
    position: relative;
  }

  .history-entry.undone {
    opacity: 0.6;
    background: rgba(200, 200, 200, 0.3);
  }

  .history-entry.is-undo {
    border-left: 4px solid var(--accent, #7b61ff);
  }

  .entry-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    cursor: pointer;
    flex-wrap: wrap;
    padding-right: 6rem; /* Make room for undo button */
  }

  .expand-icon {
    color: var(--accent, #7b61ff);
    font-size: 0.8rem;
    width: 1rem;
    flex-shrink: 0;
  }

  .action-badge {
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    color: white;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .entity-type {
    font-weight: 600;
    color: #333;
  }

  .entity-id {
    color: #888;
    font-size: 0.85rem;
  }

  .entry-summary {
    color: #666;
    font-size: 0.9rem;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-time {
    color: #888;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .entry-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-top: 0.5rem;
    padding-left: 1.75rem;
    font-size: 0.85rem;
    flex-wrap: wrap;
  }

  .actor {
    color: #666;
  }

  .undone-badge {
    background: #ffc107;
    color: #333;
    padding: 0.1rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .undoes-badge {
    background: var(--accent, #7b61ff);
    color: white;
    padding: 0.1rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
  }

  .reason {
    color: #888;
    font-style: italic;
  }

  /* Expanded details */
  .entry-details {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #eee;
  }

  .payload-section {
    margin-bottom: 1rem;
  }

  .payload-section h4 {
    margin: 0 0 0.5rem 0;
    color: #666;
    font-size: 0.9rem;
  }

  .payload-section pre {
    background: #f5f5f5;
    padding: 1rem;
    border-radius: 8px;
    overflow-x: auto;
    font-size: 0.8rem;
    max-height: 300px;
    overflow-y: auto;
  }

  .entry-footer {
    display: flex;
    justify-content: space-between;
    color: #888;
    font-size: 0.8rem;
    margin-top: 1rem;
    padding-top: 0.5rem;
    border-top: 1px solid #eee;
  }

  .correlation {
    font-family: monospace;
  }

  /* Undo button */
  .undo-btn {
    position: absolute;
    right: 1.5rem;
    top: 50%;
    transform: translateY(-50%);
    padding: 0.4rem 0.8rem;
    background: var(--accent, #7b61ff);
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .undo-btn:hover:not(:disabled) {
    background: #6b51ef;
  }

  .undo-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .expanded-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 1rem;
  }

  .expanded-actions .undo-btn {
    position: static;
    transform: none;
  }

  /* Dialog */
  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .dialog {
    background: white;
    border-radius: 16px;
    padding: 2rem;
    max-width: 450px;
    width: 90%;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
  }

  .dialog h3 {
    margin: 0 0 1rem 0;
    color: var(--accent, #7b61ff);
  }

  .dialog p {
    color: #666;
    margin-bottom: 1rem;
  }

  .dialog-summary {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding: 0.75rem;
    background: #f5f5f5;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .reason-label {
    display: block;
    margin-bottom: 1.5rem;
  }

  .reason-label span {
    display: block;
    font-weight: 600;
    color: #666;
    margin-bottom: 0.5rem;
  }

  .reason-label input {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 1rem;
  }

  .reason-label input:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .dialog-actions {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
  }

  .cancel-btn {
    padding: 0.75rem 1.5rem;
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 8px;
    cursor: pointer;
    font-size: 1rem;
  }

  .cancel-btn:hover {
    background: #eee;
  }

  .confirm-btn {
    padding: 0.75rem 1.5rem;
    background: #e76f51;
    color: white;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    font-size: 1rem;
  }

  .confirm-btn:hover:not(:disabled) {
    background: #d45f41;
  }

  .confirm-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  /* Mobile responsive */
  @media (max-width: 768px) {
    .filter-row {
      flex-direction: column;
      align-items: stretch;
    }

    .verify-btn {
      margin-left: 0;
      margin-top: 0.5rem;
    }

    .entry-header {
      flex-wrap: wrap;
      padding-right: 0; /* Remove padding on mobile since button is not absolute */
    }

    .entry-summary {
      flex-basis: 100%;
      padding-left: 1.75rem;
      margin-top: 0.25rem;
    }

    .entry-time {
      flex-basis: 100%;
      padding-left: 1.75rem;
      margin-top: 0.25rem;
    }

    .undo-btn {
      position: static;
      transform: none;
      margin-top: 0.75rem;
      margin-left: 1.75rem;
    }

    .entry-footer {
      flex-direction: column;
      gap: 0.25rem;
    }
  }
</style>
