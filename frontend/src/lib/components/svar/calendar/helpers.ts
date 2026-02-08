// Calendar navigation helpers - forked from @svar-ui/svelte-core
import { Month } from '@svar-ui/svelte-core';
import type { Component } from 'svelte';

// Local forks of SVAR internal components (not in public exports)
import Year from './Year.svelte';
import Duodecade from './Duodecade.svelte';

type CalendarType = 'month' | 'year' | 'duodecade';

// Props accepted by the dynamically-rendered calendar component in Panel.svelte.
// Month (from SVAR), Year, and Duodecade have similar but not identical signatures:
// - SVAR Month accepts value as Date | {start, end} for range selection
// - Our local Year/Duodecade accept value as Date | null
// Using a common subset with optional properties for dynamic component rendering.
interface CalendarPanelProps {
  value?: Date | null;
  current?: Date;
  part?: string;
  markers?: unknown;
  oncancel?: () => void;
  onchange?: (date: Date) => void;
  onshift?: (ev: { diff?: number; type?: string }) => void;
}

interface CalendarConfig {
  component: Component<CalendarPanelProps>;
  next: (current: Date) => Date;
  prev: (current: Date) => Date;
}

export const configs: Record<CalendarType, CalendarConfig> = {
  month: {
    // Type assertion needed: SVAR Month expects value as Date|{start,end} for range mode,
    // but we only use single-date mode where null/undefined are equivalent at runtime
    component: Month as unknown as Component<CalendarPanelProps>,
    next: nextMonth,
    prev: prevMonth
  },
  year: {
    component: Year,
    next: nextYear,
    prev: prevYear
  },
  duodecade: {
    component: Duodecade,
    next: nextDuodecade,
    prev: prevDuodecade
  }
};

function prevMonth(current: Date): Date {
  current = new Date(current);
  current.setMonth(current.getMonth() - 1);
  return current;
}

function nextMonth(current: Date): Date {
  current = new Date(current);
  current.setMonth(current.getMonth() + 1);
  return current;
}

function prevYear(current: Date): Date {
  current = new Date(current);
  current.setFullYear(current.getFullYear() - 1);
  return current;
}

function nextYear(current: Date): Date {
  current = new Date(current);
  current.setFullYear(current.getFullYear() + 1);
  return current;
}

function prevDuodecade(current: Date): Date {
  current = new Date(current);
  current.setFullYear(current.getFullYear() - 10);
  return current;
}

function nextDuodecade(current: Date): Date {
  current = new Date(current);
  current.setFullYear(current.getFullYear() + 10);
  return current;
}

interface PartValue {
  start?: Date | null;
  end?: Date | null;
}

export function getPartValue(
  value: Date | PartValue | null,
  part: string
): Date | null | undefined {
  let date: Date | null | undefined;
  if (part === 'normal') date = value as Date | null;
  else {
    const { start, end } = value as PartValue;
    if (part === 'left') date = start;
    else if (part == 'right') date = end;
    else date = start ? end : start;
  }

  return date;
}
