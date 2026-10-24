<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import { _ } from '$lib/i18n';

  let {
    onexport,
    disabled = false
  }: {
    onexport: () => void;
    disabled?: boolean;
  } = $props();
</script>

<button type="button" class="export-csv-btn" onclick={onexport} {disabled}>
  <Download size={16} color="currentColor" />
  {$_('common.exportCsv')}
</button>

<style>
  .export-csv-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    background: #f0f0f0;
    border: none;
    border-radius: 6px;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 0.9rem;
    color: #666;
  }

  .export-csv-btn:hover:not(:disabled) {
    background: #e0e0e0;
  }

  .export-csv-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
