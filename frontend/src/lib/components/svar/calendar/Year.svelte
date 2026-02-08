<script lang="ts">
  import { getContext } from 'svelte';
  import { SvelteDate } from 'svelte/reactivity';
  import { delegateClick } from '@svar-ui/lib-dom';
  import Button from './Button.svelte';
  import { getPartValue } from './helpers';

  interface Props {
    value?: Date | null;
    current?: Date;
    part?: string;
    oncancel?: () => void;
    onchange?: (date: Date) => void;
    onshift?: (ev: { diff?: number; type?: string }) => void;
  }

  let {
    value = $bindable(),
    current = $bindable(new Date()),
    part = 'normal',
    oncancel,
    onchange,
    onshift
  }: Props = $props();

  const locale = getContext<{ getRaw: () => { calendar: { monthShort: string[]; done: string } } }>(
    'wx-i18n'
  ).getRaw().calendar;
  const months = locale.monthShort;

  const monthNum = $derived.by(() => current.getMonth());

  const selectMonths = {
    click: selectMonth
  };

  function selectMonth(month: number | null, e: Event) {
    if (month || month === 0) {
      e.stopPropagation();
      current.setMonth(month);
      current = new SvelteDate(current);

      if (onshift) onshift({});
    }

    if (part === 'normal') value = new SvelteDate(current);

    if (oncancel) oncancel();
  }

  function done() {
    const date = new SvelteDate(getPartValue(value ?? null, part) || current);

    date.setMonth(current.getMonth());
    date.setFullYear(current.getFullYear());

    if (onchange) onchange(date);
  }
</script>

<div class="wx-months" use:delegateClick={selectMonths}>
  {#each months as month, i (i)}
    <div class="wx-month" class:wx-current={monthNum === i} data-id={i}>
      {month}
    </div>
  {/each}
</div>
<div class="wx-buttons">
  <Button onclick={done}>{locale.done}</Button>
</div>

<style>
  .wx-months {
    display: flex;
    flex-wrap: wrap;
    margin: var(--wx-calendar-gap);
  }

  .wx-month {
    flex: 0 0 calc(100% / 4 - var(--wx-calendar-gap) * 2);
    max-width: calc(100% / 4 - var(--wx-calendar-gap) * 2);
    margin: calc(var(--wx-calendar-gap) * 2) var(--wx-calendar-gap);
    text-align: center;
    cursor: pointer;
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    justify-content: center;
    height: var(--wx-calendar-cell-size);
    border-radius: var(--wx-calendar-border-radius);
  }
  .wx-month.wx-current {
    background: var(--wx-color-primary);
    color: var(--wx-color-primary-font);
  }
  .wx-month:not(:global(.wx-current)):hover {
    background-color: var(--wx-background-hover);
  }

  .wx-buttons {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    justify-content: center;
    margin-top: var(--wx-calendar-gap);
  }
</style>
