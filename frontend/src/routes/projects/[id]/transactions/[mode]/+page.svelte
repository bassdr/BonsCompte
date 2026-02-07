<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import {
    getPayment,
    createPayment,
    updatePayment,
    type PaymentWithContributions,
    type CreatePaymentInput
  } from '$lib/api';
  import { participants, canEdit, members } from '$lib/stores/project';
  import { auth } from '$lib/auth';
  import { _ } from '$lib/i18n';
  import { formatCurrency } from '$lib/format/currency';
  import { SvelteDate } from 'svelte/reactivity';
  import { getErrorKey } from '$lib/errors';
  import DateInput from '$lib/components/DateInput.svelte';

  // Mode from URL params
  let mode = $derived($page.params.mode as 'outgoing' | 'incoming' | 'internal' | 'rule');
  let isValidMode = $derived(['outgoing', 'incoming', 'internal', 'rule'].includes(mode));

  // Edit ID from URL params
  let editId = $derived.by(() => {
    const id = $page.url.searchParams.get('edit');
    return id ? parseInt(id) : null;
  });

  // Get project ID from URL
  let projectId = $derived(parseInt($page.params.id ?? ''));

  let loading = $state(true);
  let errorKey = $state('');

  // Edit mode state
  let editingPaymentId = $state<number | null>(null);
  let editingPaymentOriginal = $state<PaymentWithContributions | null>(null);

  // Split date option for recurring payments
  let useSplitDate = $state(false);
  let splitFromDate = $state(getLocalDateString());

  // Form state
  let amount = $state('');
  let description = $state('');
  let payerId = $state<number | null>(null);
  let paymentDate = $state(getLocalDateString());
  let submitting = $state(false);

  // Receipt image state
  let receiptImage = $state<string | null>(null);
  let receiptPreview = $state<string | null>(null);
  let receiptInputEl = $state<HTMLInputElement | undefined>(undefined);

  // Recurrence state
  let isRecurring = $state(false);
  let recurrenceInterval = $state(1);
  let recurrenceType = $state<'daily' | 'weekly' | 'monthly' | 'yearly'>('monthly');
  let recurrenceEndDate = $state('');
  let recurrenceCount = $state<number | null>(null);

  // Enhanced recurrence patterns
  let recurrenceWeekdays = $state<number[][]>([]);
  let recurrenceMonthdays = $state<number[]>([]);
  let recurrenceMonths = $state<number[]>([]);

  // Compute buttons for date pickers - no Clear (mandatory), Today only if not already today
  let paymentDateButtons = $derived.by((): 'today'[] => {
    const today = getLocalDateString();
    return paymentDate === today ? [] : ['today'];
  });

  // Recurrence end date buttons - keep Clear (optional), Today only if not already today and paymentDate is in past
  let recurrenceEndDateButtons = $derived.by((): ('clear' | 'today')[] => {
    const today = getLocalDateString();
    // Hide Today if recurrenceEndDate is already today OR if paymentDate is today or future
    if (recurrenceEndDate === today || paymentDate >= today) {
      return ['clear'];
    }
    return ['clear', 'today'];
  });

  // Split-from-date buttons - no Clear (mandatory), Today only if within min/max range and not already today
  let splitFromDateButtons = $derived.by((): 'today'[] => {
    if (!editingPaymentOriginal) return [];
    const today = getLocalDateString();
    if (splitFromDate === today) return [];
    const minDate = editingPaymentOriginal.payment_date.split('T')[0];
    const maxDate = editingPaymentOriginal.recurrence_end_date?.split('T')[0] || '';
    // Check if today is within range
    if (today >= minDate && (maxDate === '' || today <= maxDate)) {
      return ['today'];
    }
    return [];
  });

  // Payment finalization status: true = final (default), false = draft
  let isFinal = $state(true);

  // Pool expectation toggles (UI-friendly names)
  // isPayerApproved: When payer is a pool, "Approved" withdrawals reduce expected minimum
  let isPayerApproved = $state(false);
  // isReceiverEarmarked: When receiver is a pool, "Earmarked" deposits increase expected minimum
  let isReceiverEarmarked = $state(false);

  // Track if user has manually modified recurrence selections
  let userModifiedWeekdays = $state(false);
  let userModifiedMonthdays = $state(false);
  let userModifiedMonths = $state(false);

  // Pre-computed i18n labels
  let weekLabel = $derived($_('transactions.week'));

  // Day and month names for display
  let WEEKDAY_NAMES = $derived([
    $_('weekdays.sun'),
    $_('weekdays.mon'),
    $_('weekdays.tue'),
    $_('weekdays.wed'),
    $_('weekdays.thu'),
    $_('weekdays.fri'),
    $_('weekdays.sat')
  ]);
  let MONTH_NAMES = $derived([
    $_('months.jan'),
    $_('months.feb'),
    $_('months.mar'),
    $_('months.apr'),
    $_('months.may'),
    $_('months.jun'),
    $_('months.jul'),
    $_('months.aug'),
    $_('months.sep'),
    $_('months.oct'),
    $_('months.nov'),
    $_('months.dec')
  ]);

  // Derived values for UI logic
  let showWeekdaySelector = $derived(
    isRecurring && recurrenceType === 'weekly' && recurrenceInterval <= 4
  );
  let showMonthdaySelector = $derived(
    isRecurring && recurrenceType === 'monthly' && recurrenceInterval === 1
  );
  let showMonthSelector = $derived(
    isRecurring && recurrenceType === 'yearly' && recurrenceInterval === 1
  );
  let showMonthdayWarning = $derived(recurrenceMonthdays.some((d) => d > 28));

  // Computed end date from occurrence count
  let endDateFromCount = $derived.by(() => {
    if (!isRecurring || !recurrenceCount || recurrenceCount < 1 || !paymentDate) return null;
    return computeNthOccurrenceDate(
      paymentDate,
      recurrenceCount,
      recurrenceType,
      recurrenceInterval,
      recurrenceWeekdays.length > 0 ? recurrenceWeekdays : undefined,
      recurrenceMonthdays.length > 0 ? recurrenceMonthdays : undefined,
      recurrenceMonths.length > 0 ? recurrenceMonths : undefined
    );
  });

  // Determine which end constraint wins
  let effectiveEndDate = $derived.by(() => {
    if (!recurrenceEndDate && !endDateFromCount) return null;
    if (!recurrenceEndDate) return endDateFromCount;
    if (!endDateFromCount) return recurrenceEndDate;
    return recurrenceEndDate < endDateFromCount ? recurrenceEndDate : endDateFromCount;
  });

  // Show warning when both constraints are set and differ
  let endDateConflict = $derived.by(() => {
    if (!recurrenceEndDate || !endDateFromCount) return null;
    if (recurrenceEndDate === endDateFromCount) return null;
    return recurrenceEndDate < endDateFromCount ? 'date' : 'count';
  });

  // Internal transfer: receiver_account_id
  let receiverAccountId = $state<number | null>(null);

  // External inflow mode: money entering from outside (no payer)
  let isExternalInflow = $state(false);

  // Derived: Check if payer or receiver is a pool account
  let payerIsPool = $derived(
    payerId !== null && $participants.find((p) => p.id === payerId)?.account_type === 'pool'
  );
  let receiverIsPool = $derived(
    receiverAccountId !== null &&
      $participants.find((p) => p.id === receiverAccountId)?.account_type === 'pool'
  );

  // Contribution weights per participant
  let weights: Record<number, number> = $state({});
  let included: Record<number, boolean> = $state({});

  // Helper functions for local date handling
  function getLocalDateString(date: Date = new SvelteDate()): string {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}-${month}-${day}`;
  }

  // Computed shares
  let shares = $derived.by(() => {
    if (!amount || parseFloat(amount) <= 0) return {} as Record<number, number>;

    const total = parseFloat(amount);
    const activeParticipants = $participants.filter((p) => included[p.id] !== false);
    const totalWeight = activeParticipants.reduce(
      (sum, p) => sum + (weights[p.id] ?? p.default_weight),
      0
    );

    if (totalWeight === 0) return {} as Record<number, number>;

    const result: Record<number, number> = {};
    for (const p of $participants) {
      if (included[p.id] === false) {
        result[p.id] = 0;
      } else {
        const w = weights[p.id] ?? p.default_weight;
        result[p.id] = Math.round(((total * w) / totalWeight) * 100) / 100;
      }
    }
    return result;
  });

  // Mode-specific configuration and labels
  let modeConfig = $derived.by(() => {
    switch (mode) {
      case 'incoming':
        return {
          title: editingPaymentId
            ? $_('transactions.editIncoming')
            : $_('transactions.addIncoming'),
          submitLabel: editingPaymentId
            ? $_('transactions.updateTransaction')
            : $_('transactions.addIncoming'),
          isExternalInflow: true,
          requiresReceiver: true,
          hidePayerField: true,
          showContributorsTable: true,
          isRule: false
        };
      case 'internal':
        return {
          title: editingPaymentId
            ? $_('transactions.editTransfer')
            : $_('transactions.addTransfer'),
          submitLabel: editingPaymentId
            ? $_('transactions.updateTransaction')
            : $_('transactions.addTransfer'),
          isExternalInflow: false,
          requiresReceiver: true,
          hidePayerField: false,
          showContributorsTable: false,
          isRule: false
        };
      case 'rule':
        return {
          title: editingPaymentId ? $_('transactions.editRule') : $_('transactions.addRule'),
          submitLabel: editingPaymentId
            ? $_('transactions.updateTransaction')
            : $_('transactions.addRule'),
          isExternalInflow: true, // No payer (like incoming)
          requiresReceiver: true, // Goes to pool
          hidePayerField: true,
          showContributorsTable: true, // Show split between
          isRule: true // Special flag for rule mode
        };
      case 'outgoing':
      default:
        return {
          title: editingPaymentId ? $_('transactions.editPayment') : $_('transactions.addPayment'),
          submitLabel: editingPaymentId
            ? $_('transactions.updateTransaction')
            : $_('transactions.addPayment'),
          isExternalInflow: false,
          requiresReceiver: false,
          hidePayerField: false,
          showContributorsTable: true,
          isRule: false
        };
    }
  });

  // Configure mode on mount and when mode changes
  $effect(() => {
    if (!isValidMode) return;

    // Don't override if we're editing (startEditing will set these)
    if (editingPaymentId !== null) return;

    // Configure based on mode
    if (mode === 'incoming') {
      isExternalInflow = true;
      // Set default receiver to pool if available, else first participant
      if (!receiverAccountId && $participants.length > 0) {
        const pool = $participants.find((p) => p.account_type === 'pool');
        receiverAccountId = pool ? pool.id : $participants[0].id;
      }
    } else if (mode === 'internal') {
      isExternalInflow = false;
      // Set default receiver if not set
      if (!receiverAccountId && $participants.length > 0) {
        receiverAccountId = $participants[0].id;
      }
    } else if (mode === 'rule') {
      isExternalInflow = true; // No payer for rules
      // Rules must go to a pool - set receiver to pool
      if (!receiverAccountId && $participants.length > 0) {
        const pool = $participants.find((p) => p.account_type === 'pool');
        if (pool) {
          receiverAccountId = pool.id;
        }
      }
    } else {
      // outgoing
      isExternalInflow = false;
      receiverAccountId = null;
    }
  });

  // For internal transfers, always set payer as the only contributor
  // This applies to both new transfers and edited ones
  $effect(() => {
    if (mode !== 'internal' || payerId === null || $participants.length === 0) return;

    // Set contributor to payer only for internal transfers
    for (const p of $participants) {
      if (p.id === payerId) {
        included[p.id] = true;
        weights[p.id] = 1;
      } else {
        included[p.id] = false;
        weights[p.id] = 0;
      }
    }
  });

  // Load payment for editing when editId changes
  $effect(() => {
    if (editId && projectId && $participants.length > 0) {
      loadPaymentForEditing(editId);
    } else if (!editId) {
      loading = false;
    }
  });

  async function loadPaymentForEditing(paymentId: number) {
    loading = true;
    errorKey = '';
    try {
      const payment = await getPayment(projectId, paymentId);
      startEditing(payment);
    } catch (e) {
      errorKey = getErrorKey(e, 'transactions.failedToLoadPayment');
    } finally {
      loading = false;
    }
  }

  // Initialize form when participants change
  $effect(() => {
    if ($participants.length > 0 && editingPaymentId === null) {
      for (const p of $participants) {
        if (weights[p.id] === undefined) weights[p.id] = p.default_weight;
        if (included[p.id] === undefined) included[p.id] = p.default_weight > 0;
      }
      // Default payer to current user's participant if linked
      if (payerId === null) {
        const currentUserMember = $members.find((m) => m.user_id === $auth.user?.id);
        if (currentUserMember?.participant_id) {
          payerId = currentUserMember.participant_id;
        } else if ($participants.length > 0) {
          payerId = $participants[0].id;
        }
      }
    }
  });

  // Pre-fill form from URL parameters
  $effect(() => {
    const params = $page.url.searchParams;

    // Check for receiver parameter (from quick action buttons)
    const receiverParam = params.get('receiver');
    if (receiverParam) {
      const id = parseInt(receiverParam);
      if (!isNaN(id)) {
        receiverAccountId = id;
        // For internal mode, set up as transfer
        if (mode === 'internal' && payerId !== null) {
          // Set contributor to payer only
          for (const p of $participants) {
            if (p.id === payerId) {
              included[p.id] = true;
              weights[p.id] = 1;
            } else {
              included[p.id] = false;
              weights[p.id] = 0;
            }
          }
        }
      }
    }

    if (params.get('prefill') === 'true') {
      const payerIdParam = params.get('payer_id');
      const amountParam = params.get('amount');
      const recurringParam = params.get('recurring');
      const descParam = params.get('description');
      const recurrenceTypeParam = params.get('recurrence_type');
      const recurrenceIntervalParam = params.get('recurrence_interval');

      if (payerIdParam) {
        const id = parseInt(payerIdParam);
        if (!isNaN(id)) payerId = id;
      }
      if (amountParam) amount = amountParam;
      if (recurringParam === 'true') isRecurring = true;
      if (descParam) description = decodeURIComponent(descParam.replace(/\+/g, ' '));
      if (
        recurrenceTypeParam &&
        ['daily', 'weekly', 'monthly', 'yearly'].includes(recurrenceTypeParam)
      ) {
        recurrenceType = recurrenceTypeParam as 'daily' | 'weekly' | 'monthly' | 'yearly';
      }
      if (recurrenceIntervalParam) {
        const interval = parseInt(recurrenceIntervalParam);
        if (!isNaN(interval) && interval > 0) recurrenceInterval = interval;
      }
    }
  });

  // Initialize enhanced recurrence patterns when type/interval/date changes
  $effect(() => {
    if (!isRecurring) return;

    const defaultWeekday = paymentDate ? getDefaultWeekday(paymentDate) : 0;
    const defaultMonthDay = paymentDate ? getDefaultMonthDay(paymentDate) : 1;
    const defaultMonth = paymentDate ? getDefaultMonth(paymentDate) : 1;

    if (recurrenceType === 'weekly' && recurrenceInterval <= 4) {
      if (recurrenceWeekdays.length !== recurrenceInterval) {
        recurrenceWeekdays = initializeWeekdayArrays(recurrenceInterval, defaultWeekday);
        userModifiedWeekdays = false;
      } else if (!userModifiedWeekdays) {
        const newWeekdays = initializeWeekdayArrays(recurrenceInterval, defaultWeekday);
        if (JSON.stringify(recurrenceWeekdays) !== JSON.stringify(newWeekdays)) {
          recurrenceWeekdays = newWeekdays;
        }
      }
    }

    if (recurrenceType === 'monthly' && recurrenceInterval === 1) {
      if (recurrenceMonthdays.length === 0) {
        recurrenceMonthdays = [defaultMonthDay];
        userModifiedMonthdays = false;
      } else if (!userModifiedMonthdays && recurrenceMonthdays.length === 1) {
        if (recurrenceMonthdays[0] !== defaultMonthDay) {
          recurrenceMonthdays = [defaultMonthDay];
        }
      }
    }

    if (recurrenceType === 'yearly' && recurrenceInterval === 1) {
      if (recurrenceMonths.length === 0) {
        recurrenceMonths = [defaultMonth];
        userModifiedMonths = false;
      } else if (!userModifiedMonths && recurrenceMonths.length === 1) {
        if (recurrenceMonths[0] !== defaultMonth) {
          recurrenceMonths = [defaultMonth];
        }
      }
    }
  });

  function handleWeightChange(participantId: number, newWeight: number) {
    weights[participantId] = newWeight;
    if (newWeight === 0) {
      included[participantId] = false;
    }
  }

  function handleIncludedChange(participantId: number, isIncluded: boolean) {
    included[participantId] = isIncluded;
    if (!isIncluded) {
      weights[participantId] = 0;
    } else if (weights[participantId] === 0) {
      const participant = $participants.find((p) => p.id === participantId);
      weights[participantId] = participant?.default_weight || 1;
    }
  }

  // Include all participants with default weight > 0
  function includeAll() {
    for (const p of $participants) {
      if (p.default_weight > 0) {
        included[p.id] = true;
        weights[p.id] = p.default_weight;
      }
    }
  }

  // Handle file upload for receipt
  function handleFileChange(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) {
      receiptImage = null;
      receiptPreview = null;
      return;
    }

    const allowedTypes = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
    if (!allowedTypes.includes(file.type)) {
      errorKey = 'transactions.image.unsupportedFormat';
      return;
    }

    const maxSizeMB = 5;
    const maxSizeBytes = maxSizeMB * 1024 * 1024;
    if (file.size > maxSizeBytes) {
      errorKey = 'transactions.image.tooLarge';
      return;
    }

    const reader = new FileReader();
    reader.onerror = () => {
      errorKey = 'transactions.image.readFailed';
    };
    reader.onload = (e) => {
      const base64 = e.target?.result as string;
      if (!isValidImageMagic(base64)) {
        errorKey = 'transactions.image.invalidImage';
        return;
      }
      receiptImage = base64;
      receiptPreview = base64;
      errorKey = '';
    };
    reader.readAsDataURL(file);
  }

  function isValidImageMagic(base64: string): boolean {
    const base64Data = base64.includes(',') ? base64.split(',')[1] : base64;
    try {
      const binaryString = atob(base64Data.substring(0, 12));
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
      if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return true;
      if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47)
        return true;
      if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return true;
      if (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46)
        return true;
      return false;
    } catch {
      return false;
    }
  }

  function clearReceipt() {
    receiptImage = null;
    receiptPreview = null;
  }

  // Cancel editing and go back to list
  function cancelEditing() {
    goto(`/projects/${projectId}/transactions`);
  }

  // Start editing a payment
  function startEditing(payment: PaymentWithContributions) {
    editingPaymentId = payment.id;
    editingPaymentOriginal = payment;
    amount = payment.amount.toString();
    description = payment.description;
    payerId = payment.payer_id;
    paymentDate = payment.payment_date.split('T')[0];
    receiptImage = payment.receipt_image;
    receiptPreview = payment.receipt_image;
    isRecurring = payment.is_recurring;
    receiverAccountId = payment.receiver_account_id;
    isExternalInflow = payment.payer_id === null && payment.receiver_account_id !== null;
    isFinal = payment.is_final;
    // Convert API flags to UI toggles
    isPayerApproved = payment.affects_payer_expectation;
    isReceiverEarmarked = payment.affects_receiver_expectation;

    useSplitDate = false;
    splitFromDate = getLocalDateString();

    if (payment.is_recurring) {
      recurrenceType =
        (payment.recurrence_type as 'daily' | 'weekly' | 'monthly' | 'yearly') || 'monthly';
      recurrenceInterval = payment.recurrence_interval || 1;
      recurrenceEndDate = payment.recurrence_end_date || '';

      if (payment.recurrence_weekdays) {
        try {
          recurrenceWeekdays = JSON.parse(payment.recurrence_weekdays);
          userModifiedWeekdays = true;
        } catch {
          recurrenceWeekdays = [];
          userModifiedWeekdays = false;
        }
      } else {
        recurrenceWeekdays = [];
        userModifiedWeekdays = false;
      }

      if (payment.recurrence_monthdays) {
        try {
          recurrenceMonthdays = JSON.parse(payment.recurrence_monthdays);
          userModifiedMonthdays = true;
        } catch {
          recurrenceMonthdays = [];
          userModifiedMonthdays = false;
        }
      } else {
        recurrenceMonthdays = [];
        userModifiedMonthdays = false;
      }

      if (payment.recurrence_months) {
        try {
          recurrenceMonths = JSON.parse(payment.recurrence_months);
          userModifiedMonths = true;
        } catch {
          recurrenceMonths = [];
          userModifiedMonths = false;
        }
      } else {
        recurrenceMonths = [];
        userModifiedMonths = false;
      }
    }

    // Set weights and included from contributions
    for (const p of $participants) {
      const contrib = payment.contributions.find((c) => c.participant_id === p.id);
      if (contrib) {
        included[p.id] = true;
        weights[p.id] = contrib.weight;
      } else {
        included[p.id] = false;
        weights[p.id] = 0;
      }
    }
  }

  // Parse date string YYYY-MM-DD into Date object
  function parseDate(dateStr: string): Date {
    const [year, month, day] = dateStr.split('-').map(Number);
    return new SvelteDate(year, month - 1, day);
  }

  // Calculate days between two dates
  function daysBetween(start: Date, end: Date): number {
    const oneDay = 24 * 60 * 60 * 1000;
    return Math.round((end.getTime() - start.getTime()) / oneDay);
  }

  // Add days to a date
  function addDays(date: Date, days: number): SvelteDate {
    const result = new SvelteDate(date);
    result.setDate(result.getDate() + days);
    return result;
  }

  function getDefaultWeekday(dateStr: string): number {
    const date = parseDate(dateStr);
    return date.getDay();
  }

  function getDefaultMonthDay(dateStr: string): number {
    const date = parseDate(dateStr);
    return date.getDate();
  }

  function getDefaultMonth(dateStr: string): number {
    const date = parseDate(dateStr);
    return date.getMonth() + 1;
  }

  function initializeWeekdayArrays(numWeeks: number, defaultDay: number): number[][] {
    const result: number[][] = [];
    for (let i = 0; i < numWeeks; i++) {
      result.push([defaultDay]);
    }
    return result;
  }

  function toggleWeekday(weekIndex: number, day: number) {
    userModifiedWeekdays = true;
    const week = recurrenceWeekdays[weekIndex] || [];
    const idx = week.indexOf(day);
    if (idx >= 0) {
      if (week.length > 1) {
        recurrenceWeekdays[weekIndex] = week.filter((d) => d !== day);
      }
    } else {
      recurrenceWeekdays[weekIndex] = [...week, day].sort((a, b) => a - b);
    }
    recurrenceWeekdays = [...recurrenceWeekdays];
  }

  function toggleMonthday(day: number) {
    userModifiedMonthdays = true;
    const idx = recurrenceMonthdays.indexOf(day);
    if (idx >= 0) {
      if (recurrenceMonthdays.length > 1) {
        recurrenceMonthdays = recurrenceMonthdays.filter((d) => d !== day);
      }
    } else {
      recurrenceMonthdays = [...recurrenceMonthdays, day].sort((a, b) => a - b);
    }
  }

  function toggleMonth(month: number) {
    userModifiedMonths = true;
    const idx = recurrenceMonths.indexOf(month);
    if (idx >= 0) {
      if (recurrenceMonths.length > 1) {
        recurrenceMonths = recurrenceMonths.filter((m) => m !== month);
      }
    } else {
      recurrenceMonths = [...recurrenceMonths, month].sort((a, b) => a - b);
    }
  }

  function getRecurrenceDayInterval(
    type: string,
    interval: number,
    weekdays?: number[][],
    monthdays?: number[],
    months?: number[]
  ): number {
    if (type === 'weekly' && weekdays && weekdays.length > 0) {
      const totalDays = weekdays.reduce((sum, week) => sum + week.length, 0);
      if (totalDays > 0) {
        return Math.max(1, Math.floor((7 * interval) / totalDays));
      }
    }
    if (type === 'monthly' && monthdays && monthdays.length > 0) {
      return Math.max(1, Math.floor(30 / monthdays.length));
    }
    if (type === 'yearly' && months && months.length > 0) {
      return Math.max(1, Math.floor(365 / months.length));
    }
    switch (type) {
      case 'daily':
        return interval;
      case 'weekly':
        return interval * 7;
      case 'monthly':
        return interval * 30;
      case 'yearly':
        return interval * 365;
      default:
        return 30;
    }
  }

  function getLastOccurrenceBefore(
    startDate: Date,
    beforeDate: Date,
    type: string,
    interval: number,
    weekdays?: number[][],
    monthdays?: number[],
    months?: number[]
  ): Date | null {
    const dayInterval = getRecurrenceDayInterval(type, interval, weekdays, monthdays, months);
    const daysFromStart = daysBetween(startDate, beforeDate);
    if (daysFromStart < 0) return null;
    const occurrences = Math.floor(daysFromStart / dayInterval);
    if (occurrences < 0) return null;
    return addDays(startDate, occurrences * dayInterval);
  }

  function getFirstOccurrenceFrom(
    startDate: Date,
    fromDate: Date,
    type: string,
    interval: number,
    weekdays?: number[][],
    monthdays?: number[],
    months?: number[]
  ): Date {
    const dayInterval = getRecurrenceDayInterval(type, interval, weekdays, monthdays, months);
    const daysFromStart = daysBetween(startDate, fromDate);
    if (daysFromStart <= 0) {
      return new SvelteDate(startDate);
    }
    const fullIntervals = Math.ceil(daysFromStart / dayInterval);
    return addDays(startDate, fullIntervals * dayInterval);
  }

  function addMonths(date: Date, months: number): Date {
    const result = new SvelteDate(date);
    const targetMonth = result.getMonth() + months;
    result.setMonth(targetMonth);
    if (result.getMonth() !== ((targetMonth % 12) + 12) % 12) {
      result.setDate(0);
    }
    return result;
  }

  function addYears(date: Date, years: number): Date {
    const result = new SvelteDate(date);
    result.setFullYear(result.getFullYear() + years);
    if (date.getMonth() === 1 && date.getDate() === 29 && result.getDate() !== 29) {
      result.setDate(28);
    }
    return result;
  }

  function computeNthOccurrenceDate(
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
      return computeNthYearlyOccurrence(startDate, count, months);
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
    return getLocalDateString(result);
  }

  function computeNthWeeklyOccurrence(
    startDate: Date,
    count: number,
    interval: number,
    weekdays: number[][]
  ): string {
    let occurrenceCount = 0;
    let currentWeekStart = new SvelteDate(startDate);
    currentWeekStart.setDate(currentWeekStart.getDate() - currentWeekStart.getDay());
    let cycleWeek = 0;

    for (let weeks = 0; weeks < 1000; weeks++) {
      const weekDays = weekdays[cycleWeek] || [];
      for (const dayOfWeek of weekDays.sort((a, b) => a - b)) {
        const occurrenceDate = addDays(currentWeekStart, dayOfWeek);
        if (occurrenceDate >= startDate) {
          occurrenceCount++;
          if (occurrenceCount === count) {
            return getLocalDateString(occurrenceDate);
          }
        }
      }
      currentWeekStart = addDays(currentWeekStart, 7);
      cycleWeek = (cycleWeek + 1) % interval;
    }
    return getLocalDateString(addDays(startDate, count * 7));
  }

  function computeNthMonthlyOccurrence(
    startDate: Date,
    count: number,
    monthdays: number[]
  ): string {
    let occurrenceCount = 0;
    let currentYear = startDate.getFullYear();
    let currentMonth = startDate.getMonth();

    for (let m = 0; m < 1000; m++) {
      const daysInMonth = new SvelteDate(currentYear, currentMonth + 1, 0).getDate();
      for (const day of monthdays.sort((a, b) => a - b)) {
        const effectiveDay = Math.min(day, daysInMonth);
        const occurrenceDate = new SvelteDate(currentYear, currentMonth, effectiveDay);
        if (occurrenceDate >= startDate) {
          occurrenceCount++;
          if (occurrenceCount === count) {
            return getLocalDateString(occurrenceDate);
          }
        }
      }
      currentMonth++;
      if (currentMonth > 11) {
        currentMonth = 0;
        currentYear++;
      }
    }
    return getLocalDateString(addMonths(startDate, count));
  }

  function computeNthYearlyOccurrence(startDate: Date, count: number, months: number[]): string {
    let occurrenceCount = 0;
    const dayOfMonth = startDate.getDate();
    let currentYear = startDate.getFullYear();

    for (let y = 0; y < 1000; y++) {
      for (const month of months.sort((a, b) => a - b)) {
        const monthIndex = month - 1;
        const daysInMonth = new SvelteDate(currentYear, monthIndex + 1, 0).getDate();
        const effectiveDay = Math.min(dayOfMonth, daysInMonth);
        const occurrenceDate = new SvelteDate(currentYear, monthIndex, effectiveDay);
        if (occurrenceDate >= startDate) {
          occurrenceCount++;
          if (occurrenceCount === count) {
            return getLocalDateString(occurrenceDate);
          }
        }
      }
      currentYear++;
    }
    return getLocalDateString(addYears(startDate, count));
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!amount || parseFloat(amount) <= 0) return;

    // Validate mode-specific requirements
    if (modeConfig.requiresReceiver && receiverAccountId === null) {
      errorKey = 'transactions.receiverRequired';
      return;
    }

    submitting = true;
    errorKey = '';

    try {
      // For internal transfers, always use payer as sole contributor
      // This ensures the transfer is 100% paid by the payer
      let contributions;
      if (mode === 'internal' && payerId !== null) {
        contributions = [{ participant_id: payerId, weight: 1 }];
      } else {
        contributions = $participants
          .filter((p) => included[p.id] !== false)
          .map((p) => ({
            participant_id: p.id,
            weight: weights[p.id] ?? p.default_weight
          }));
      }

      // Compute API flags from UI state
      // Rules: affects_balance=false, affects_receiver_expectation=true (set expected minimum)
      // Normal transactions: affects_balance=true
      // Pool withdrawals can be "Approved" (affects_payer_expectation=true)
      // Pool deposits can be "Earmarked" (affects_receiver_expectation=true)
      const isRuleMode = modeConfig.isRule;
      const affectsBalance = !isRuleMode; // Rules don't move money
      const affectsPayerExpectation = !isRuleMode && isPayerApproved;
      const affectsReceiverExpectation = isRuleMode || isReceiverEarmarked;

      const payload: CreatePaymentInput = {
        payer_id: isExternalInflow ? null : payerId,
        amount: parseFloat(amount),
        description,
        payment_date: paymentDate,
        contributions,
        receipt_image: receiptImage ?? undefined,
        is_recurring: isRecurring,
        receiver_account_id: receiverAccountId,
        is_final: isRuleMode ? true : isFinal, // Rules are always final
        affects_balance: affectsBalance,
        affects_payer_expectation: affectsPayerExpectation,
        affects_receiver_expectation: affectsReceiverExpectation
      };

      if (isRecurring) {
        payload.recurrence_type = recurrenceType;
        payload.recurrence_interval = recurrenceInterval;
        if (effectiveEndDate) {
          payload.recurrence_end_date = effectiveEndDate;
        }
        if (
          recurrenceType === 'weekly' &&
          recurrenceInterval <= 4 &&
          recurrenceWeekdays.length > 0
        ) {
          payload.recurrence_weekdays = JSON.stringify(recurrenceWeekdays);
        }
        if (
          recurrenceType === 'monthly' &&
          recurrenceInterval === 1 &&
          recurrenceMonthdays.length > 0
        ) {
          payload.recurrence_monthdays = JSON.stringify(recurrenceMonthdays);
        }
        if (
          recurrenceType === 'yearly' &&
          recurrenceInterval === 1 &&
          recurrenceMonths.length > 0
        ) {
          payload.recurrence_months = JSON.stringify(recurrenceMonths);
        }
      }

      if (editingPaymentId !== null && useSplitDate && editingPaymentOriginal?.is_recurring) {
        // Split logic for recurring payments
        const originalStartDate = parseDate(editingPaymentOriginal.payment_date.split('T')[0]);
        const splitDate = parseDate(splitFromDate);
        const originalEndDate = editingPaymentOriginal.recurrence_end_date
          ? parseDate(editingPaymentOriginal.recurrence_end_date.split('T')[0])
          : null;

        const splitRecurrenceType =
          payload.recurrence_type || (editingPaymentOriginal.recurrence_type as string);
        const splitRecurrenceInterval =
          payload.recurrence_interval || editingPaymentOriginal.recurrence_interval || 1;

        if (splitDate < originalStartDate) {
          errorKey = 'transactions.split.beforeStart';
          submitting = false;
          return;
        }

        const newEndDate = payload.recurrence_end_date
          ? parseDate(payload.recurrence_end_date)
          : originalEndDate;
        if (newEndDate && splitDate > newEndDate) {
          errorKey = 'transactions.split.afterEnd';
          submitting = false;
          return;
        }

        const lastOccurrenceBeforeSplit = getLastOccurrenceBefore(
          originalStartDate,
          addDays(splitDate, -1),
          splitRecurrenceType,
          splitRecurrenceInterval
        );

        if (!lastOccurrenceBeforeSplit) {
          errorKey = 'transactions.split.beforeFirstOccurrence';
          submitting = false;
          return;
        }

        const firstOccurrenceFromSplit = getFirstOccurrenceFrom(
          splitDate,
          splitDate,
          recurrenceType,
          recurrenceInterval || 1
        );

        const endDateForOriginal = getLocalDateString(lastOccurrenceBeforeSplit);
        const newPaymentStartDate = getLocalDateString(firstOccurrenceFromSplit);

        let originalShouldRecur = true;
        if (endDateForOriginal === editingPaymentOriginal.payment_date.split('T')[0]) {
          originalShouldRecur = false;
        }

        let newShouldRecur = payload.is_recurring;
        if (newPaymentStartDate === (payload.recurrence_end_date || '')) {
          newShouldRecur = false;
        }

        const originalPayload: CreatePaymentInput = {
          payer_id: editingPaymentOriginal.payer_id,
          amount: editingPaymentOriginal.amount,
          description: editingPaymentOriginal.description,
          payment_date: editingPaymentOriginal.payment_date.split('T')[0],
          contributions: editingPaymentOriginal.contributions.map((c) => ({
            participant_id: c.participant_id,
            weight: c.weight
          })),
          receipt_image: editingPaymentOriginal.receipt_image ?? undefined,
          is_recurring: originalShouldRecur,
          receiver_account_id: editingPaymentOriginal.receiver_account_id,
          is_final: editingPaymentOriginal.is_final,
          affects_balance: editingPaymentOriginal.affects_balance,
          affects_payer_expectation: editingPaymentOriginal.affects_payer_expectation,
          affects_receiver_expectation: editingPaymentOriginal.affects_receiver_expectation
        };

        if (originalShouldRecur) {
          originalPayload.recurrence_type = editingPaymentOriginal.recurrence_type as
            | 'daily'
            | 'weekly'
            | 'monthly'
            | 'yearly';
          originalPayload.recurrence_interval =
            editingPaymentOriginal.recurrence_interval ?? undefined;
          originalPayload.recurrence_end_date = endDateForOriginal;
          if (editingPaymentOriginal.recurrence_weekdays) {
            originalPayload.recurrence_weekdays = editingPaymentOriginal.recurrence_weekdays;
          }
          if (editingPaymentOriginal.recurrence_monthdays) {
            originalPayload.recurrence_monthdays = editingPaymentOriginal.recurrence_monthdays;
          }
          if (editingPaymentOriginal.recurrence_months) {
            originalPayload.recurrence_months = editingPaymentOriginal.recurrence_months;
          }
        }

        await updatePayment(projectId, editingPaymentId, originalPayload);

        if (newShouldRecur || !payload.is_recurring) {
          payload.payment_date = newPaymentStartDate;
          if (newShouldRecur && !payload.recurrence_end_date && newEndDate) {
            payload.recurrence_end_date = getLocalDateString(newEndDate);
          }
          await createPayment(projectId, payload);
        }
      } else if (editingPaymentId !== null) {
        await updatePayment(projectId, editingPaymentId, payload);
      } else {
        await createPayment(projectId, payload);
      }

      // Redirect to list after success
      goto(`/projects/${projectId}/transactions`);
    } catch (e) {
      errorKey = getErrorKey(e, 'transactions.failedToSave');
    } finally {
      submitting = false;
    }
  }

  function formatDate(dateStr: string): string {
    const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
    const date = new SvelteDate(year, month - 1, day);
    return date.toLocaleDateString();
  }
</script>

{#if !isValidMode}
  <div class="error">{$_('transactions.invalidMode')}</div>
{:else if loading}
  <div class="loading">{$_('common.loading')}</div>
{:else}
  {#if errorKey}
    <div class="error">{$_(errorKey)}</div>
  {/if}

  {#if $canEdit}
    <section class="card">
      <h3>{modeConfig.title}</h3>

      {#if $participants.length === 0}
        <p class="warning">{$_('transactions.addParticipantsFirst')}</p>
      {:else}
        <form onsubmit={handleSubmit}>
          <div class="form-row">
            {#if mode === 'incoming'}
              <!-- Incoming: show receiver field -->
              <div class="field">
                <label for="inflow-receiver">{$_('transactions.receivedInto')}</label>
                <select id="inflow-receiver" bind:value={receiverAccountId} required>
                  {#each $participants as p (p.id)}
                    <option value={p.id}>
                      {p.name}{p.account_type === 'pool' ? ` (${$_('participants.pool')})` : ''}
                    </option>
                  {/each}
                </select>
              </div>
            {:else if mode === 'internal'}
              <!-- Internal: show from (payer) and to (receiver) -->
              <div class="field">
                <label for="payer">{$_('transactions.from')}</label>
                <select id="payer" bind:value={payerId}>
                  {#each $participants as p (p.id)}
                    <option value={p.id}>{p.name}</option>
                  {/each}
                </select>
              </div>
              <div class="field">
                <label for="receiver">{$_('transactions.to')}</label>
                <select id="receiver" bind:value={receiverAccountId} required>
                  {#each $participants as p (p.id)}
                    <option value={p.id}>
                      {p.name}{p.account_type === 'pool' ? ` (${$_('participants.pool')})` : ''}
                    </option>
                  {/each}
                </select>
              </div>
            {:else if mode === 'rule'}
              <!-- Rule: show applies-to field -->
              <div class="field">
                <label for="payer">{$_('transactions.appliesTo')}</label>
                <select id="payer" bind:value={receiverAccountId}>
                  {#each $participants as p (p.id)}
                    {#if p.account_type === 'pool'}
                      <option value={p.id}>{p.name}</option>
                    {/if}
                  {/each}
                </select>
              </div>
            {:else}
              <!-- Outgoing: show payer field -->
              <div class="field">
                <label for="payer">{$_('transactions.paidBy')}</label>
                <select id="payer" bind:value={payerId}>
                  {#each $participants as p (p.id)}
                    <option value={p.id}>{p.name}</option>
                  {/each}
                </select>
              </div>
            {/if}

            <div class="field">
              <label for="amount">{$_('transactions.amount')}</label>
              <input
                id="amount"
                type="number"
                bind:value={amount}
                min="0.01"
                step="0.01"
                required
              />
            </div>

            <div class="field">
              <label for="payment-date">{$_('transactions.date')}</label>
              <DateInput id="payment-date" bind:value={paymentDate} buttons={paymentDateButtons} />
            </div>
          </div>

          <div class="field">
            <label for="description">{$_('transactions.description')}</label>
            <input
              id="description"
              type="text"
              bind:value={description}
              placeholder={$_('transactions.descriptionPlaceholder')}
              required
            />
          </div>

          <!-- Receipt Image -->
          <div class="field">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label>
              {$_('transactions.receiptImage')}
              <span class="hint">{$_('transactions.imageFormatsHint')}</span>
            </label>
            <div class="receipt-upload">
              <input
                type="file"
                accept="image/jpeg,image/png,image/gif,image/webp"
                onchange={handleFileChange}
                bind:this={receiptInputEl}
                class="visually-hidden"
              />
              <button type="button" class="choose-file-btn" onclick={() => receiptInputEl?.click()}>
                {$_('transactions.chooseFile')}
              </button>
              {#if receiptPreview}
                <div class="receipt-preview">
                  <img src={receiptPreview} alt={$_('transactions.receiptPreview')} />
                  <button type="button" class="clear-btn" onclick={clearReceipt}>&times;</button>
                </div>
              {/if}
            </div>
          </div>

          <!-- Mode-specific banners -->
          {#if mode === 'incoming' && receiverAccountId}
            {@const receiver = $participants.find((p) => p.id === receiverAccountId)}
            <div class="external-inflow-banner">
              <span class="inflow-icon">↙</span>
              <span class="inflow-text">
                {$_('transactions.externalInflowTo')}
                {receiver?.name ?? $_('common.unknown')}
                {receiver?.account_type === 'pool' ? `(${$_('participants.pool')})` : ''}
              </span>
            </div>
          {:else if mode === 'internal' && receiverAccountId}
            {@const receiver = $participants.find((p) => p.id === receiverAccountId)}
            <div class="internal-transfer-banner">
              <span class="transfer-icon">↗</span>
              <span class="transfer-text">
                {$_('transactions.internalTransferTo')}
                {receiver?.name ?? $_('common.unknown')}
              </span>
            </div>
          {/if}

          <!-- Contributors table - only for outgoing and incoming -->
          {#if modeConfig.showContributorsTable}
            <div class="split-header">
              <h4>{$_('transactions.splitBetween')}</h4>
              <button type="button" class="small-btn" onclick={includeAll}>
                {$_('transactions.includeAll')}
              </button>
            </div>
            <table class="split-table">
              <thead>
                <tr>
                  <th>{$_('transactions.participant')}</th>
                  <th>{$_('transactions.include')}</th>
                  <th>{$_('transactions.weight')}</th>
                  <th>{$_('transactions.share')}</th>
                </tr>
              </thead>
              <tbody>
                {#each $participants as p (p.id)}
                  <tr>
                    <td>{p.name}</td>
                    <td>
                      <input
                        type="checkbox"
                        checked={included[p.id]}
                        onchange={(e) => handleIncludedChange(p.id, e.currentTarget.checked)}
                      />
                    </td>
                    <td>
                      <input
                        type="number"
                        value={weights[p.id]}
                        oninput={(e) =>
                          handleWeightChange(p.id, parseFloat(e.currentTarget.value) || 0)}
                        min="0"
                        step="0.5"
                        disabled={!included[p.id]}
                      />
                    </td>
                    <td class="share">{formatCurrency(shares[p.id] ?? 0)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}

          <!-- Status Section - Only show if NOT in rule mode -->
          {#if !modeConfig.isRule}
            <div class="status-section">
              <label for="payment-status">{$_('transactions.status')}</label>
              <div class="status-toggle">
                <button
                  type="button"
                  class="status-btn"
                  class:active={isFinal}
                  onclick={() => (isFinal = true)}
                >
                  {$_('transactions.statusFinal')}
                </button>
                <button
                  type="button"
                  class="status-btn"
                  class:active={!isFinal}
                  onclick={() => (isFinal = false)}
                >
                  {$_('transactions.statusDraft')}
                </button>
              </div>
            </div>
          {/if}

          <!-- Pool Approval Section - For pool withdrawals (payer is pool) -->
          {#if payerIsPool && !modeConfig.isRule}
            <div class="status-section">
              <label for="pool-approval">{$_('transactions.withdrawalType')}</label>
              <div class="status-toggle">
                <button
                  type="button"
                  class="status-btn"
                  class:active={!isPayerApproved}
                  onclick={() => (isPayerApproved = false)}
                >
                  {$_('transactions.standard')}
                </button>
                <button
                  type="button"
                  class="status-btn"
                  class:active={isPayerApproved}
                  onclick={() => (isPayerApproved = true)}
                >
                  {$_('transactions.approved')}
                </button>
              </div>
              <p class="hint-text">
                {isPayerApproved
                  ? $_('transactions.approvedHint')
                  : $_('transactions.standardWithdrawalHint')}
              </p>
            </div>
          {/if}

          <!-- Pool Earmark Section - For pool deposits (receiver is pool) -->
          {#if receiverIsPool && !modeConfig.isRule}
            <div class="status-section">
              <label for="pool-earmark">{$_('transactions.depositType')}</label>
              <div class="status-toggle">
                <button
                  type="button"
                  class="status-btn"
                  class:active={!isReceiverEarmarked}
                  onclick={() => (isReceiverEarmarked = false)}
                >
                  {$_('transactions.free')}
                </button>
                <button
                  type="button"
                  class="status-btn"
                  class:active={isReceiverEarmarked}
                  onclick={() => (isReceiverEarmarked = true)}
                >
                  {$_('transactions.earmarked')}
                </button>
              </div>
              <p class="hint-text">
                {isReceiverEarmarked
                  ? $_('transactions.earmarkedHint')
                  : $_('transactions.freeHint')}
              </p>
            </div>
          {/if}

          <!-- Recurrence Section -->
          <div class="recurrence-section">
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={isRecurring} />
              {$_('transactions.recurringTransaction')}
            </label>

            {#if isRecurring}
              <div class="recurrence-options">
                <div class="form-row">
                  <div class="field small">
                    <label for="recurrence-interval">{$_('transactions.every')}</label>
                    <input
                      id="recurrence-interval"
                      type="number"
                      bind:value={recurrenceInterval}
                      min="1"
                    />
                  </div>

                  <div class="field">
                    <label for="recurrence-type">{$_('transactions.period')}</label>
                    <select id="recurrence-type" bind:value={recurrenceType}>
                      <option value="daily"
                        >{recurrenceInterval === 1
                          ? $_('transactions.day')
                          : $_('transactions.days')}</option
                      >
                      <option value="weekly"
                        >{recurrenceInterval === 1
                          ? $_('transactions.week')
                          : $_('transactions.weeks')}</option
                      >
                      <option value="monthly"
                        >{recurrenceInterval === 1
                          ? $_('transactions.month')
                          : $_('transactions.months')}</option
                      >
                      <option value="yearly"
                        >{recurrenceInterval === 1
                          ? $_('transactions.year')
                          : $_('transactions.years')}</option
                      >
                    </select>
                  </div>
                </div>

                {#if showWeekdaySelector}
                  <div
                    class="weekday-selector"
                    role="group"
                    aria-labelledby="weekday-selector-label"
                  >
                    <span id="weekday-selector-label" class="selector-label"
                      >{$_('transactions.selectDaysOfWeek')}</span
                    >
                    {#each Array(recurrenceInterval) as _, weekIdx (weekIdx)}
                      {#if recurrenceInterval > 1}
                        <div class="week-label">{weekLabel} {weekIdx + 1}:</div>
                      {/if}
                      <div class="weekday-row">
                        {#each WEEKDAY_NAMES as day, dayIdx (dayIdx)}
                          <button
                            type="button"
                            class="weekday-btn"
                            class:selected={recurrenceWeekdays[weekIdx]?.includes(dayIdx)}
                            onclick={() => toggleWeekday(weekIdx, dayIdx)}
                          >
                            {day}
                          </button>
                        {/each}
                      </div>
                    {/each}
                  </div>
                {/if}

                {#if showMonthdaySelector}
                  <div
                    class="monthday-selector"
                    role="group"
                    aria-labelledby="monthday-selector-label"
                  >
                    <span id="monthday-selector-label" class="selector-label"
                      >{$_('transactions.selectDaysOfMonth')}</span
                    >
                    <div class="monthday-grid">
                      {#each Array(31) as _, idx (idx)}
                        {@const day = idx + 1}
                        <button
                          type="button"
                          class="monthday-btn"
                          class:selected={recurrenceMonthdays.includes(day)}
                          onclick={() => toggleMonthday(day)}
                        >
                          {day}
                        </button>
                      {/each}
                    </div>
                    {#if showMonthdayWarning}
                      <p class="warning-hint">{$_('transactions.monthdayWarning')}</p>
                    {/if}
                  </div>
                {/if}

                {#if showMonthSelector}
                  <div class="month-selector" role="group" aria-labelledby="month-selector-label">
                    <span id="month-selector-label" class="selector-label"
                      >{$_('transactions.selectMonths')}</span
                    >
                    <div class="month-grid">
                      {#each MONTH_NAMES as monthName, idx (idx)}
                        {@const month = idx + 1}
                        <button
                          type="button"
                          class="month-btn"
                          class:selected={recurrenceMonths.includes(month)}
                          onclick={() => toggleMonth(month)}
                        >
                          {monthName}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}

                <div class="recurrence-limits">
                  <div class="field">
                    <label for="end-date">{$_('transactions.endDateOptional')}</label>
                    <DateInput
                      id="end-date"
                      bind:value={recurrenceEndDate}
                      min={paymentDate}
                      buttons={recurrenceEndDateButtons}
                    />
                  </div>

                  <div class="field">
                    <label for="recurrence-count">{$_('transactions.occurrencesOptional')}</label>
                    <input
                      id="recurrence-count"
                      type="number"
                      bind:value={recurrenceCount}
                      min="1"
                      placeholder={$_('transactions.occurrencesPlaceholder')}
                    />
                    {#if endDateFromCount}
                      <span class="count-hint"
                        >{$_('transactions.lastOccurrence')}: {formatDate(endDateFromCount)}</span
                      >
                    {/if}
                  </div>
                </div>

                {#if endDateConflict}
                  <div class="recurrence-conflict-warning">
                    {#if endDateConflict === 'date'}
                      End date ({formatDate(recurrenceEndDate)}) is more restrictive than {recurrenceCount}
                      occurrences — end date will be used.
                    {:else}
                      {recurrenceCount} occurrences (ending {formatDate(endDateFromCount ?? '')}) is
                      more restrictive than end date — count will be used.
                    {/if}
                  </div>
                {/if}
              </div>
            {/if}
          </div>

          <!-- Split date option for recurring payments -->
          {#if editingPaymentId !== null && editingPaymentOriginal?.is_recurring}
            <div class="split-date-section">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={useSplitDate} />
                Apply changes starting from date
              </label>

              {#if useSplitDate}
                <div class="split-date-field">
                  <label for="split-from-date">Changes start from</label>
                  <DateInput
                    id="split-from-date"
                    bind:value={splitFromDate}
                    min={editingPaymentOriginal.payment_date.split('T')[0]}
                    max={editingPaymentOriginal.recurrence_end_date?.split('T')[0] || ''}
                    buttons={splitFromDateButtons}
                  />
                  <p class="split-hint">
                    {#if splitFromDate}
                      {@const lastBefore = getLastOccurrenceBefore(
                        parseDate(editingPaymentOriginal.payment_date.split('T')[0]),
                        addDays(parseDate(splitFromDate), -1),
                        recurrenceType || editingPaymentOriginal.recurrence_type || 'monthly',
                        recurrenceInterval || editingPaymentOriginal.recurrence_interval || 1
                      )}
                      {@const firstFrom = getFirstOccurrenceFrom(
                        parseDate(splitFromDate),
                        parseDate(splitFromDate),
                        recurrenceType,
                        recurrenceInterval || 1
                      )}
                      {#if lastBefore}
                        Original payment will end on {formatDate(getLocalDateString(lastBefore))},
                        new payment will start on {formatDate(getLocalDateString(firstFrom))}.
                      {:else}
                        <span class="warning">Date is before the first recurrence.</span>
                      {/if}
                    {:else}
                      Choose a date to see the split plan.
                    {/if}
                  </p>
                </div>
              {/if}
            </div>
          {/if}

          <div class="form-actions">
            <button type="submit" disabled={submitting || $participants.length === 0}>
              {submitting
                ? $_('common.saving')
                : editingPaymentId !== null
                  ? useSplitDate
                    ? $_('transactions.splitAndUpdate')
                    : $_('transactions.updateTransaction')
                  : modeConfig.submitLabel}
            </button>
            <button type="button" class="cancel-btn" onclick={cancelEditing}>
              {$_('common.cancel')}
            </button>
          </div>
        </form>
      {/if}
    </section>
  {:else}
    <p class="warning">{$_('transactions.noEditPermission')}</p>
  {/if}
{/if}

<style>
  .loading {
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 200px;
    color: #666;
  }

  .card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
  }

  h3 {
    margin-top: 0;
    margin-bottom: 1rem;
    color: var(--accent, #7b61ff);
  }

  h4 {
    margin: 0;
    font-size: 1rem;
  }

  .error {
    background: #fee;
    color: #c00;
    padding: 0.75rem;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .warning {
    background: #fff3cd;
    color: #856404;
    padding: 0.75rem;
    border-radius: 8px;
  }

  .form-row {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .field {
    flex: 1;
    min-width: 120px;
    margin-bottom: 1rem;
  }

  .field.small {
    flex: 0 0 80px;
    min-width: 80px;
  }

  label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 600;
    font-size: 0.9rem;
  }

  label .hint {
    font-weight: normal;
    font-size: 0.8rem;
    color: #666;
    display: block;
    margin-top: 0.25rem;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
  }

  .checkbox-label input[type='checkbox'] {
    width: auto;
  }

  input:not([type='file']),
  select {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 1rem;
  }

  input:not([type='file']):focus,
  select:focus {
    outline: none;
    border-color: var(--accent, #7b61ff);
  }

  /* Receipt upload */
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .choose-file-btn {
    display: inline-flex;
    align-items: center;
    padding: 0.5rem 1rem;
    border: 1px solid #ccc;
    border-radius: 6px;
    background: #f5f5f5;
    cursor: pointer;
    font-size: 0.9rem;
    color: #333;
  }

  .choose-file-btn:hover {
    background: #e8e8e8;
  }

  .receipt-upload {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .receipt-preview {
    position: relative;
    display: inline-block;
    max-width: 200px;
  }

  .receipt-preview img {
    max-width: 100%;
    max-height: 150px;
    border-radius: 8px;
    border: 1px solid #ddd;
  }

  .clear-btn {
    position: absolute;
    top: -8px;
    right: -8px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #c00;
    color: white;
    border: none;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
  }

  /* Status section */
  .status-section {
    margin-bottom: 1rem;
  }

  .status-section > label {
    display: block;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: #333;
  }

  .status-toggle {
    display: flex;
    gap: 0;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid #ddd;
    width: fit-content;
  }

  .status-btn {
    padding: 0.5rem 1rem;
    border: none;
    background: #f5f5f5;
    color: #666;
    cursor: pointer;
    font-size: 0.875rem;
    transition: all 0.2s;
  }

  .status-btn:hover {
    background: #eee;
  }

  .status-btn.active {
    background: #4a90d9;
    color: white;
  }

  .status-btn.active:last-child {
    background: #f0ad4e;
    color: white;
  }

  /* Hint text for pool toggles */
  .hint-text {
    margin: 0.5rem 0 0 0;
    color: #666;
    font-size: 0.85rem;
    font-style: italic;
  }

  /* Recurrence section */
  .recurrence-section {
    background: #f9f9f9;
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .recurrence-options {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #eee;
  }

  .recurrence-limits {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    margin-top: 1rem;
  }

  .recurrence-limits .field {
    flex: 1;
    min-width: 150px;
  }

  .count-hint {
    display: block;
    font-size: 0.8rem;
    color: #666;
    margin-top: 0.25rem;
  }

  .recurrence-conflict-warning {
    background: #fff3cd;
    border: 1px solid #ffc107;
    color: #856404;
    padding: 0.75rem;
    border-radius: 8px;
    margin-top: 0.75rem;
    font-size: 0.9rem;
  }

  .split-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 1rem 0 0.5rem;
  }

  .small-btn {
    padding: 0.25rem 0.75rem;
    background: #e0e0e0;
    border: none;
    border-radius: 4px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .small-btn:hover {
    background: #d0d0d0;
  }

  .split-table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 1rem;
  }

  .split-table th,
  .split-table td {
    padding: 0.5rem;
    text-align: left;
    border-bottom: 1px solid #eee;
  }

  .split-table input[type='number'] {
    width: 80px;
    padding: 0.5rem;
  }

  .split-table input[type='checkbox'] {
    width: auto;
  }

  .share {
    font-weight: 600;
    color: var(--accent, #7b61ff);
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
  }

  button[type='submit'] {
    padding: 0.75rem 1.5rem;
    background: var(--accent, #7b61ff);
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 1rem;
    cursor: pointer;
  }

  button[type='submit']:hover:not(:disabled) {
    opacity: 0.9;
  }

  button[type='submit']:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .cancel-btn {
    padding: 0.75rem 1.5rem;
    background: #e0e0e0;
    color: #333;
    border: none;
    border-radius: 8px;
    font-size: 1rem;
    cursor: pointer;
  }

  .cancel-btn:hover {
    background: #d0d0d0;
  }

  /* Internal Transfer Styles */
  .internal-transfer-banner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: linear-gradient(135deg, #e8f5e9 0%, #c8e6c9 100%);
    border: 1px solid #81c784;
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .transfer-icon {
    font-size: 1.2rem;
    color: #2e7d32;
  }

  .transfer-text {
    flex: 1;
    font-weight: 500;
    color: #2e7d32;
  }

  /* External inflow styles */
  .external-inflow-banner {
    background: #e3f2fd;
    border: 1px solid #1976d2;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin-bottom: 1rem;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .inflow-icon {
    font-size: 1.2rem;
    color: #1976d2;
  }

  .inflow-text {
    flex: 1;
    color: #1565c0;
    font-weight: 500;
  }

  /* Split date section */
  .split-date-section {
    background: #fff8e1;
    border: 1px solid #ffcc80;
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .split-date-field {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid #ffcc80;
  }

  .split-date-field label {
    font-size: 0.9rem;
    margin-bottom: 0.5rem;
  }

  .split-hint {
    margin-top: 0.5rem;
    margin-bottom: 0;
    font-size: 0.85rem;
    color: #6d4c41;
    font-style: italic;
  }

  .split-hint .warning {
    display: inline;
    padding: 0.15rem 0.4rem;
    margin: 0;
  }

  /* Enhanced Recurrence Selectors */
  .weekday-selector,
  .monthday-selector,
  .month-selector {
    margin-top: 1rem;
    padding: 1rem;
    background: #f5f5f5;
    border-radius: 8px;
  }

  .selector-label {
    display: block;
    margin-bottom: 0.75rem;
    font-weight: 600;
  }

  .week-label {
    font-size: 0.85rem;
    color: #666;
    margin: 0.5rem 0 0.25rem;
  }

  .weekday-row {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
  }

  .weekday-btn,
  .monthday-btn,
  .month-btn {
    padding: 0.5rem 0.75rem;
    border: 1px solid #ddd;
    background: white;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s;
  }

  .weekday-btn:hover,
  .monthday-btn:hover,
  .month-btn:hover {
    border-color: var(--accent, #7b61ff);
  }

  .weekday-btn.selected,
  .monthday-btn.selected,
  .month-btn.selected {
    background: var(--accent, #7b61ff);
    color: white;
    border-color: var(--accent, #7b61ff);
  }

  .monthday-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(28px, 36px));
    gap: 0.2rem;
    justify-content: start;
  }

  .monthday-btn {
    aspect-ratio: 1;
    padding: 0.15rem;
    font-size: 0.75rem;
  }

  .month-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.5rem;
  }

  .warning-hint {
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: #fff3cd;
    border-radius: 4px;
    font-size: 0.85rem;
    color: #856404;
  }

  /* Tablet responsive styles */
  @media (max-width: 768px) {
    .split-header {
      margin: 0.75rem 0 0.4rem;
    }

    .split-header h4 {
      font-size: 1rem;
    }

    .small-btn {
      padding: 0.2rem 0.5rem;
      font-size: 0.75rem;
    }

    .split-table th,
    .split-table td {
      padding: 0.4rem;
      font-size: 0.9rem;
    }

    .split-table input[type='number'] {
      width: 60px;
      padding: 0.4rem;
      font-size: 0.85rem;
    }
  }

  /* Mobile responsive styles */
  @media (max-width: 480px) {
    .split-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
      margin: 0.5rem 0 0.3rem;
    }

    .split-header h4 {
      font-size: 0.95rem;
    }

    .small-btn {
      align-self: stretch;
      padding: 0.3rem 0.5rem;
      font-size: 0.75rem;
    }

    .split-table {
      font-size: 0.8rem;
      display: block;
      overflow-x: auto;
    }

    .split-table th,
    .split-table td {
      padding: 0.3rem 0.2rem;
      font-size: 0.8rem;
    }

    .split-table th:first-child,
    .split-table td:first-child {
      padding-left: 0.4rem;
    }

    .split-table th:last-child,
    .split-table td:last-child {
      padding-right: 0.4rem;
    }

    .split-table input[type='number'] {
      width: 50px;
      padding: 0.3rem 0.2rem;
      font-size: 0.75rem;
    }

    .share {
      font-size: 0.8rem;
    }
  }
</style>
