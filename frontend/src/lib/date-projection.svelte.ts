/**
 * Shared reactive state factory for Cashflow and Reconciliation pages.
 * Uses Svelte 5 runes ($state, $derived, $effect).
 */

import { SvelteSet, SvelteMap } from 'svelte/reactivity';
import { getDebts, getPayments } from '$lib/api';
import type { DebtSummary, PaymentWithContributions } from '$lib/api';
import { getErrorKey } from '$lib/errors';
import {
  getLocalDateString,
  parseLocalDate,
  getHorizonEndDate,
  getNextRecurringDate,
  type HorizonOption
} from './date-projection';
import { formatDateWithWeekday, type DateFormatType } from '$lib/format/date';
import { preferences } from '$lib/stores/preferences';
import { get } from 'svelte/store';

export function createOverviewState() {
  // ─── Core state ──────────────────────────────────────────────────────────

  let debts: DebtSummary | null = $state(null);
  let startDebts: DebtSummary | null = $state(null);
  let allPayments: PaymentWithContributions[] = $state([]);
  let loading = $state(true);
  let errorKey = $state('');

  // Focus mode
  let focusParticipantId = $state<number | null>(null);

  // Include draft payments
  let includeDrafts = $state(false);

  // Date state
  let targetDate = $state(getLocalDateString());
  let endDate = $state<string | null>(null);
  let chosenHorizon = $state<HorizonOption>('none');

  // ─── Date pickers disabled state ─────────────────────────────────────────

  const fromDateDisabledButtons = $derived.by((): 'today'[] => {
    const today = getLocalDateString();
    return targetDate === today ? ['today'] : [];
  });

  const fromDateDisabledReasons = $derived.by((): Record<string, string> => {
    const today = getLocalDateString();
    if (targetDate === today) {
      return { today: 'Already set to today' };
    }
    return {};
  });

  const toDateDisabledButtons = $derived.by((): 'today'[] => {
    const today = getLocalDateString();
    const fromDate = parseLocalDate(targetDate);
    const todayDate = parseLocalDate(today);
    if (fromDate >= todayDate || endDate === today) {
      return ['today'];
    }
    return [];
  });

  const toDateDisabledReasons = $derived.by((): Record<string, string> => {
    const today = getLocalDateString();
    const fromDate = parseLocalDate(targetDate);
    const todayDate = parseLocalDate(today);
    if (endDate === today) {
      return { today: 'Already set to today' };
    }
    if (fromDate >= todayDate) {
      return { today: 'From date is today or later' };
    }
    return {};
  });

  // ─── Horizon options ─────────────────────────────────────────────────────

  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const now = new Date();
  const horizonThisMonth = $derived(
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 1, 0))
  );
  const horizonNextMonth = $derived(
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    getLocalDateString(new Date(now.getFullYear(), now.getMonth() + 2, 0))
  );
  const horizon3Months = $derived(getHorizonEndDate(targetDate, 3));
  const horizon6Months = $derived(getHorizonEndDate(targetDate, 6));
  const horizon12Months = $derived(getHorizonEndDate(targetDate, 12));

  const selectedHorizon = $derived.by((): HorizonOption => {
    if (endDate === null) return 'none';
    if (endDate === horizonThisMonth) return 'thisMonth';
    if (endDate === horizonNextMonth) return 'nextMonth';
    if (endDate === horizon3Months) return '3months';
    if (endDate === horizon6Months) return '6months';
    if (endDate === horizon12Months) return '12months';
    return 'custom';
  });

  function setHorizon(option: HorizonOption) {
    chosenHorizon = option;
    switch (option) {
      case 'none':
        endDate = null;
        break;
      case 'thisMonth':
        endDate = horizonThisMonth;
        break;
      case 'nextMonth':
        endDate = horizonNextMonth;
        break;
      case '3months':
        endDate = horizon3Months;
        break;
      case '6months':
        endDate = horizon6Months;
        break;
      case '12months':
        endDate = horizon12Months;
        break;
    }
  }

  // Keep endDate in sync with preset horizons when targetDate changes
  $effect(() => {
    if (chosenHorizon === 'thisMonth') {
      endDate = horizonThisMonth;
    } else if (chosenHorizon === 'nextMonth') {
      endDate = horizonNextMonth;
    } else if (chosenHorizon === '3months') {
      endDate = horizon3Months;
    } else if (chosenHorizon === '6months') {
      endDate = horizon6Months;
    } else if (chosenHorizon === '12months') {
      endDate = horizon12Months;
    }
  });

  // Reset horizon to None if startDate >= endDate
  $effect(() => {
    if (endDate !== null && targetDate >= endDate) {
      endDate = null;
      chosenHorizon = 'none';
    }
  });

  const isRangeMode = $derived(endDate !== null && endDate > targetDate);

  // ─── Derived data ────────────────────────────────────────────────────────

  const paymentMap = $derived.by(() => {
    const map = new SvelteMap<number, PaymentWithContributions>();
    for (const payment of allPayments) {
      map.set(payment.id, payment);
    }
    return map;
  });

  const poolIds = $derived.by(() => {
    if (!debts?.pool_ownerships) return new SvelteSet<number>();
    return new SvelteSet(debts.pool_ownerships.map((p) => p.pool_id));
  });

  const nonPoolBalances = $derived.by(() => {
    if (!debts) return [];
    return debts.balances.filter((b) => !poolIds.has(b.participant_id));
  });

  const todayStr = $derived(getLocalDateString());

  // ─── Navigation ──────────────────────────────────────────────────────────

  const paymentDates = $derived.by(() => {
    if (!debts?.occurrences) return [];
    const dates = new SvelteSet(debts.occurrences.map((o) => o.occurrence_date));
    dates.add(todayStr);
    return [...dates].sort();
  });

  const previousPaymentDate = $derived.by(() => {
    const currentDates = paymentDates.filter((d) => d < targetDate);
    return currentDates.length > 0 ? currentDates[currentDates.length - 1] : null;
  });

  const nextPaymentDate = $derived.by(() => {
    if (allPayments.length === 0) return null;

    const targetDateObj = parseLocalDate(targetDate);
    const nextDates: string[] = [];

    for (const payment of allPayments) {
      const paymentDateStr = payment.payment_date.split('T')[0];

      if (!payment.is_recurring) {
        if (paymentDateStr > targetDate) {
          nextDates.push(paymentDateStr);
        }
      } else {
        const nextOccurrence = getNextRecurringDate(payment, targetDateObj);
        if (nextOccurrence) {
          nextDates.push(nextOccurrence);
        }
      }
    }

    if (nextDates.length === 0) return null;
    nextDates.sort();
    return nextDates[0];
  });

  const previousEndPaymentDate = $derived.by(() => {
    if (!endDate || !paymentDates.length) return null;
    const currentEndDate = endDate;
    const validDates = paymentDates.filter((d) => d < currentEndDate && d > targetDate);
    return validDates.length > 0 ? validDates[validDates.length - 1] : null;
  });

  const nextEndPaymentDate = $derived.by(() => {
    if (!endDate || allPayments.length === 0) return null;

    const endDateObj = parseLocalDate(endDate);
    const nextDates: string[] = [];

    for (const payment of allPayments) {
      const paymentDateStr = payment.payment_date.split('T')[0];

      if (!payment.is_recurring) {
        if (paymentDateStr > endDate) {
          nextDates.push(paymentDateStr);
        }
      } else {
        const nextOccurrence = getNextRecurringDate(payment, endDateObj);
        if (nextOccurrence) {
          nextDates.push(nextOccurrence);
        }
      }
    }

    if (nextDates.length === 0) return null;
    nextDates.sort();
    return nextDates[0];
  });

  function goToPreviousPayment() {
    if (previousPaymentDate) {
      targetDate = previousPaymentDate;
    }
  }

  function goToNextPayment() {
    if (nextPaymentDate) {
      targetDate = nextPaymentDate;
    }
  }

  function goToPreviousEndPayment() {
    if (previousEndPaymentDate) {
      endDate = previousEndPaymentDate;
      chosenHorizon = 'custom';
    }
  }

  function goToNextEndPayment() {
    if (nextEndPaymentDate) {
      endDate = nextEndPaymentDate;
      chosenHorizon = 'custom';
    }
  }

  // ─── Date formatting ─────────────────────────────────────────────────────

  const dateFormat = $derived(get(preferences).date_format as DateFormatType);

  function formatDate(dateStr: string): string {
    return formatDateWithWeekday(dateStr, dateFormat);
  }

  // ─── Range dates ─────────────────────────────────────────────────────────

  const rangeDates = $derived.by(() => {
    if (!isRangeMode || !debts || !endDate) return [];
    const end = endDate;
    return paymentDates.filter((d) => d >= targetDate && d <= end);
  });

  // ─── Data loading ────────────────────────────────────────────────────────

  async function loadDebts(projectId: number) {
    loading = true;
    errorKey = '';
    try {
      const dateToLoad = isRangeMode && endDate ? endDate : targetDate;
      debts = await getDebts(projectId, dateToLoad, includeDrafts);

      if (isRangeMode) {
        startDebts = await getDebts(projectId, targetDate, includeDrafts);
      } else {
        startDebts = null;
      }

      allPayments = await getPayments(projectId);
    } catch (e) {
      errorKey = getErrorKey(e, 'overview.failedToLoad');
    } finally {
      loading = false;
    }
  }

  // ─── Return public API ───────────────────────────────────────────────────

  return {
    // Core data (getters for reactive access)
    get debts() {
      return debts;
    },
    get startDebts() {
      return startDebts;
    },
    get allPayments() {
      return allPayments;
    },
    get loading() {
      return loading;
    },
    get errorKey() {
      return errorKey;
    },

    // Filter state
    get focusParticipantId() {
      return focusParticipantId;
    },
    set focusParticipantId(v: number | null) {
      focusParticipantId = v;
    },
    get includeDrafts() {
      return includeDrafts;
    },
    set includeDrafts(v: boolean) {
      includeDrafts = v;
    },

    // Date state
    get targetDate() {
      return targetDate;
    },
    set targetDate(v: string) {
      targetDate = v;
    },
    get endDate() {
      return endDate;
    },
    set endDate(v: string | null) {
      endDate = v;
    },
    get chosenHorizon() {
      return chosenHorizon;
    },
    set chosenHorizon(v: HorizonOption) {
      chosenHorizon = v;
    },
    get isRangeMode() {
      return isRangeMode;
    },

    // Date picker helpers
    get fromDateDisabledButtons() {
      return fromDateDisabledButtons;
    },
    get fromDateDisabledReasons() {
      return fromDateDisabledReasons;
    },
    get toDateDisabledButtons() {
      return toDateDisabledButtons;
    },
    get toDateDisabledReasons() {
      return toDateDisabledReasons;
    },

    // Horizon
    get selectedHorizon() {
      return selectedHorizon;
    },
    setHorizon,

    // Derived data
    get paymentMap() {
      return paymentMap;
    },
    get poolIds() {
      return poolIds;
    },
    get nonPoolBalances() {
      return nonPoolBalances;
    },
    get todayStr() {
      return todayStr;
    },
    get rangeDates() {
      return rangeDates;
    },
    get dateFormat() {
      return dateFormat;
    },

    // Navigation
    get paymentDates() {
      return paymentDates;
    },
    get previousPaymentDate() {
      return previousPaymentDate;
    },
    get nextPaymentDate() {
      return nextPaymentDate;
    },
    get previousEndPaymentDate() {
      return previousEndPaymentDate;
    },
    get nextEndPaymentDate() {
      return nextEndPaymentDate;
    },
    goToPreviousPayment,
    goToNextPayment,
    goToPreviousEndPayment,
    goToNextEndPayment,

    // Formatting
    formatDate,

    // Data loading
    loadDebts
  };
}

export type OverviewState = ReturnType<typeof createOverviewState>;
