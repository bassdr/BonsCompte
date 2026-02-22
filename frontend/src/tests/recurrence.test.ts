import { describe, it, expect } from 'vitest';
import {
  computeLastOccurrenceDate,
  computeNthOccurrenceDate,
  getLastPatternOccurrenceBefore,
  getFirstPatternOccurrenceFrom
} from '$lib/recurrence-utils';

/**
 * Tests for shared recurrence occurrence utilities.
 * These test the actual functions used by TransactionCard and the transactions page.
 */

function fmt(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

function date(s: string): Date {
  return new Date(s + 'T12:00:00');
}

describe('computeLastOccurrenceDate', () => {
  describe('monthly every 2 months', () => {
    it('computes last occurrence when end date falls between occurrences', () => {
      // Start April 20, every 2 months, end August 22
      // Occurrences: April 20, June 20, August 20
      expect(computeLastOccurrenceDate('2025-04-20', '2025-08-22', 'monthly', 2)).toBe(
        '2025-08-20'
      );
    });

    it('returns start date when end date is before second occurrence', () => {
      expect(computeLastOccurrenceDate('2025-04-20', '2025-05-30', 'monthly', 2)).toBe(
        '2025-04-20'
      );
    });

    it('includes end date when it falls exactly on an occurrence', () => {
      expect(computeLastOccurrenceDate('2025-04-20', '2025-08-20', 'monthly', 2)).toBe(
        '2025-08-20'
      );
    });

    it('stops just before end date when end is one day before occurrence', () => {
      expect(computeLastOccurrenceDate('2025-04-20', '2025-08-19', 'monthly', 2)).toBe(
        '2025-06-20'
      );
    });
  });

  describe('monthly every 3 months', () => {
    it('computes quarterly occurrences correctly', () => {
      // Occurrences: Jan 15, Apr 15, Jul 15, Oct 15
      expect(computeLastOccurrenceDate('2025-01-15', '2025-12-31', 'monthly', 3)).toBe(
        '2025-10-15'
      );
    });
  });

  describe('monthly interval 1', () => {
    it('computes with end date between months', () => {
      expect(computeLastOccurrenceDate('2025-01-01', '2025-03-15', 'monthly', 1)).toBe(
        '2025-03-01'
      );
    });
  });

  describe('weekly every 2 weeks', () => {
    it('computes biweekly last occurrence', () => {
      // Jan 6, Jan 20, Feb 3
      expect(computeLastOccurrenceDate('2025-01-06', '2025-02-03', 'weekly', 2)).toBe('2025-02-03');
    });

    it('excludes occurrence after end date', () => {
      expect(computeLastOccurrenceDate('2025-01-06', '2025-02-02', 'weekly', 2)).toBe('2025-01-20');
    });
  });

  describe('daily every 3 days', () => {
    it('computes every-3-day last occurrence', () => {
      // Jan 1, Jan 4, Jan 7, Jan 10
      expect(computeLastOccurrenceDate('2025-01-01', '2025-01-10', 'daily', 3)).toBe('2025-01-10');
    });

    it('does not include occurrence past end date', () => {
      expect(computeLastOccurrenceDate('2025-01-01', '2025-01-09', 'daily', 3)).toBe('2025-01-07');
    });
  });

  describe('yearly every 2 years', () => {
    it('computes biennial last occurrence', () => {
      // 2024-06-15, 2026-06-15, 2028-06-15
      expect(computeLastOccurrenceDate('2024-06-15', '2030-01-01', 'yearly', 2)).toBe('2028-06-15');
    });
  });

  describe('edge cases', () => {
    it('returns null when end equals start', () => {
      expect(computeLastOccurrenceDate('2025-01-01', '2025-01-01', 'monthly', 1)).toBeNull();
    });

    it('returns start date when only one occurrence fits', () => {
      expect(computeLastOccurrenceDate('2025-01-01', '2025-01-15', 'monthly', 1)).toBe(
        '2025-01-01'
      );
    });
  });

  describe('with pattern data (JSON strings)', () => {
    it('handles monthly with specific monthdays', () => {
      // Monthly on 5th and 20th, end March 10
      // Occurrences: Jan 5, Jan 20, Feb 5, Feb 20, Mar 5
      expect(
        computeLastOccurrenceDate('2025-01-01', '2025-03-10', 'monthly', 1, null, '[5, 20]')
      ).toBe('2025-03-05');
    });
  });
});

describe('getLastPatternOccurrenceBefore', () => {
  describe('monthly every 2 months', () => {
    it('uses calendar arithmetic, not day approximation (the bug)', () => {
      // The bug: 30*2=60 days approximation gave August 18 instead of August 20
      // Pass endDate + 1 day to get "on or before" semantics
      const result = getLastPatternOccurrenceBefore(
        date('2025-04-20'),
        date('2025-08-23'), // Aug 22 + 1 day
        'monthly',
        2
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-08-20');
    });

    it('finds exact match when before date is day after occurrence', () => {
      const result = getLastPatternOccurrenceBefore(
        date('2025-04-20'),
        date('2025-08-21'), // Aug 20 + 1 day
        'monthly',
        2
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-08-20');
    });

    it('finds previous occurrence when end is before next', () => {
      const result = getLastPatternOccurrenceBefore(
        date('2025-04-20'),
        date('2025-08-20'), // Strictly before Aug 20 → June 20
        'monthly',
        2
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-06-20');
    });
  });

  describe('monthly every 3 months', () => {
    it('correctly finds quarterly last occurrence', () => {
      const result = getLastPatternOccurrenceBefore(
        date('2025-01-15'),
        date('2025-12-01'),
        'monthly',
        3
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-10-15');
    });
  });

  describe('monthly interval 1', () => {
    it('handles simple monthly', () => {
      const result = getLastPatternOccurrenceBefore(
        date('2025-01-01'),
        date('2025-03-16'), // +1 day
        'monthly',
        1
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-03-01');
    });

    it('handles monthly with specific monthdays', () => {
      // Monthly on 10th and 25th, find last before March 15
      const result = getLastPatternOccurrenceBefore(
        date('2025-01-01'),
        date('2025-03-15'),
        'monthly',
        1,
        undefined,
        [10, 25]
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-03-10');
    });
  });

  describe('yearly every 2 years', () => {
    it('finds biennial last occurrence', () => {
      const result = getLastPatternOccurrenceBefore(
        date('2024-06-15'),
        date('2030-01-01'),
        'yearly',
        2
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2028-06-15');
    });
  });

  describe('returns null', () => {
    it('returns null when beforeDate <= startDate', () => {
      expect(
        getLastPatternOccurrenceBefore(date('2025-04-20'), date('2025-04-20'), 'monthly', 2)
      ).toBeNull();
    });

    it('returns start date when beforeDate is just after start', () => {
      // April 20 is the start and a valid occurrence, before April 21
      const result = getLastPatternOccurrenceBefore(
        date('2025-04-20'),
        date('2025-04-21'),
        'monthly',
        2
      );
      expect(result).not.toBeNull();
      expect(fmt(result!)).toBe('2025-04-20');
    });
  });
});

describe('getFirstPatternOccurrenceFrom', () => {
  describe('monthly every 2 months', () => {
    it('returns fromDate when it matches the pattern', () => {
      // Start April 20, every 2 months - April 20 is the first occurrence
      const result = getFirstPatternOccurrenceFrom(date('2025-04-20'), 'monthly', 2);
      expect(fmt(result)).toBe('2025-04-20');
    });

    it('returns fromDate for simple monthly (day matches)', () => {
      const result = getFirstPatternOccurrenceFrom(date('2025-01-15'), 'monthly', 3);
      expect(fmt(result)).toBe('2025-01-15');
    });
  });

  describe('monthly with specific monthdays', () => {
    it('finds first monthday on or after fromDate', () => {
      // From Jan 10, monthly on 5th and 20th → first is Jan 20
      const result = getFirstPatternOccurrenceFrom(
        date('2025-01-10'),
        'monthly',
        1,
        undefined,
        [5, 20]
      );
      expect(fmt(result)).toBe('2025-01-20');
    });

    it('returns fromDate when it falls on a monthday', () => {
      const result = getFirstPatternOccurrenceFrom(
        date('2025-01-05'),
        'monthly',
        1,
        undefined,
        [5, 20]
      );
      expect(fmt(result)).toBe('2025-01-05');
    });

    it('goes to next month when all monthdays have passed', () => {
      // From Jan 25, monthly on 5th and 20th → first is Feb 5
      const result = getFirstPatternOccurrenceFrom(
        date('2025-01-25'),
        'monthly',
        1,
        undefined,
        [5, 20]
      );
      expect(fmt(result)).toBe('2025-02-05');
    });

    it('handles monthdays with interval > 1', () => {
      // From Jan 25, every 2 months on 5th and 20th
      // Jan already passed, next valid month is March (offset 2)
      const result = getFirstPatternOccurrenceFrom(
        date('2025-01-25'),
        'monthly',
        2,
        undefined,
        [5, 20]
      );
      expect(fmt(result)).toBe('2025-03-05');
    });
  });

  describe('yearly', () => {
    it('returns fromDate when it matches', () => {
      const result = getFirstPatternOccurrenceFrom(date('2025-06-15'), 'yearly', 1);
      expect(fmt(result)).toBe('2025-06-15');
    });

    it('finds first matching year for every-2-years', () => {
      const result = getFirstPatternOccurrenceFrom(date('2025-06-15'), 'yearly', 2);
      expect(fmt(result)).toBe('2025-06-15');
    });

    it('finds first occurrence with specific months', () => {
      // From March 1, yearly in June and December
      const result = getFirstPatternOccurrenceFrom(
        date('2025-03-01'),
        'yearly',
        1,
        undefined,
        undefined,
        [6, 12]
      );
      expect(fmt(result)).toBe('2025-06-01');
    });
  });

  describe('daily', () => {
    it('returns fromDate for daily pattern', () => {
      const result = getFirstPatternOccurrenceFrom(date('2025-01-01'), 'daily', 3);
      expect(fmt(result)).toBe('2025-01-01');
    });
  });
});

describe('computeNthOccurrenceDate', () => {
  describe('monthly', () => {
    it('returns start date for count=1', () => {
      expect(computeNthOccurrenceDate('2025-04-20', 1, 'monthly', 2)).toBe('2025-04-20');
    });

    it('computes 2nd occurrence for every 2 months', () => {
      expect(computeNthOccurrenceDate('2025-04-20', 2, 'monthly', 2)).toBe('2025-06-20');
    });

    it('computes 3rd occurrence for every 2 months', () => {
      expect(computeNthOccurrenceDate('2025-04-20', 3, 'monthly', 2)).toBe('2025-08-20');
    });

    it('computes quarterly occurrences', () => {
      expect(computeNthOccurrenceDate('2025-01-15', 4, 'monthly', 3)).toBe('2025-10-15');
    });

    it('handles month-end clamping', () => {
      // Jan 31 + 1 month = Feb 28
      expect(computeNthOccurrenceDate('2025-01-31', 2, 'monthly', 1)).toBe('2025-02-28');
    });
  });

  describe('monthly with specific monthdays', () => {
    it('counts occurrences across months', () => {
      // Start Jan 1, monthly on 5th and 20th
      // Occurrences: Jan 5, Jan 20, Feb 5, Feb 20, Mar 5
      expect(computeNthOccurrenceDate('2025-01-01', 5, 'monthly', 1, undefined, [5, 20])).toBe(
        '2025-03-05'
      );
    });
  });

  describe('weekly', () => {
    it('computes biweekly Nth occurrence', () => {
      // Jan 6, Jan 20, Feb 3
      expect(computeNthOccurrenceDate('2025-01-06', 3, 'weekly', 2)).toBe('2025-02-03');
    });
  });

  describe('daily', () => {
    it('computes every-3-day Nth occurrence', () => {
      // Jan 1, Jan 4, Jan 7, Jan 10
      expect(computeNthOccurrenceDate('2025-01-01', 4, 'daily', 3)).toBe('2025-01-10');
    });
  });

  describe('yearly', () => {
    it('computes biennial Nth occurrence', () => {
      // 2024, 2026, 2028
      expect(computeNthOccurrenceDate('2024-06-15', 3, 'yearly', 2)).toBe('2028-06-15');
    });
  });

  describe('edge cases', () => {
    it('returns null for count < 1', () => {
      expect(computeNthOccurrenceDate('2025-01-01', 0, 'monthly', 1)).toBeNull();
    });

    it('returns null for invalid type', () => {
      expect(computeNthOccurrenceDate('2025-01-01', 1, 'invalid', 1)).toBeNull();
    });
  });
});
