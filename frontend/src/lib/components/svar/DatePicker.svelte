<script lang="ts">
  import { getContext } from 'svelte';
  import { dateToString } from '@svar-ui/lib-dom';

  // Use original SVAR components for Text and Dropdown
  import { Text, Dropdown } from '@svar-ui/svelte-core';
  // Use our forked Calendar with disabledButtons support
  import Calendar from './Calendar.svelte';

  // Calendar locale contains month/day names for date formatting
  interface CalendarLocale {
    monthFull: string[];
    monthShort: string[];
    dayFull: string[];
    dayShort: string[];
    done: string;
    clear: string;
    today: string;
  }

  interface I18nContext {
    getRaw: () => {
      calendar: CalendarLocale;
      formats: { dateFormat: string | ((date: Date) => string) };
    };
  }

  // Default locale fallback
  function defaultLocale(): I18nContext {
    return {
      getRaw: () => ({
        calendar: {
          monthFull: [
            'January',
            'February',
            'March',
            'April',
            'May',
            'June',
            'July',
            'August',
            'September',
            'October',
            'November',
            'December'
          ],
          monthShort: [
            'Jan',
            'Feb',
            'Mar',
            'Apr',
            'May',
            'Jun',
            'Jul',
            'Aug',
            'Sep',
            'Oct',
            'Nov',
            'Dec'
          ],
          dayFull: ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'],
          dayShort: ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'],
          done: 'Done',
          clear: 'Clear',
          today: 'Today'
        },
        formats: { dateFormat: '%d/%m/%Y' }
      })
    };
  }

  interface Props {
    value?: Date | null;
    id?: string;
    disabled?: boolean;
    error?: boolean;
    width?: string;
    align?: 'start' | 'center' | 'end';
    placeholder?: string;
    format?: string | ((date: Date) => string);
    buttons?: ('clear' | 'today')[];
    disabledButtons?: string[];
    disabledReasons?: Record<string, string>;
    css?: string;
    title?: string;
    editable?: boolean | ((value: string) => Date | null);
    clear?: boolean;
    onchange?: (ev: { value: Date | null }) => void;
  }

  let {
    value = $bindable<Date | null>(null),
    id,
    disabled = false,
    error = false,
    width = 'unset',
    align = 'start',
    placeholder = '',
    format = '',
    buttons = ['clear', 'today'],
    disabledButtons = [],
    disabledReasons = {},
    css = '',
    title = '',
    editable = false,
    clear = false,
    onchange
  }: Props = $props();

  const i18nContext = getContext<I18nContext>('wx-i18n') || defaultLocale();
  const { calendar: calendarLocale, formats } = i18nContext.getRaw();
  const dateFormat = $derived.by((): ((date: Date) => string) => {
    const f = format || formats.dateFormat;
    return typeof f === 'function' ? f : dateToString(f, calendarLocale);
  });

  let popup = $state(false);

  function oncancel(): void {
    popup = false;
  }

  function doChange(v: Date | null): void {
    // skip "select" event if the same value
    // or different objects with the same value
    const skipEvent =
      v === value || (v && value && v.valueOf() === value.valueOf()) || (!v && !value);

    value = v;
    if (!skipEvent && onchange) {
      onchange({ value });
    }

    // fire after on-click finished
    setTimeout(oncancel, 1);
  }

  const formattedValue = $derived(value ? dateFormat(value) : '');

  function change(ev: { value: string | number; input?: boolean }): void {
    const { value: v, input } = ev;
    if (!editable && !clear) return;
    if (input) return;

    // convert to date, but ignore empty string input
    let date: Date | null;
    if (typeof editable === 'function') {
      date = editable(String(v));
    } else {
      date = v ? new Date(v) : null;
    }

    // if date is invalid ( incorrect text input ) then use old value
    // else use the entered date
    // in any case fallback to null, to prevent undefined as value
    date = date && isNaN(date.getTime()) ? value || null : date || null;
    doChange(date);
  }
</script>

<svelte:window onscroll={oncancel} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wx-datepicker" onclick={() => (popup = true)}>
  <Text
    {css}
    {title}
    value={formattedValue}
    {id}
    readonly={!editable}
    {disabled}
    {error}
    {placeholder}
    onchange={change}
    icon="wxi-calendar"
    inputStyle="cursor: pointer; width: 100%; padding-right: calc(var(--wx-input-icon-size) + var(--wx-input-icon-indent) * 2);"
    {clear}
  />

  {#if popup && !disabled}
    <Dropdown {oncancel} {width} {align} autoFit={!!align}>
      <Calendar
        {buttons}
        {disabledButtons}
        {disabledReasons}
        {value}
        onchange={(e) => doChange(e.value)}
      />
    </Dropdown>
  {/if}
</div>

<style>
  .wx-datepicker {
    position: relative;
    width: var(--wx-input-width);
  }
</style>
