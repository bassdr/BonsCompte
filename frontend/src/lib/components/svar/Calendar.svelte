<script lang="ts">
  // Use our forked Panel with disabledButtons support
  import Panel from './calendar/Panel.svelte';
  // Use original SVAR Locale
  import { Locale } from '@svar-ui/svelte-core';
  import { SvelteDate } from 'svelte/reactivity';

  interface Props {
    value?: Date | null;
    current?: Date;
    markers?: unknown;
    buttons?: ('clear' | 'today' | 'done')[];
    disabledButtons?: string[];
    disabledReasons?: Record<string, string>;
    onchange?: (ev: { value: Date | null }) => void;
  }

  let {
    value = $bindable<Date | null>(null),
    current = $bindable<Date>(),
    markers = null,
    buttons = ['clear', 'today'],
    disabledButtons = [],
    disabledReasons = {},
    onchange
  }: Props = $props();

  function fixCurrent(force?: boolean): void {
    if (!current || force) current = value ? new SvelteDate(value) : new SvelteDate();
    current.setDate(1);
  }
  fixCurrent(!!value);

  function change(v: { value?: Date | null }): void {
    const x = v.value;
    if (x) {
      value = new SvelteDate(x);
      fixCurrent(true);
    } else {
      value = null;
    }

    if (onchange) onchange({ value });
  }
</script>

<Locale>
  <Panel
    {value}
    bind:current
    {markers}
    {buttons}
    {disabledButtons}
    {disabledReasons}
    onchange={change}
  />
</Locale>
