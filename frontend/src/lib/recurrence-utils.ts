/**
 * Pure utility functions for recurring payment occurrence calculations.
 * Used by TransactionCard (display) and the transactions page (form + info messages).
 */

// --- Exported date helpers ---

export function parseDate(dateStr: string): Date {
  const [year, month, day] = dateStr.split('-').map(Number);
  return new Date(year, month - 1, day, 12);
}

export function formatDateStr(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function addDays(date: Date, days: number): Date {
  const result = new Date(date);
  result.setDate(result.getDate() + days);
  return result;
}

export function addMonths(date: Date, months: number): Date {
  const result = new Date(date);
  const targetMonth = result.getMonth() + months;
  result.setMonth(targetMonth);
  if (result.getMonth() !== ((targetMonth % 12) + 12) % 12) {
    result.setDate(0);
  }
  return result;
}

export function addYears(date: Date, years: number): Date {
  const result = new Date(date);
  result.setFullYear(result.getFullYear() + years);
  if (date.getMonth() === 1 && date.getDate() === 29 && result.getDate() !== 29) {
    result.setDate(28);
  }
  return result;
}

// --- Occurrence computation ---

/**
 * Compute the Nth occurrence date of a recurring payment.
 * count is 1-based: count=1 returns the start date itself (for simple patterns).
 */
export function computeNthOccurrenceDate(
  startDateStr: string,
  count: number,
  type: string,
  interval: number,
  weekdays?: number[][],
  monthdays?: number[],
  months?: number[]
): string | null {
  if (count < 1) return null;
  const startDate = parseDate(startDateStr);

  if (type === 'weekly' && weekdays && weekdays.length > 0 && interval <= 4) {
    return computeNthWeeklyOccurrence(startDate, count, interval, weekdays);
  }
  if (type === 'monthly' && monthdays && monthdays.length > 0 && interval === 1) {
    return computeNthMonthlyOccurrence(startDate, count, monthdays);
  }
  if (type === 'yearly' && months && months.length > 0 && interval === 1) {
    return computeNthYearlyOccurrence(startDate, count, months, monthdays);
  }

  let result: Date;
  switch (type) {
    case 'daily':
      result = addDays(startDate, (count - 1) * interval);
      break;
    case 'weekly':
      result = addDays(startDate, (count - 1) * interval * 7);
      break;
    case 'monthly':
      result = addMonths(startDate, (count - 1) * interval);
      break;
    case 'yearly':
      result = addYears(startDate, (count - 1) * interval);
      break;
    default:
      return null;
  }
  return formatDateStr(result);
}

function computeNthWeeklyOccurrence(
  startDate: Date,
  count: number,
  interval: number,
  weekdays: number[][]
): string {
  let occurrenceCount = 0;
  const currentWeekStart = new Date(startDate);
  currentWeekStart.setDate(currentWeekStart.getDate() - currentWeekStart.getDay());
  let cycleWeek = 0;

  for (let weeks = 0; weeks < 1000; weeks++) {
    const weekDays = weekdays[cycleWeek] || [];
    for (const dayOfWeek of weekDays.sort((a, b) => a - b)) {
      const occurrenceDate = addDays(currentWeekStart, dayOfWeek);
      if (occurrenceDate >= startDate) {
        occurrenceCount++;
        if (occurrenceCount === count) {
          return formatDateStr(occurrenceDate);
        }
      }
    }
    currentWeekStart.setDate(currentWeekStart.getDate() + 7);
    cycleWeek = (cycleWeek + 1) % interval;
  }
  return formatDateStr(addDays(startDate, count * 7));
}

function computeNthMonthlyOccurrence(startDate: Date, count: number, monthdays: number[]): string {
  let occurrenceCount = 0;
  let currentYear = startDate.getFullYear();
  let currentMonth = startDate.getMonth();

  for (let m = 0; m < 1000; m++) {
    const dim = daysInMonth(currentYear, currentMonth);
    for (const day of monthdays.sort((a, b) => a - b)) {
      const effectiveDay = Math.min(day, dim);
      const occurrenceDate = new Date(currentYear, currentMonth, effectiveDay, 12);
      if (occurrenceDate >= startDate) {
        occurrenceCount++;
        if (occurrenceCount === count) {
          return formatDateStr(occurrenceDate);
        }
      }
    }
    currentMonth++;
    if (currentMonth > 11) {
      currentMonth = 0;
      currentYear++;
    }
  }
  return formatDateStr(addMonths(startDate, count));
}

function computeNthYearlyOccurrence(
  startDate: Date,
  count: number,
  months: number[],
  monthdays?: number[]
): string {
  let occurrenceCount = 0;
  const daysToUse = monthdays && monthdays.length > 0 ? monthdays : [startDate.getDate()];
  let currentYear = startDate.getFullYear();

  for (let y = 0; y < 1000; y++) {
    for (const month of months.sort((a, b) => a - b)) {
      const monthIndex = month - 1;
      const dim = daysInMonth(currentYear, monthIndex);
      for (const dayOfMonth of daysToUse.sort((a, b) => a - b)) {
        const effectiveDay = Math.min(dayOfMonth, dim);
        const occurrenceDate = new Date(currentYear, monthIndex, effectiveDay, 12);
        if (occurrenceDate >= startDate) {
          occurrenceCount++;
          if (occurrenceCount === count) {
            return formatDateStr(occurrenceDate);
          }
        }
      }
    }
    currentYear++;
  }
  return formatDateStr(addYears(startDate, count));
}

// --- Range-based occurrence finders ---

/**
 * Compute the last occurrence date of a recurring payment on or before endDate.
 * Uses proper calendar arithmetic (not day approximations) for monthly/yearly.
 */
export function computeLastOccurrenceDate(
  startStr: string,
  endStr: string,
  recurrenceType: string,
  recurrenceInterval: number,
  recurrenceWeekdays?: string | null,
  recurrenceMonthdays?: string | null,
  recurrenceMonths?: string | null
): string | null {
  const start = new Date(startStr + 'T12:00:00');
  const end = new Date(endStr + 'T12:00:00');
  const intv = recurrenceInterval;

  if (end <= start) return null;

  const weekdaysArr = parseJson<number[][]>(recurrenceWeekdays ?? null);
  const monthdaysArr = parseJson<number[]>(recurrenceMonthdays ?? null);
  const monthsArr = parseJson<number[]>(recurrenceMonths ?? null);

  let last: Date = start;

  // Weekly with specific weekdays
  if (recurrenceType === 'weekly' && weekdaysArr && weekdaysArr.length > 0) {
    const weekStart = new Date(start);
    weekStart.setDate(weekStart.getDate() - weekStart.getDay());
    let cycleWeek = 0;
    for (let weeks = 0; weeks < 1000; weeks++) {
      const days = weekdaysArr[cycleWeek] || [];
      for (const dow of [...days].sort((a, b) => a - b)) {
        const occ = new Date(weekStart);
        occ.setDate(occ.getDate() + dow);
        if (occ > end) break;
        if (occ >= start) last = occ;
      }
      weekStart.setDate(weekStart.getDate() + 7);
      cycleWeek = (cycleWeek + 1) % intv;
      if (weekStart > end) break;
    }
  }
  // Monthly with specific monthdays
  else if (recurrenceType === 'monthly' && monthdaysArr && monthdaysArr.length > 0) {
    const sy = start.getFullYear();
    const sm = start.getMonth();
    for (let mo = 0; mo < 1200; mo += intv) {
      const month = (sm + mo) % 12;
      const year = sy + Math.floor((sm + mo) / 12);
      const dim = daysInMonth(year, month);
      let pastEnd = true;
      for (const day of [...monthdaysArr].sort((a, b) => a - b)) {
        const ed = Math.min(day, dim);
        const occ = new Date(year, month, ed, 12);
        if (occ > end) break;
        if (occ >= start) {
          last = occ;
          pastEnd = false;
        }
      }
      if (pastEnd && new Date(year, month, 1, 12) > end) break;
    }
  }
  // Yearly with specific months
  else if (recurrenceType === 'yearly' && monthsArr && monthsArr.length > 0) {
    const dtu = monthdaysArr && monthdaysArr.length > 0 ? monthdaysArr : [start.getDate()];
    const sy = start.getFullYear();
    for (let yo = 0; yo < 200; yo += intv) {
      const year = sy + yo;
      if (year > end.getFullYear() + 1) break;
      for (const month of [...monthsArr].sort((a, b) => a - b)) {
        const mi = month - 1;
        const dim = daysInMonth(year, mi);
        for (const day of [...dtu].sort((a, b) => a - b)) {
          const ed = Math.min(day, dim);
          const occ = new Date(year, mi, ed, 12);
          if (occ > end) break;
          if (occ >= start) last = occ;
        }
      }
    }
  }
  // Simple interval patterns
  else {
    let current: Date = new Date(start);
    for (let i = 0; i < 10000; i++) {
      let next: Date;
      switch (recurrenceType) {
        case 'daily':
          next = new Date(current);
          next.setDate(next.getDate() + intv);
          break;
        case 'weekly':
          next = new Date(current);
          next.setDate(next.getDate() + intv * 7);
          break;
        case 'monthly': {
          next = addMonths(current, intv);
          break;
        }
        case 'yearly': {
          next = addYears(current, intv);
          break;
        }
        default:
          return null;
      }
      if (next > end) break;
      last = next;
      current = next;
    }
  }

  return formatDateStr(last);
}

/**
 * Find the last occurrence that matches a recurrence pattern strictly before beforeDate.
 * Uses proper calendar arithmetic for monthly/yearly instead of day approximations.
 *
 * Caller should pass endDate + 1 day to get "on or before endDate" semantics.
 */
export function getLastPatternOccurrenceBefore(
  startDate: Date,
  beforeDate: Date,
  type: string,
  interval: number,
  weekdays?: number[][],
  monthdays?: number[],
  months?: number[]
): Date | null {
  if (beforeDate <= startDate) return null;

  // For weekly with specific weekdays
  if (type === 'weekly' && weekdays && weekdays.length > 0 && interval <= 4) {
    for (let daysBack = 1; daysBack <= 7 * interval * 2; daysBack++) {
      const checkDate = addDays(startDate, daysBetween(startDate, beforeDate) - daysBack);
      if (checkDate < startDate) break;

      const dFromStart = daysBetween(startDate, checkDate);
      const weekInCycle = Math.floor(dFromStart / 7) % interval;

      if (weekdays[weekInCycle]?.includes(checkDate.getDay())) {
        return checkDate;
      }
    }
    return null;
  }

  // For monthly (any interval, with or without specific monthdays)
  if (type === 'monthly') {
    const daysToUse =
      monthdays && monthdays.length > 0
        ? [...monthdays].sort((a, b) => b - a)
        : [startDate.getDate()];

    const startYear = startDate.getFullYear();
    const startMonth = startDate.getMonth();

    const monthsFromStart =
      (beforeDate.getFullYear() - startYear) * 12 + (beforeDate.getMonth() - startMonth);

    let offset = Math.floor(monthsFromStart / interval) * interval;

    for (; offset >= 0; offset -= interval) {
      const targetMonth = (startMonth + offset) % 12;
      const targetYear = startYear + Math.floor((startMonth + offset) / 12);
      const dim = daysInMonth(targetYear, targetMonth);

      for (const day of daysToUse) {
        const effectiveDay = Math.min(day, dim);
        const occurrenceDate = new Date(targetYear, targetMonth, effectiveDay, 12);
        if (occurrenceDate >= startDate && occurrenceDate < beforeDate) {
          return occurrenceDate;
        }
        if (occurrenceDate < startDate) {
          return null;
        }
      }
    }
    return null;
  }

  // For yearly (any interval, with or without specific months/monthdays)
  if (type === 'yearly') {
    const sortedMonths =
      months && months.length > 0 ? [...months].sort((a, b) => b - a) : [startDate.getMonth() + 1];
    const daysToUse =
      monthdays && monthdays.length > 0
        ? [...monthdays].sort((a, b) => b - a)
        : [startDate.getDate()];

    const startYear = startDate.getFullYear();
    const yearsFromStart = beforeDate.getFullYear() - startYear;
    let yearOffset = Math.floor(yearsFromStart / interval) * interval;

    for (; yearOffset >= 0; yearOffset -= interval) {
      const currentYear = startYear + yearOffset;
      for (const month of sortedMonths) {
        const monthIndex = month - 1;
        const dim = daysInMonth(currentYear, monthIndex);
        for (const dayOfMonth of daysToUse) {
          const effectiveDay = Math.min(dayOfMonth, dim);
          const occurrenceDate = new Date(currentYear, monthIndex, effectiveDay, 12);
          if (occurrenceDate >= startDate && occurrenceDate < beforeDate) {
            return occurrenceDate;
          }
          if (occurrenceDate < startDate) {
            return null;
          }
        }
      }
    }
    return null;
  }

  // Fallback: simple day-based interval (daily, weekly without patterns)
  const dayInterval = getDayInterval(type, interval);
  const dFromStart = daysBetween(startDate, beforeDate);
  if (dFromStart < 0) return null;
  const occurrences = Math.floor(dFromStart / dayInterval);
  if (occurrences < 0) return null;
  const lastOccurrence = addDays(startDate, occurrences * dayInterval);
  if (lastOccurrence >= beforeDate) {
    if (occurrences === 0) return null;
    return addDays(startDate, (occurrences - 1) * dayInterval);
  }
  return lastOccurrence;
}

/**
 * Find the first occurrence that matches a recurrence pattern on or after fromDate.
 * Uses proper calendar arithmetic for monthly/yearly with any interval.
 */
export function getFirstPatternOccurrenceFrom(
  fromDate: Date,
  type: string,
  interval: number,
  weekdays?: number[][],
  monthdays?: number[],
  months?: number[]
): Date {
  // For weekly with specific weekdays
  if (type === 'weekly' && weekdays && weekdays.length > 0 && interval <= 4) {
    for (let daysAhead = 0; daysAhead < 7 * interval; daysAhead++) {
      const checkDate = addDays(fromDate, daysAhead);
      const weekInCycle = Math.floor(daysAhead / 7) % interval;
      if (weekdays[weekInCycle]?.includes(checkDate.getDay())) {
        return checkDate;
      }
    }
    // Fallback: find the first selected day
    const allSelectedDays = weekdays.flat().sort((a, b) => a - b);
    if (allSelectedDays.length > 0) {
      const fromDayOfWeek = fromDate.getDay();
      let daysUntilFirst = (allSelectedDays[0] - fromDayOfWeek + 7) % 7;
      if (daysUntilFirst === 0) daysUntilFirst = 7;
      return addDays(fromDate, daysUntilFirst);
    }
  }

  // For monthly (any interval, with or without specific monthdays)
  if (type === 'monthly') {
    const daysToUse =
      monthdays && monthdays.length > 0
        ? [...monthdays].sort((a, b) => a - b)
        : [fromDate.getDate()];

    const startYear = fromDate.getFullYear();
    const startMonth = fromDate.getMonth();

    for (let offset = 0; offset < 1200; offset += interval) {
      const targetMonth = (startMonth + offset) % 12;
      const targetYear = startYear + Math.floor((startMonth + offset) / 12);
      const dim = daysInMonth(targetYear, targetMonth);

      for (const day of daysToUse) {
        const effectiveDay = Math.min(day, dim);
        const occurrenceDate = new Date(targetYear, targetMonth, effectiveDay, 12);
        if (occurrenceDate >= fromDate) {
          return occurrenceDate;
        }
      }
    }
  }

  // For yearly (any interval, with or without specific months/monthdays)
  if (type === 'yearly') {
    const sortedMonths =
      months && months.length > 0 ? [...months].sort((a, b) => a - b) : [fromDate.getMonth() + 1]; // 1-based
    const daysToUse =
      monthdays && monthdays.length > 0
        ? [...monthdays].sort((a, b) => a - b)
        : [fromDate.getDate()];

    const startYear = fromDate.getFullYear();

    for (let yearOffset = 0; yearOffset < 200; yearOffset += interval) {
      const currentYear = startYear + yearOffset;
      for (const month of sortedMonths) {
        const monthIndex = month - 1;
        const dim = daysInMonth(currentYear, monthIndex);
        for (const dayOfMonth of daysToUse) {
          const effectiveDay = Math.min(dayOfMonth, dim);
          const occurrenceDate = new Date(currentYear, monthIndex, effectiveDay, 12);
          if (occurrenceDate >= fromDate) {
            return occurrenceDate;
          }
        }
      }
    }
  }

  // Fallback for simple patterns or when no specific days/months selected
  return new Date(fromDate);
}

// --- Internal helpers ---

function parseJson<T>(raw: string | null): T | null {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function daysInMonth(year: number, month: number): number {
  return new Date(year, month + 1, 0).getDate();
}

function daysBetween(start: Date, end: Date): number {
  const oneDay = 24 * 60 * 60 * 1000;
  return Math.round((end.getTime() - start.getTime()) / oneDay);
}

function getDayInterval(type: string, interval: number): number {
  switch (type) {
    case 'daily':
      return interval;
    case 'weekly':
      return interval * 7;
    default:
      return 30;
  }
}
