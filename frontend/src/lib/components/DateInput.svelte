<script lang="ts">
  // Use our forked DatePicker with disabledButtons support
  import { DatePicker } from '$lib/components/svar';
  import { preferences } from '$lib/stores/preferences';
  import { getSVARDateFormat, parseLocalDate, getLocalDateString } from '$lib/format/date';
  import type { DateFormatType } from '$lib/format/date';

  interface Props {
    value?: string | null; // ISO format: YYYY-MM-DD
    id?: string;
    disabled?: boolean;
    min?: string; // ISO format: YYYY-MM-DD
    max?: string; // ISO format: YYYY-MM-DD
    class?: string;
    placeholder?: string;
    buttons?: ('clear' | 'today')[]; // Which buttons to show
    disabledButtons?: ('clear' | 'today')[]; // Which buttons to disable (grayed out)
    disabledReasons?: Record<string, string>; // Tooltip text for disabled buttons
  }

  let {
    value = $bindable<string | null>(''),
    id,
    disabled = false,
    min,
    max,
    class: className = '',
    placeholder,
    buttons = ['clear', 'today'],
    disabledButtons = [],
    disabledReasons = {}
  }: Props = $props();

  // Convert ISO string to Date for SVAR
  let dateValue = $derived.by(() => {
    if (!value) return undefined;
    const date = parseLocalDate(value);
    // Ensure it's a valid date
    if (isNaN(date.getTime())) return undefined;
    return date;
  });

  // Get format from user preferences
  let dateFormat = $derived(getSVARDateFormat($preferences.date_format as DateFormatType));

  // Handle date change from SVAR
  function handleChange(ev: { value: Date | null }): void {
    const newDate = ev?.value;
    if (newDate instanceof Date && !isNaN(newDate.getTime())) {
      value = getLocalDateString(newDate);
    } else if (!newDate) {
      value = '';
    }
  }

  // Validation for min/max constraints
  let errorMessage = $derived.by(() => {
    if (!value) return undefined;

    const current = parseLocalDate(value);

    if (min) {
      const minDate = parseLocalDate(min);
      if (current < minDate) {
        return 'Date is before minimum allowed date';
      }
    }

    if (max) {
      const maxDate = parseLocalDate(max);
      if (current > maxDate) {
        return 'Date is after maximum allowed date';
      }
    }

    return undefined;
  });
</script>

<div class="date-input-wrapper {className}" class:has-error={!!errorMessage}>
  <DatePicker
    {id}
    value={dateValue}
    format={dateFormat}
    {disabled}
    {placeholder}
    editable={true}
    {buttons}
    {disabledButtons}
    {disabledReasons}
    onchange={handleChange}
  />
  {#if errorMessage}
    <p class="date-input-error">{errorMessage}</p>
  {/if}
</div>

<style>
  .date-input-wrapper {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .date-input-error {
    margin: 0;
    padding: 0.5rem 0.75rem;
    background-color: #fee;
    border: 1px solid #fcc;
    border-radius: 4px;
    color: #c33;
    font-size: 0.875rem;
  }
</style>
