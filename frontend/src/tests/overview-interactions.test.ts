import { describe, it, expect, beforeEach } from 'vitest';
import { SvelteSet, SvelteMap } from 'svelte/reactivity';

/**
 * Tests for expand/collapse interactions in overview page.
 *
 * These tests verify that the toggle functions work correctly with SvelteSet/SvelteMap.
 * The key pattern is that SvelteSet/SvelteMap must be mutated IN-PLACE, not reassigned,
 * because they maintain their own reactivity. Reassigning breaks the reactive binding.
 *
 * Bug context: Commit 07d9ca39589ceb introduced a regression where toggle functions
 * created new SvelteSet/SvelteMap instances and reassigned them, which broke reactivity
 * because the variables weren't wrapped in $state().
 *
 * Fix: Toggle functions now mutate in-place instead of reassigning.
 */

describe('Overview - Expand/Collapse Interactions', () => {
  // Simulating the toggle patterns used in the overview page

  describe('SvelteSet toggle pattern (expandedBalanceRows, expandedPoolEntries, etc.)', () => {
    let expandedRows: SvelteSet<number>;

    // Correct pattern: mutate in place
    function toggleRowCorrect(id: number) {
      if (expandedRows.has(id)) {
        expandedRows.delete(id);
      } else {
        expandedRows.add(id);
      }
    }

    beforeEach(() => {
      expandedRows = new SvelteSet<number>();
    });

    it('should expand a row when clicked', () => {
      expect(expandedRows.has(1)).toBe(false);
      toggleRowCorrect(1);
      expect(expandedRows.has(1)).toBe(true);
    });

    it('should collapse a row when clicked again', () => {
      expandedRows.add(1);
      expect(expandedRows.has(1)).toBe(true);
      toggleRowCorrect(1);
      expect(expandedRows.has(1)).toBe(false);
    });

    it('should handle multiple rows independently', () => {
      toggleRowCorrect(1);
      toggleRowCorrect(2);
      expect(expandedRows.has(1)).toBe(true);
      expect(expandedRows.has(2)).toBe(true);

      toggleRowCorrect(1);
      expect(expandedRows.has(1)).toBe(false);
      expect(expandedRows.has(2)).toBe(true);
    });

    it('should maintain reference identity after toggle (critical for reactivity)', () => {
      const originalRef = expandedRows;
      toggleRowCorrect(1);
      // The reference should be the same object - mutation in place
      expect(expandedRows).toBe(originalRef);
    });
  });

  describe('SvelteMap with nested SvelteSet pattern (expandedYears/Months)', () => {
    let expandedYears: SvelteSet<string>;
    let expandedMonths: SvelteMap<string, SvelteSet<string>>;

    // Correct pattern: mutate in place
    function toggleYear(year: string) {
      if (expandedYears.has(year)) {
        expandedYears.delete(year);
      } else {
        expandedYears.add(year);
      }
    }

    function toggleMonth(year: string, monthKey: string) {
      if (!expandedMonths.has(year)) {
        expandedMonths.set(year, new SvelteSet());
      }
      const monthSet = expandedMonths.get(year)!;
      if (monthSet.has(monthKey)) {
        monthSet.delete(monthKey);
      } else {
        monthSet.add(monthKey);
      }
    }

    function isYearExpanded(year: string): boolean {
      return expandedYears.has(year);
    }

    function isMonthExpanded(year: string, monthKey: string): boolean {
      return expandedMonths.get(year)?.has(monthKey) ?? false;
    }

    beforeEach(() => {
      expandedYears = new SvelteSet<string>();
      expandedMonths = new SvelteMap<string, SvelteSet<string>>();
    });

    it('should expand a year when clicked', () => {
      expect(isYearExpanded('2024')).toBe(false);
      toggleYear('2024');
      expect(isYearExpanded('2024')).toBe(true);
    });

    it('should collapse a year when clicked again', () => {
      toggleYear('2024');
      toggleYear('2024');
      expect(isYearExpanded('2024')).toBe(false);
    });

    it('should expand a month within a year', () => {
      expect(isMonthExpanded('2024', '2024-01')).toBe(false);
      toggleMonth('2024', '2024-01');
      expect(isMonthExpanded('2024', '2024-01')).toBe(true);
    });

    it('should collapse a month when clicked again', () => {
      toggleMonth('2024', '2024-01');
      toggleMonth('2024', '2024-01');
      expect(isMonthExpanded('2024', '2024-01')).toBe(false);
    });

    it('should handle multiple months in different years', () => {
      toggleMonth('2024', '2024-01');
      toggleMonth('2024', '2024-02');
      toggleMonth('2023', '2023-12');

      expect(isMonthExpanded('2024', '2024-01')).toBe(true);
      expect(isMonthExpanded('2024', '2024-02')).toBe(true);
      expect(isMonthExpanded('2023', '2023-12')).toBe(true);
      expect(isMonthExpanded('2023', '2023-01')).toBe(false);
    });

    it('should maintain reference identity for years after toggle', () => {
      const originalRef = expandedYears;
      toggleYear('2024');
      expect(expandedYears).toBe(originalRef);
    });

    it('should maintain reference identity for months map after toggle', () => {
      const originalRef = expandedMonths;
      toggleMonth('2024', '2024-01');
      expect(expandedMonths).toBe(originalRef);
    });
  });

  describe('Pairwise expand pattern (expandedPairwise)', () => {
    let expandedPairwise: SvelteSet<string>;
    let expandedPairwiseYears: SvelteMap<string, SvelteSet<string>>;

    function togglePairwiseRow(participantId: number, otherId: number) {
      const key = `${participantId}-${otherId}`;
      if (expandedPairwise.has(key)) {
        expandedPairwise.delete(key);
      } else {
        expandedPairwise.add(key);
      }
    }

    function isPairwiseExpanded(participantId: number, otherId: number): boolean {
      return expandedPairwise.has(`${participantId}-${otherId}`);
    }

    function togglePairwiseYear(pairwiseKey: string, year: string) {
      if (!expandedPairwiseYears.has(pairwiseKey)) {
        expandedPairwiseYears.set(pairwiseKey, new SvelteSet());
      }
      const yearSet = expandedPairwiseYears.get(pairwiseKey)!;
      if (yearSet.has(year)) {
        yearSet.delete(year);
      } else {
        yearSet.add(year);
      }
    }

    function isPairwiseYearExpanded(pairwiseKey: string, year: string): boolean {
      return expandedPairwiseYears.get(pairwiseKey)?.has(year) ?? false;
    }

    beforeEach(() => {
      expandedPairwise = new SvelteSet<string>();
      expandedPairwiseYears = new SvelteMap<string, SvelteSet<string>>();
    });

    it('should expand pairwise row when participant name is clicked', () => {
      expect(isPairwiseExpanded(1, 2)).toBe(false);
      togglePairwiseRow(1, 2);
      expect(isPairwiseExpanded(1, 2)).toBe(true);
    });

    it('should collapse pairwise row when clicked again', () => {
      togglePairwiseRow(1, 2);
      togglePairwiseRow(1, 2);
      expect(isPairwiseExpanded(1, 2)).toBe(false);
    });

    it('should track different participant pairs independently', () => {
      togglePairwiseRow(1, 2);
      togglePairwiseRow(1, 3);
      togglePairwiseRow(2, 3);

      expect(isPairwiseExpanded(1, 2)).toBe(true);
      expect(isPairwiseExpanded(1, 3)).toBe(true);
      expect(isPairwiseExpanded(2, 3)).toBe(true);
      expect(isPairwiseExpanded(2, 1)).toBe(false); // Different key
    });

    it('should expand year within pairwise breakdown', () => {
      const pairwiseKey = '1-2';
      expect(isPairwiseYearExpanded(pairwiseKey, '2024')).toBe(false);
      togglePairwiseYear(pairwiseKey, '2024');
      expect(isPairwiseYearExpanded(pairwiseKey, '2024')).toBe(true);
    });

    it('should maintain reference identity after pairwise toggle', () => {
      const originalRef = expandedPairwise;
      togglePairwiseRow(1, 2);
      expect(expandedPairwise).toBe(originalRef);
    });
  });

  describe('Pool entry expand pattern', () => {
    let expandedPoolEntries: SvelteSet<number>;
    let expandedPoolYears: SvelteMap<number, SvelteSet<string>>;
    let expandedPoolMonths: SvelteMap<number, SvelteMap<string, SvelteSet<string>>>;

    function togglePoolEntry(participantId: number) {
      if (expandedPoolEntries.has(participantId)) {
        expandedPoolEntries.delete(participantId);
      } else {
        expandedPoolEntries.add(participantId);
      }
    }

    function togglePoolYear(participantId: number, year: string) {
      if (!expandedPoolYears.has(participantId)) {
        expandedPoolYears.set(participantId, new SvelteSet());
      }
      const yearSet = expandedPoolYears.get(participantId)!;
      if (yearSet.has(year)) {
        yearSet.delete(year);
      } else {
        yearSet.add(year);
      }
    }

    function togglePoolMonth(participantId: number, year: string, monthKey: string) {
      if (!expandedPoolMonths.has(participantId)) {
        expandedPoolMonths.set(participantId, new SvelteMap());
      }
      const yearMap = expandedPoolMonths.get(participantId)!;
      if (!yearMap.has(year)) {
        yearMap.set(year, new SvelteSet());
      }
      const monthSet = yearMap.get(year)!;
      if (monthSet.has(monthKey)) {
        monthSet.delete(monthKey);
      } else {
        monthSet.add(monthKey);
      }
    }

    function isPoolEntryExpanded(participantId: number): boolean {
      return expandedPoolEntries.has(participantId);
    }

    function isPoolYearExpanded(participantId: number, year: string): boolean {
      return expandedPoolYears.get(participantId)?.has(year) ?? false;
    }

    function isPoolMonthExpanded(participantId: number, year: string, monthKey: string): boolean {
      return expandedPoolMonths.get(participantId)?.get(year)?.has(monthKey) ?? false;
    }

    beforeEach(() => {
      expandedPoolEntries = new SvelteSet<number>();
      expandedPoolYears = new SvelteMap<number, SvelteSet<string>>();
      expandedPoolMonths = new SvelteMap<number, SvelteMap<string, SvelteSet<string>>>();
    });

    it('should expand pool entry when participant name is clicked', () => {
      expect(isPoolEntryExpanded(1)).toBe(false);
      togglePoolEntry(1);
      expect(isPoolEntryExpanded(1)).toBe(true);
    });

    it('should expand pool year within participant', () => {
      togglePoolYear(1, '2024');
      expect(isPoolYearExpanded(1, '2024')).toBe(true);
      expect(isPoolYearExpanded(2, '2024')).toBe(false);
    });

    it('should expand pool month within participant and year', () => {
      togglePoolMonth(1, '2024', '2024-01');
      expect(isPoolMonthExpanded(1, '2024', '2024-01')).toBe(true);
      expect(isPoolMonthExpanded(1, '2024', '2024-02')).toBe(false);
    });

    it('should maintain reference identity after pool toggles', () => {
      const entriesRef = expandedPoolEntries;
      const yearsRef = expandedPoolYears;
      const monthsRef = expandedPoolMonths;

      togglePoolEntry(1);
      togglePoolYear(1, '2024');
      togglePoolMonth(1, '2024', '2024-01');

      expect(expandedPoolEntries).toBe(entriesRef);
      expect(expandedPoolYears).toBe(yearsRef);
      expect(expandedPoolMonths).toBe(monthsRef);
    });
  });
});
