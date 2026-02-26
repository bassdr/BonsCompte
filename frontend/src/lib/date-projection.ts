/**
 * Shared pure functions, types, and constants for Cashflow and Reconciliation pages.
 */

import { SvelteDate, SvelteMap } from 'svelte/reactivity';
import { formatMonthYear as formatMonthYearI18n } from '$lib/format/date';
import { getCurrentLocale } from '$lib/i18n';
import { fr, enUS } from 'date-fns/locale';
import type {
  PaymentWithContributions,
  PaymentOccurrence,
  PairwisePaymentBreakdown
} from '$lib/api';

// ─── Constants ───────────────────────────────────────────────────────────────

export const CHART_COLORS = [
  '#7b61ff', // accent purple
  '#2a9d8f', // teal
  '#e76f51', // coral
  '#f4a261', // orange
  '#264653', // dark blue
  '#e9c46a', // yellow
  '#6c757d', // gray
  '#dc3545' // red
];

// ─── Types ───────────────────────────────────────────────────────────────────

export type HorizonOption =
  | 'none'
  | 'thisMonth'
  | 'nextMonth'
  | '3months'
  | '6months'
  | '12months'
  | 'custom';

export interface BreakdownMonthData {
  monthKey: string;
  month: string;
  total: number;
  items: PairwisePaymentBreakdown[];
}

export interface BreakdownYearData {
  year: string;
  total: number;
  months: SvelteMap<string, BreakdownMonthData>;
}

export interface OccurrencesByDateData {
  date: string;
  dateDisplay: string;
  occurrences: PaymentOccurrence[];
  totalAmount: number;
}

export interface OccurrencesByMonthData {
  monthKey: string;
  month: string;
  expanded: boolean;
  totalAmount: number;
  dates: SvelteMap<string, OccurrencesByDateData>;
  participantTotals: Map<string, number>;
}

export interface OccurrencesByYearData {
  year: string;
  expanded: boolean;
  months: SvelteMap<string, OccurrencesByMonthData>;
}

export interface PoolTotalStats {
  currentTotal: number;
  projectedTotal: number;
  minTotal: number;
  hasNegative: boolean;
  currentExpectedMinimum: number;
  projectedExpectedMinimum: number;
  maxExpectedMinimum: number;
  isBelowExpected: boolean;
}

// ─── Date helpers ────────────────────────────────────────────────────────────

export function getLocalDateString(date: Date = new SvelteDate()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function parseLocalDate(dateStr: string): Date {
  const [year, month, day] = dateStr.split('-').map(Number);
  return new SvelteDate(year, month - 1, day);
}

export function getYear(dateStr: string): string {
  return dateStr.substring(0, 4);
}

export function getMonthKey(dateStr: string): string {
  return dateStr.substring(0, 7);
}

export function formatMonthYear(dateStr: string): string {
  return formatMonthYearI18n(dateStr);
}

export function getHorizonEndDate(startDate: string, months: number): string {
  const date = parseLocalDate(startDate);
  date.setMonth(date.getMonth() + months);
  return getLocalDateString(date);
}

// ─── Chart helpers ───────────────────────────────────────────────────────────

export function getDateFnsLocale() {
  const lang = getCurrentLocale();
  return lang.startsWith('fr') ? fr : enUS;
}

export function getChartDateFormat(dateFormat: string): string {
  switch (dateFormat) {
    case 'iso':
      return 'MM-dd';
    case 'ymd':
      return 'MM/dd';
    case 'dmy':
      return 'dd/MM';
    case 'mdy':
    default:
      return 'MM/dd';
  }
}

// ─── Focus helpers ───────────────────────────────────────────────────────────

export function paymentInvolvesFocus(
  paymentId: number,
  payerId: number | null,
  focusParticipantId: number | null,
  paymentMap: Map<number, PaymentWithContributions>
): boolean {
  if (focusParticipantId === null) return true;
  if (payerId === focusParticipantId) return true;

  const payment = paymentMap.get(paymentId);
  if (payment?.contributions) {
    return payment.contributions.some(
      (c: { participant_id: number }) => c.participant_id === focusParticipantId
    );
  }
  return false;
}

export function settlementInvolvesFocus(
  s: { from_participant_id: number; to_participant_id: number },
  focusParticipantId: number | null
): boolean {
  if (focusParticipantId === null) return true;
  return s.from_participant_id === focusParticipantId || s.to_participant_id === focusParticipantId;
}

// ─── Breakdown grouping ─────────────────────────────────────────────────────

export function groupBreakdownByYearMonth(
  breakdown: PairwisePaymentBreakdown[]
): SvelteMap<string, BreakdownYearData> {
  const yearMap = new SvelteMap<string, BreakdownYearData>();

  for (const item of breakdown) {
    const year = getYear(item.occurrence_date);
    const monthKey = getMonthKey(item.occurrence_date);

    if (!yearMap.has(year)) {
      yearMap.set(year, {
        year,
        total: 0,
        months: new SvelteMap()
      });
    }

    const yearData = yearMap.get(year)!;
    if (!yearData.months.has(monthKey)) {
      yearData.months.set(monthKey, {
        monthKey,
        month: formatMonthYear(item.occurrence_date),
        total: 0,
        items: []
      });
    }

    const monthData = yearData.months.get(monthKey)!;
    monthData.items.push(item);
    monthData.total += item.amount;
    yearData.total += item.amount;
  }

  const sortedYears = [...yearMap.entries()].sort((a, b) => b[0].localeCompare(a[0]));

  const result = new SvelteMap<string, BreakdownYearData>();
  for (const [year, yearData] of sortedYears) {
    const sortedMonths = [...yearData.months.entries()].sort((a, b) => b[0].localeCompare(a[0]));

    yearData.months = new SvelteMap();
    for (const [monthKey, monthData] of sortedMonths) {
      monthData.items.sort((a, b) => b.occurrence_date.localeCompare(a.occurrence_date));
      yearData.months.set(monthKey, monthData);
    }

    result.set(year, yearData);
  }

  return result;
}

// ─── Occurrence list helpers ─────────────────────────────────────────────────

export function getYearTotal(yearData: OccurrencesByYearData): number {
  let total = 0;
  for (const monthData of yearData.months.values()) {
    total += monthData.totalAmount;
  }
  return total;
}

export function getTransactionCount(monthData: OccurrencesByMonthData): number {
  let count = 0;
  for (const dateData of monthData.dates.values()) {
    count += dateData.occurrences.length;
  }
  return count;
}

export function getYearTransactionCount(yearData: OccurrencesByYearData): number {
  let count = 0;
  for (const monthData of yearData.months.values()) {
    count += getTransactionCount(monthData);
  }
  return count;
}

export function getYearParticipantTotals(yearData: OccurrencesByYearData): Map<string, number> {
  const totals = new SvelteMap<string, number>();
  for (const monthData of yearData.months.values()) {
    for (const [participantName, amount] of monthData.participantTotals.entries()) {
      const current = totals.get(participantName) || 0;
      totals.set(participantName, current + amount);
    }
  }
  return totals;
}

// ─── Recurring date calculation ──────────────────────────────────────────────

export function getNextRecurringDate(
  payment: PaymentWithContributions,
  afterDate: Date
): string | null {
  const startDateStr = payment.payment_date.split('T')[0];
  const startDate = parseLocalDate(startDateStr);

  if (payment.recurrence_end_date) {
    const endDate = parseLocalDate(payment.recurrence_end_date);
    if (endDate <= afterDate) return null;
  }

  const recurrenceType = payment.recurrence_type || 'monthly';
  const interval = payment.recurrence_interval || 1;

  function getDaysInMonth(year: number, month: number): number {
    return new SvelteDate(year, month + 1, 0).getDate();
  }

  // Handle enhanced weekly with weekdays pattern
  if (payment.recurrence_weekdays && recurrenceType === 'weekly') {
    try {
      const weekPatterns: number[][] = JSON.parse(payment.recurrence_weekdays);
      if (weekPatterns.length === 0) return null;

      const searchStart = new SvelteDate(afterDate);
      searchStart.setDate(searchStart.getDate() + 1);

      const startWeekday = startDate.getDay();
      const weekStart = new SvelteDate(startDate);
      weekStart.setDate(weekStart.getDate() - startWeekday);

      const msPerWeek = 7 * 24 * 60 * 60 * 1000;

      for (let daysAhead = 0; daysAhead < 365 * 2; daysAhead++) {
        const checkDate = new SvelteDate(searchStart);
        checkDate.setDate(checkDate.getDate() + daysAhead);

        if (checkDate < startDate) continue;

        if (payment.recurrence_end_date) {
          const endDate = parseLocalDate(payment.recurrence_end_date);
          if (checkDate > endDate) return null;
        }

        const checkWeekStart = new SvelteDate(checkDate);
        checkWeekStart.setDate(checkWeekStart.getDate() - checkDate.getDay());
        const weeksDiff = Math.floor((checkWeekStart.getTime() - weekStart.getTime()) / msPerWeek);
        const cycleWeek =
          ((weeksDiff % weekPatterns.length) + weekPatterns.length) % weekPatterns.length;

        const weekdays = weekPatterns[cycleWeek];
        if (weekdays.includes(checkDate.getDay())) {
          return getLocalDateString(checkDate);
        }
      }
      return null;
    } catch {
      // Fall through to legacy logic
    }
  }

  // Handle enhanced monthly with monthdays pattern
  if (payment.recurrence_monthdays && recurrenceType === 'monthly') {
    try {
      const monthdays: number[] = JSON.parse(payment.recurrence_monthdays);
      if (monthdays.length === 0) return null;

      let year = afterDate.getFullYear();
      let month = afterDate.getMonth();

      for (let monthsAhead = 0; monthsAhead < 12 * 10; monthsAhead++) {
        const daysInMonth = getDaysInMonth(year, month);

        for (const day of monthdays.sort((a, b) => a - b)) {
          const actualDay = Math.min(day, daysInMonth);
          const checkDate = new SvelteDate(year, month, actualDay);

          if (checkDate <= afterDate) continue;
          if (checkDate < startDate) continue;

          if (payment.recurrence_end_date) {
            const endDate = parseLocalDate(payment.recurrence_end_date);
            if (checkDate > endDate) return null;
          }

          return getLocalDateString(checkDate);
        }

        month++;
        if (month > 11) {
          month = 0;
          year++;
        }
      }
      return null;
    } catch {
      // Fall through to legacy logic
    }
  }

  // Handle enhanced yearly with months pattern
  if (payment.recurrence_months && recurrenceType === 'yearly') {
    try {
      const months: number[] = JSON.parse(payment.recurrence_months);
      if (months.length === 0) return null;

      const baseDay = startDate.getDate();
      let year = afterDate.getFullYear();

      for (let yearsAhead = 0; yearsAhead < 50; yearsAhead++) {
        for (const month of months.sort((a, b) => a - b)) {
          if (month < 1 || month > 12) continue;

          const jsMonth = month - 1;
          const daysInMonth = getDaysInMonth(year, jsMonth);
          const actualDay = Math.min(baseDay, daysInMonth);
          const checkDate = new SvelteDate(year, jsMonth, actualDay);

          if (checkDate <= afterDate) continue;
          if (checkDate < startDate) continue;

          if (payment.recurrence_end_date) {
            const endDate = parseLocalDate(payment.recurrence_end_date);
            if (checkDate > endDate) return null;
          }

          return getLocalDateString(checkDate);
        }

        year++;
      }
      return null;
    } catch {
      // Fall through to legacy logic
    }
  }

  // Legacy simple interval logic
  const timesPer = payment.recurrence_times_per;

  let dayInterval: number;
  if (timesPer) {
    switch (recurrenceType) {
      case 'weekly':
        dayInterval = Math.max(1, Math.floor(7 / timesPer));
        break;
      case 'monthly':
        dayInterval = Math.max(1, Math.floor(30 / timesPer));
        break;
      case 'yearly':
        dayInterval = Math.max(1, Math.floor(365 / timesPer));
        break;
      default:
        dayInterval = 1;
    }
  } else {
    switch (recurrenceType) {
      case 'daily':
        dayInterval = interval;
        break;
      case 'weekly':
        dayInterval = interval * 7;
        break;
      case 'monthly':
        dayInterval = interval * 30;
        break;
      case 'yearly':
        dayInterval = interval * 365;
        break;
      default:
        dayInterval = 30;
    }
  }

  const current = new SvelteDate(startDate);
  const maxIterations = 1000;
  let iterations = 0;

  while (current <= afterDate && iterations < maxIterations) {
    current.setDate(current.getDate() + dayInterval);
    iterations++;
  }

  if (iterations >= maxIterations) return null;

  if (payment.recurrence_end_date) {
    const endDate = parseLocalDate(payment.recurrence_end_date);
    if (current > endDate) return null;
  }

  return getLocalDateString(current);
}
