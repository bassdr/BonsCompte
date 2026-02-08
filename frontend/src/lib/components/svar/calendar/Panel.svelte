<script lang="ts">
  import { getContext } from 'svelte';
  // Use our local Header component
  import Header from './Header.svelte';
  // Use our forked Button with disabled support
  import Button from './Button.svelte';
  // Use our local helpers
  import { configs } from './helpers';

  const _ = getContext<{ getGroup: (name: string) => (key: string) => string }>('wx-i18n').getGroup(
    'calendar'
  );

  interface Props {
    value?: Date | null;
    current?: Date;
    part?: 'normal' | 'left' | 'right' | 'both';
    markers?: unknown;
    buttons?: ('clear' | 'today' | 'done')[];
    disabledButtons?: string[];
    disabledReasons?: Record<string, string>;
    onshift?: () => void;
    onchange?: (ev: { value?: Date | null; select?: boolean }) => void;
  }

  let {
    value,
    current = $bindable(new Date()),
    part = 'normal',
    markers = null,
    buttons = ['clear', 'today'],
    disabledButtons = [],
    disabledReasons = {},
    onshift: shift,
    onchange: change
  }: Props = $props();

  let type = $state<'month' | 'year' | 'duodecade'>('month');

  let buttonList = $derived.by(() => {
    if (Array.isArray(buttons)) return buttons;
    return buttons ? ['clear', 'today'] : [];
  });

  function isButtonDisabled(btn: string): boolean {
    return disabledButtons.includes(btn);
  }

  function getButtonTitle(btn: string): string {
    if (isButtonDisabled(btn) && disabledReasons[btn]) {
      return disabledReasons[btn];
    }
    return '';
  }

  function selectDate(ev: Event, date: Date | null | number): void {
    ev.preventDefault();
    if (date === -1) {
      // "done" button - ignore
      return;
    }
    if (change) change({ value: date as Date | null });
  }

  function oncancel(): void {
    if (type === 'duodecade') type = 'year';
    else if (type === 'year') type = 'month';
  }

  function onshift(ev: { diff?: number; type?: string }): void {
    const diff = ev.diff ?? 0;

    if (diff === 0) {
      if (type === 'month') type = 'year';
      else if (type === 'year') type = 'duodecade';
      return;
    }
    if (diff) {
      const obj = configs[type];
      current = diff > 0 ? obj.next(current) : obj.prev(current);
    }
    if (shift) shift();
  }

  function handleCalendarChange(newValue: Date): void {
    type = 'month';
    if (change) change({ select: true, value: newValue });
  }

  function getButtonValue(btn: string): Date | null | number {
    if (btn === 'done') return -1;
    if (btn === 'clear') return null;
    if (btn === 'today') return new Date();
    return null;
  }

  const SvelteComponent = $derived(configs[type].component);
</script>

<div class="wx-calendar {part !== 'normal' && part !== 'both' ? 'wx-part' : ''}">
  <div class="wx-wrap">
    <Header date={current} {part} {type} {onshift} />
    <div>
      <SvelteComponent
        {value}
        bind:current
        {part}
        {markers}
        onchange={handleCalendarChange}
        {oncancel}
        {onshift}
      />

      {#if type === 'month' && buttonList.length > 0}
        <div class="wx-buttons">
          {#each buttonList as btn (btn)}
            <div class="wx-button-item">
              <Button
                disabled={isButtonDisabled(btn)}
                title={getButtonTitle(btn)}
                onclick={(e: Event) => !isButtonDisabled(btn) && selectDate(e, getButtonValue(btn))}
              >
                {_(btn)}
              </Button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .wx-calendar {
    height: auto;
    width: 100%;
    padding: var(--wx-calendar-padding);
    cursor: default;
    font-family: var(--wx-calendar-font-family);
    font-size: var(--wx-calendar-font-size);
    line-height: var(--wx-calendar-line-height);
    font-weight: var(--wx-calendar-font-weight);
    color: var(--wx-calendar-font-color);
  }
  .wx-calendar.wx-part {
    padding-bottom: 0;
  }
  .wx-wrap {
    width: calc(var(--wx-calendar-cell-size) * 7);
    margin: 0 auto;
  }
  .wx-buttons {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    justify-content: flex-end;
    margin-top: calc(var(--wx-calendar-gap) * 2);
  }
  .wx-button-item + .wx-button-item {
    margin-left: calc(var(--wx-calendar-gap) * 3);
  }
</style>
