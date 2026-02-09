import { preferences } from '$lib/stores/preferences';
import { getCurrentLocale } from '$lib/i18n';

export type DateFormatType = 'mdy' | 'dmy' | 'ymd' | 'iso';

/**
 * Parse a date string (YYYY-MM-DD or ISO) into a Date object
 * without timezone issues
 */
export function parseLocalDate(dateStr: string): Date {
  const [datePart] = dateStr.split('T');
  const [year, month, day] = datePart.split('-').map(Number);
  return new Date(year, month - 1, day);
}

/**
 * Get today's date as YYYY-MM-DD string
 */
export function getLocalDateString(date: Date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

/**
 * Format a date according to user preferences.
 * Pass dateFormat explicitly for reactivity in Svelte components.
 * This produces exact format strings (not locale-dependent).
 */
export function formatDate(dateStr: string | Date, dateFormat?: DateFormatType): string {
  if (!dateStr) return '';

  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;

  // Handle invalid dates
  if (isNaN(date.getTime())) {
    return String(dateStr);
  }

  // Use passed format, or fall back to preferences (for non-reactive contexts)
  const format = dateFormat ?? (preferences.get().date_format as DateFormatType) ?? 'mdy';

  return formatDateExact(date, format);
}

/**
 * Format a date with weekday prefix.
 * Pass dateFormat explicitly for reactivity in Svelte components.
 */
export function formatDateWithWeekday(dateStr: string | Date, dateFormat?: DateFormatType): string {
  const lang = getCurrentLocale();
  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;

  // Use passed format, or fall back to preferences (for non-reactive contexts)
  const format = dateFormat ?? (preferences.get().date_format as DateFormatType) ?? 'mdy';

  // Get localized weekday
  const weekday = new Intl.DateTimeFormat(lang, { weekday: 'short' }).format(date);

  // Get exact date format
  const dateFormatted = formatDateExact(date, format);

  return `${weekday} ${dateFormatted}`;
}

/**
 * Format date exactly according to format preference (not locale-dependent for ordering).
 */
function formatDateExact(date: Date, format: DateFormatType): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');

  switch (format) {
    case 'iso':
      return `${year}-${month}-${day}`;
    case 'ymd':
      return `${year}/${month}/${day}`;
    case 'dmy':
      return `${day}/${month}/${year}`;
    case 'mdy':
    default:
      return `${month}/${day}/${year}`;
  }
}

/**
 * Format a date showing month and year only
 */
export function formatMonthYear(dateStr: string | Date): string {
  const lang = getCurrentLocale();
  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;

  return new Intl.DateTimeFormat(lang, {
    month: 'long',
    year: 'numeric'
  }).format(date);
}

/**
 * Get the number of days between a date and today.
 * Positive = future, negative = past, 0 = today.
 */
export function getDaysDiff(dateStr: string | Date): number {
  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : new Date(dateStr);
  const today = new Date();

  // Normalize to midnight
  date.setHours(0, 0, 0, 0);
  today.setHours(0, 0, 0, 0);

  return Math.round((date.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
}

/**
 * Get SVAR DatePicker format string based on user's date format preference
 * SVAR uses strftime-style format strings
 */
export function getSVARDateFormat(dateFormat: DateFormatType = 'mdy'): string {
  switch (dateFormat) {
    case 'mdy':
      return '%m/%d/%Y'; // 02/06/2026
    case 'dmy':
      return '%d/%m/%Y'; // 06/02/2026
    case 'ymd':
      return '%Y/%m/%d'; // 2026/02/06
    case 'iso':
      return '%Y-%m-%d'; // 2026-02-06
    default:
      return '%m/%d/%Y';
  }
}
