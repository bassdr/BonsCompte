<script lang="ts">
  import { _ } from '$lib/i18n';
  import type { OverviewState } from '$lib/date-projection.svelte';
  import { auth } from '$lib/auth';
  import { members } from '$lib/stores/project';

  let { projection }: { projection: OverviewState } = $props();

  const myParticipantId = $derived.by(() => {
    const userId = $auth.user?.id;
    if (!userId) return null;
    return $members.find((m) => m.user_id === userId)?.participant_id ?? null;
  });

  let filterOnMe = $state(false);

  $effect(() => {
    if (filterOnMe && myParticipantId !== null) {
      projection.focusParticipantId = myParticipantId;
    } else if (filterOnMe && myParticipantId === null) {
      filterOnMe = false;
    }
  });

  function handleFocusChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    projection.focusParticipantId = val === '' ? null : parseInt(val);
    if (projection.focusParticipantId !== myParticipantId) {
      filterOnMe = false;
    }
  }
</script>

{#if projection.debts && projection.nonPoolBalances.length > 0}
  <div class="focus-card card">
    <div class="focus-bar">
      <label for="focus-participant" class="focus-label">{$_('overview.focusOn')}</label>
      <select
        id="focus-participant"
        onchange={handleFocusChange}
        value={projection.focusParticipantId ?? ''}
      >
        <option value="">{$_('overview.allParticipants')}</option>
        {#each projection.nonPoolBalances as b (b.participant_id)}
          <option value={b.participant_id}>{b.participant_name}</option>
        {/each}
      </select>
      {#if myParticipantId !== null}
        <label class="filter-label">
          <input
            type="checkbox"
            bind:checked={filterOnMe}
            onchange={() => {
              if (!filterOnMe) projection.focusParticipantId = null;
            }}
          />
          {$_('overview.filterOnMe')}
        </label>
      {/if}
      <label class="filter-label">
        <input type="checkbox" bind:checked={projection.includeDrafts} />
        {$_('overview.includeDrafts')}
      </label>
      {#if projection.focusParticipantId !== null && !filterOnMe}
        <button
          class="clear-focus"
          onclick={() => {
            projection.focusParticipantId = null;
            filterOnMe = false;
          }}>{$_('common.clear')}</button
        >
      {/if}
    </div>
  </div>
{/if}

<style>
  .card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 0;
    margin-bottom: 1.5rem;
  }

  .focus-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 1.25rem;
    flex-wrap: wrap;
  }

  .focus-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: #666;
  }

  .filter-label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: #666;
    cursor: pointer;
  }

  .filter-label input[type='checkbox'] {
    width: 14px;
    height: 14px;
    cursor: pointer;
  }

  .focus-bar select {
    padding: 0.4rem 0.75rem;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 0.875rem;
    background: white;
    min-width: 160px;
    flex: 1;
    max-width: 260px;
  }

  .focus-bar select:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  .clear-focus {
    padding: 0.35rem 0.75rem;
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 0.8rem;
    cursor: pointer;
    color: #666;
  }

  .clear-focus:hover {
    background: #eee;
  }

  @media (max-width: 600px) {
    .focus-bar {
      flex-direction: column;
      align-items: stretch;
      gap: 0.5rem;
    }

    .focus-label {
      text-align: center;
    }

    .focus-bar select {
      width: 100%;
      max-width: 100%;
    }
  }
</style>
