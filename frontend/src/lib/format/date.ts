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
 * Format a date according to user preferences
 */
export function formatDate(dateStr: string | Date): string {
  if (!dateStr) return '';

  const prefs = preferences.get();
  const lang = getCurrentLocale();

  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;

  // Handle invalid dates
  if (isNaN(date.getTime())) {
    return String(dateStr);
  }

  const format = (prefs.date_format as DateFormatType) || 'mdy';

  if (format === 'iso') {
    return getLocalDateString(date);
  }

  const options: Intl.DateTimeFormatOptions = getDateFormatOptions(format);
  return new Intl.DateTimeFormat(lang, options).format(date);
}

/**
 * Format a date with weekday
 */
export function formatDateWithWeekday(dateStr: string | Date): string {
  const prefs = preferences.get();
  const lang = getCurrentLocale();

  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;
  const format = (prefs.date_format as DateFormatType) || 'mdy';

  const options: Intl.DateTimeFormatOptions = {
    weekday: 'short',
    ...getDateFormatOptions(format)
  };

  return new Intl.DateTimeFormat(lang, options).format(date);
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
 * Format a date in short form (for compact displays)
 */
export function formatDateShort(dateStr: string | Date): string {
  const prefs = preferences.get();
  const lang = getCurrentLocale();

  const date = typeof dateStr === 'string' ? parseLocalDate(dateStr) : dateStr;
  const format = (prefs.date_format as DateFormatType) || 'mdy';

  if (format === 'iso') {
    return getLocalDateString(date);
  }

  const options: Intl.DateTimeFormatOptions = {
    month: 'short',
    day: 'numeric'
  };

  return new Intl.DateTimeFormat(lang, options).format(date);
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

function getDateFormatOptions(format: DateFormatType): Intl.DateTimeFormatOptions {
  switch (format) {
    case 'mdy':
      return { month: 'short', day: 'numeric', year: 'numeric' };
    case 'dmy':
      return { day: 'numeric', month: 'short', year: 'numeric' };
    case 'ymd':
      return { year: 'numeric', month: 'short', day: 'numeric' };
    case 'iso':
      return { year: 'numeric', month: '2-digit', day: '2-digit' };
    default:
      return { month: 'short', day: 'numeric', year: 'numeric' };
  }
}
