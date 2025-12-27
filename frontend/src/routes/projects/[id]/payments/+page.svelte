<script lang="ts">
    import { page } from '$app/stores';
    import { getPayments, createPayment, updatePayment, deletePayment, type PaymentWithContributions, type CreatePaymentInput } from "$lib/api";
    import { participants, currentProject, canEdit, members } from '$lib/stores/project';
    import { auth } from '$lib/auth';

    let payments: PaymentWithContributions[] = $state([]);
    let loading = $state(true);
    let error = $state('');

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

    // Recurrence state
    let isRecurring = $state(false);
    let recurrenceInterval = $state(1);
    let recurrenceType = $state<'daily' | 'weekly' | 'monthly' | 'yearly'>('monthly');
    let recurrenceEndDate = $state('');

    // Enhanced recurrence patterns
    // For weekly: array of arrays, one per week in cycle - each inner array contains selected weekdays (0=Sun, 6=Sat)
    let recurrenceWeekdays = $state<number[][]>([]);
    // For monthly: array of selected day numbers (1-31)
    let recurrenceMonthdays = $state<number[]>([]);
    // For yearly: array of selected month numbers (1-12)
    let recurrenceMonths = $state<number[]>([]);

    // Day and month names for display
    const WEEKDAY_NAMES = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const MONTH_NAMES = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

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
    let showMonthdayWarning = $derived(
        recurrenceMonthdays.some(d => d > 28)
    );

    // Internal transfer: receiver_account_id
    // null = external expense (money leaves system)
    // number = internal transfer to this account (e.g., deposit to pool)
    let receiverAccountId = $state<number | null>(null);

    // Image modal state
    let showImageModal = $state(false);
    let modalImage = $state<string | null>(null);

    // Contribution weights per participant
    let weights: Record<number, number> = $state({});
    let included: Record<number, boolean> = $state({});

    // Helper functions for local date handling
    function getLocalDateString(date: Date = new Date()): string {
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        return `${year}-${month}-${day}`;
    }

    // Get project ID from URL
    let projectId = $derived(parseInt($page.params.id ?? ''));

    // Computed shares
    let shares = $derived.by(() => {
        if (!amount || parseFloat(amount) <= 0) return {} as Record<number, number>;

        const total = parseFloat(amount);
        const activeParticipants = $participants.filter(p => included[p.id] !== false);
        const totalWeight = activeParticipants.reduce((sum, p) => sum + (weights[p.id] ?? p.default_weight), 0);

        if (totalWeight === 0) return {} as Record<number, number>;

        const result: Record<number, number> = {};
        for (const p of $participants) {
            if (included[p.id] === false) {
                result[p.id] = 0;
            } else {
                const w = weights[p.id] ?? p.default_weight;
                result[p.id] = Math.round((total * w / totalWeight) * 100) / 100;
            }
        }
        return result;
    });

    async function loadPayments() {
        loading = true;
        error = '';
        try {
            payments = await getPayments(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load payments';
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
            // Default payer to current user's participant if linked, otherwise first participant
            if (payerId === null) {
                // Find current user's participant (user_id matches current auth user)
                const currentUserMember = $members.find(m => m.user_id === $auth.user?.id);
                if (currentUserMember?.participant_id) {
                    payerId = currentUserMember.participant_id;
                } else if ($participants.length > 0) {
                    // Fallback to first participant if user doesn't have a linked participant
                    payerId = $participants[0].id;
                }
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
            // Initialize weekday arrays if empty or wrong size
            if (recurrenceWeekdays.length !== recurrenceInterval) {
                recurrenceWeekdays = initializeWeekdayArrays(recurrenceInterval, defaultWeekday);
            }
        }

        if (recurrenceType === 'monthly' && recurrenceInterval === 1) {
            if (recurrenceMonthdays.length === 0) {
                recurrenceMonthdays = [defaultMonthDay];
            }
        }

        if (recurrenceType === 'yearly' && recurrenceInterval === 1) {
            if (recurrenceMonths.length === 0) {
                recurrenceMonths = [defaultMonth];
            }
        }
    });

    // Sync weight and included: weight 0 unchecks, uncheck sets weight to 0
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
            // Restore to default weight when re-including
            const participant = $participants.find(p => p.id === participantId);
            weights[participantId] = participant?.default_weight || 1;
        }
    }

    // Transfer money to this person/pool - creates an internal transfer
    // This is used for paying back debts or depositing to pool
    function transferTo(participantId: number) {
        // Set receiver to the target participant (person or pool)
        receiverAccountId = participantId;

        // The contributor should be the payer (who is transferring money)
        // This creates a record: payer transfers X to recipient
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

    // Check if this is an internal transfer
    let isInternalTransfer = $derived(receiverAccountId !== null);

    // Get receiver name for display
    let receiverName = $derived.by(() => {
        if (receiverAccountId === null) return null;
        return $participants.find(p => p.id === receiverAccountId)?.name ?? 'Unknown';
    });

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

        // Validate file type by MIME type
        const allowedTypes = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
        if (!allowedTypes.includes(file.type)) {
            error = 'Please select a supported image format (JPEG, PNG, GIF, or WebP)';
            return;
        }

        // Validate file size (max 5MB)
        const maxSizeMB = 5;
        const maxSizeBytes = maxSizeMB * 1024 * 1024;
        if (file.size > maxSizeBytes) {
            error = `Image must be less than ${maxSizeMB}MB (actual: ${(file.size / (1024 * 1024)).toFixed(2)}MB)`;
            return;
        }

        const reader = new FileReader();
        reader.onerror = () => {
            error = 'Failed to read the image file';
        };
        reader.onload = (e) => {
            const base64 = e.target?.result as string;

            // Validate magic bytes for actual image format
            if (!isValidImageMagic(base64)) {
                error = 'File is not a valid image. Please upload a real image file.';
                return;
            }

            receiptImage = base64;
            receiptPreview = base64;
            error = '';
        };
        reader.readAsDataURL(file);
    }

    // Check image magic bytes to validate it's an actual image
    function isValidImageMagic(base64: string): boolean {
        // Extract binary data after comma in data URI
        const base64Data = base64.includes(',') ? base64.split(',')[1] : base64;

        try {
            // Decode first few bytes to check magic
            const binaryString = atob(base64Data.substring(0, 12)); // First 12 chars = ~9 bytes
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
                bytes[i] = binaryString.charCodeAt(i);
            }

            // Check for image magic bytes
            // JPEG: FF D8 FF
            if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) return true;
            // PNG: 89 50 4E 47
            if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) return true;
            // GIF: 47 49 46 (GIF)
            if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return true;
            // WebP: 52 49 46 46 ... 57 45 42 50 (RIFF...WEBP)
            if (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46) return true;

            return false;
        } catch {
            return false;
        }
    }

    function clearReceipt() {
        receiptImage = null;
        receiptPreview = null;
    }

    // Load payments when projectId changes
    $effect(() => {
        if (projectId) {
            loadPayments();
        }
    });

    // Reset form to default state
    function resetForm() {
        editingPaymentId = null;
        editingPaymentOriginal = null;
        amount = '';
        description = '';
        paymentDate = getLocalDateString();
        receiptImage = null;
        receiptPreview = null;
        isRecurring = false;
        recurrenceInterval = 1;
        recurrenceType = 'monthly';
        recurrenceEndDate = '';
        recurrenceWeekdays = [];
        recurrenceMonthdays = [];
        recurrenceMonths = [];
        receiverAccountId = null; // Reset internal transfer state
        useSplitDate = false;
        splitFromDate = getLocalDateString();

        // Reset weights and included to defaults
        for (const p of $participants) {
            weights[p.id] = p.default_weight;
            included[p.id] = p.default_weight > 0;
        }

        // Reset payer
        if ($participants.length > 0) {
            payerId = $participants[0].id;
        }
    }

    // Start editing a payment
    function startEditing(payment: PaymentWithContributions) {
        editingPaymentId = payment.id;
        editingPaymentOriginal = payment;
        amount = payment.amount.toString();
        description = payment.description;
        payerId = payment.payer_id;
        paymentDate = payment.payment_date.split('T')[0]; // Handle ISO date format
        receiptImage = payment.receipt_image;
        receiptPreview = payment.receipt_image;
        isRecurring = payment.is_recurring;
        receiverAccountId = payment.receiver_account_id; // Load internal transfer state

        // Reset split date option
        useSplitDate = false;
        splitFromDate = getLocalDateString();

        if (payment.is_recurring) {
            recurrenceType = (payment.recurrence_type as 'daily' | 'weekly' | 'monthly' | 'yearly') || 'monthly';
            recurrenceInterval = payment.recurrence_interval || 1;
            recurrenceEndDate = payment.recurrence_end_date || '';

            // Parse enhanced patterns from JSON
            if (payment.recurrence_weekdays) {
                try {
                    recurrenceWeekdays = JSON.parse(payment.recurrence_weekdays);
                } catch {
                    recurrenceWeekdays = [];
                }
            } else {
                recurrenceWeekdays = [];
            }

            if (payment.recurrence_monthdays) {
                try {
                    recurrenceMonthdays = JSON.parse(payment.recurrence_monthdays);
                } catch {
                    recurrenceMonthdays = [];
                }
            } else {
                recurrenceMonthdays = [];
            }

            if (payment.recurrence_months) {
                try {
                    recurrenceMonths = JSON.parse(payment.recurrence_months);
                } catch {
                    recurrenceMonths = [];
                }
            } else {
                recurrenceMonths = [];
            }
        }

        // Set weights and included from contributions
        for (const p of $participants) {
            const contrib = payment.contributions.find(c => c.participant_id === p.id);
            if (contrib) {
                included[p.id] = true;
                weights[p.id] = contrib.weight;
            } else {
                included[p.id] = false;
                weights[p.id] = 0;
            }
        }
    }

    function cancelEditing() {
        resetForm();
    }

    // Parse date string YYYY-MM-DD into Date object
    function parseDate(dateStr: string): Date {
        const [year, month, day] = dateStr.split('-').map(Number);
        return new Date(year, month - 1, day);
    }

    // Calculate days between two dates
    function daysBetween(start: Date, end: Date): number {
        const oneDay = 24 * 60 * 60 * 1000;
        return Math.round((end.getTime() - start.getTime()) / oneDay);
    }

    // Add days to a date
    function addDays(date: Date, days: number): Date {
        const result = new Date(date);
        result.setDate(result.getDate() + days);
        return result;
    }

    // Get default weekday from payment date (0=Sunday, 6=Saturday)
    function getDefaultWeekday(dateStr: string): number {
        const date = parseDate(dateStr);
        return date.getDay();
    }

    // Get default month day from payment date (1-31)
    function getDefaultMonthDay(dateStr: string): number {
        const date = parseDate(dateStr);
        return date.getDate();
    }

    // Get default month from payment date (1-12)
    function getDefaultMonth(dateStr: string): number {
        const date = parseDate(dateStr);
        return date.getMonth() + 1;
    }

    // Initialize weekday arrays when interval changes
    function initializeWeekdayArrays(numWeeks: number, defaultDay: number): number[][] {
        const result: number[][] = [];
        for (let i = 0; i < numWeeks; i++) {
            result.push([defaultDay]);
        }
        return result;
    }

    // Toggle weekday in a specific week
    function toggleWeekday(weekIndex: number, day: number) {
        const week = recurrenceWeekdays[weekIndex] || [];
        const idx = week.indexOf(day);
        if (idx >= 0) {
            // Don't allow deselecting if it's the only one
            if (week.length > 1) {
                recurrenceWeekdays[weekIndex] = week.filter(d => d !== day);
            }
        } else {
            recurrenceWeekdays[weekIndex] = [...week, day].sort((a, b) => a - b);
        }
        recurrenceWeekdays = [...recurrenceWeekdays]; // Trigger reactivity
    }

    // Toggle month day
    function toggleMonthday(day: number) {
        const idx = recurrenceMonthdays.indexOf(day);
        if (idx >= 0) {
            if (recurrenceMonthdays.length > 1) {
                recurrenceMonthdays = recurrenceMonthdays.filter(d => d !== day);
            }
        } else {
            recurrenceMonthdays = [...recurrenceMonthdays, day].sort((a, b) => a - b);
        }
    }

    // Toggle month
    function toggleMonth(month: number) {
        const idx = recurrenceMonths.indexOf(month);
        if (idx >= 0) {
            if (recurrenceMonths.length > 1) {
                recurrenceMonths = recurrenceMonths.filter(m => m !== month);
            }
        } else {
            recurrenceMonths = [...recurrenceMonths, month].sort((a, b) => a - b);
        }
    }

    // Ordinal suffix for day numbers (1st, 2nd, 3rd, etc.)
    function ordinal(n: number): string {
        const s = ['th', 'st', 'nd', 'rd'];
        const v = n % 100;
        return n + (s[(v - 20) % 10] || s[v] || s[0]);
    }

    // Calculate approximate interval in days based on recurrence settings
    // For enhanced patterns (multiple days/months), returns an estimate
    function getRecurrenceDayInterval(
        type: string,
        interval: number,
        weekdays?: number[][],
        monthdays?: number[],
        months?: number[]
    ): number {
        // For enhanced patterns, estimate average interval
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

        // Fallback to simple interval calculation
        switch (type) {
            case 'daily': return interval;
            case 'weekly': return interval * 7;
            case 'monthly': return interval * 30;
            case 'yearly': return interval * 365;
            default: return 30;
        }
    }

    // Find the last occurrence on or before a given date
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

        if (daysFromStart < 0) {
            // beforeDate is before startDate
            return null;
        }

        // Calculate how many intervals fit before the date
        const occurrences = Math.floor(daysFromStart / dayInterval);
        if (occurrences < 0) return null;

        return addDays(startDate, occurrences * dayInterval);
    }

    // Find the first occurrence on or after a given date
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
            return new Date(startDate);
        }

        // Calculate how many full intervals have passed
        const fullIntervals = Math.ceil(daysFromStart / dayInterval);
        return addDays(startDate, fullIntervals * dayInterval);
    }

    async function handleSubmit(e: Event) {
        e.preventDefault();
        if (!amount || parseFloat(amount) <= 0) return;

        submitting = true;
        error = '';

        try {
            const contributions = $participants
                .filter(p => included[p.id] !== false)
                .map(p => ({
                    participant_id: p.id,
                    weight: weights[p.id] ?? p.default_weight
                }));

            const payload: CreatePaymentInput = {
                payer_id: payerId,
                amount: parseFloat(amount),
                description,
                payment_date: paymentDate,
                contributions,
                receipt_image: receiptImage ?? undefined,
                is_recurring: isRecurring,
                receiver_account_id: receiverAccountId, // Internal transfer support
            };

            if (isRecurring) {
                payload.recurrence_type = recurrenceType;
                payload.recurrence_interval = recurrenceInterval;

                if (recurrenceEndDate) {
                    payload.recurrence_end_date = recurrenceEndDate;
                }

                // Enhanced patterns - only include when applicable
                if (recurrenceType === 'weekly' && recurrenceInterval <= 4 && recurrenceWeekdays.length > 0) {
                    payload.recurrence_weekdays = JSON.stringify(recurrenceWeekdays);
                }
                if (recurrenceType === 'monthly' && recurrenceInterval === 1 && recurrenceMonthdays.length > 0) {
                    payload.recurrence_monthdays = JSON.stringify(recurrenceMonthdays);
                }
                if (recurrenceType === 'yearly' && recurrenceInterval === 1 && recurrenceMonths.length > 0) {
                    payload.recurrence_months = JSON.stringify(recurrenceMonths);
                }
            }

            if (editingPaymentId !== null && useSplitDate && editingPaymentOriginal?.is_recurring) {
                // Enhanced split logic: calculate proper recurrence boundaries
                const originalStartDate = parseDate(editingPaymentOriginal.payment_date.split('T')[0]);
                const splitDate = parseDate(splitFromDate);
                const originalEndDate = editingPaymentOriginal.recurrence_end_date
                    ? parseDate(editingPaymentOriginal.recurrence_end_date.split('T')[0])
                    : null;

                // Use edited recurrence settings if changing, otherwise use original
                const splitRecurrenceType = payload.recurrence_type || (editingPaymentOriginal.recurrence_type as string);
                const splitRecurrenceInterval = payload.recurrence_interval || editingPaymentOriginal.recurrence_interval || 1;

                // Validate: split date must be on or after first occurrence
                if (splitDate < originalStartDate) {
                    error = 'Split date cannot be before the original payment start date';
                    submitting = false;
                    return;
                }

                // Validate: split date must be on or before current end date (if edited)
                const newEndDate = payload.recurrence_end_date ? parseDate(payload.recurrence_end_date) : originalEndDate;
                if (newEndDate && splitDate > newEndDate) {
                    error = 'Split date cannot be after the recurrence end date';
                    submitting = false;
                    return;
                }

                // Calculate last occurrence before split date (using edited recurrence if applicable)
                const lastOccurrenceBeforeSplit = getLastOccurrenceBefore(
                    originalStartDate,
                    addDays(splitDate, -1),
                    splitRecurrenceType,
                    splitRecurrenceInterval
                );

                if (!lastOccurrenceBeforeSplit) {
                    error = 'Split date is before the first recurrence. Please choose a date on or after the first occurrence.';
                    submitting = false;
                    return;
                }

                // Calculate first occurrence from split date (using edited recurrence for new payment)
                const firstOccurrenceFromSplit = getFirstOccurrenceFrom(
                    splitDate,
                    splitDate,
                    recurrenceType, // Use the new/edited recurrence type
                    recurrenceInterval || 1
                );

                const endDateForOriginal = getLocalDateString(lastOccurrenceBeforeSplit);
                const newPaymentStartDate = getLocalDateString(firstOccurrenceFromSplit);

                // Check if begin date == new end date (remove recurrence for original)
                let originalShouldRecur = true;
                if (endDateForOriginal === editingPaymentOriginal.payment_date.split('T')[0]) {
                    originalShouldRecur = false;
                }

                // Check if new begin date == end date (remove recurrence for new payment)
                let newShouldRecur = payload.is_recurring;
                if (newPaymentStartDate === (payload.recurrence_end_date || '')) {
                    newShouldRecur = false;
                }

                // Update original payment
                const originalPayload: CreatePaymentInput = {
                    payer_id: editingPaymentOriginal.payer_id,
                    amount: editingPaymentOriginal.amount,
                    description: editingPaymentOriginal.description,
                    payment_date: editingPaymentOriginal.payment_date.split('T')[0],
                    contributions: editingPaymentOriginal.contributions.map(c => ({
                        participant_id: c.participant_id,
                        weight: c.weight
                    })),
                    receipt_image: editingPaymentOriginal.receipt_image ?? undefined,
                    is_recurring: originalShouldRecur,
                    receiver_account_id: editingPaymentOriginal.receiver_account_id,
                };

                if (originalShouldRecur) {
                    originalPayload.recurrence_type = editingPaymentOriginal.recurrence_type as 'daily' | 'weekly' | 'monthly' | 'yearly';
                    originalPayload.recurrence_interval = editingPaymentOriginal.recurrence_interval ?? undefined;
                    originalPayload.recurrence_end_date = endDateForOriginal;
                    // Preserve enhanced patterns from original
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

                // Create new payment only if new payment should have occurrences
                if (newShouldRecur || !payload.is_recurring) {
                    payload.payment_date = newPaymentStartDate;
                    if (newShouldRecur && payload.recurrence_end_date) {
                        // Keep the end date, but only if it's different from start date
                    } else if (newShouldRecur && !payload.recurrence_end_date && newEndDate) {
                        payload.recurrence_end_date = getLocalDateString(newEndDate);
                    }
                    await createPayment(projectId, payload);
                }
            } else if (editingPaymentId !== null) {
                await updatePayment(projectId, editingPaymentId, payload);
            } else {
                await createPayment(projectId, payload);
            }

            // Reset form
            resetForm();

            // Reload data
            await loadPayments();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to save payment';
        } finally {
            submitting = false;
        }
    }

    async function handleDelete(paymentId: number) {
        if (!confirm('Delete this payment?')) return;

        try {
            await deletePayment(projectId, paymentId);
            await loadPayments();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to delete payment';
        }
    }

    function formatDate(dateStr: string): string {
        const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
        const date = new Date(year, month - 1, day);
        return date.toLocaleDateString();
    }

    function isFutureDate(dateStr: string): boolean {
        const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
        const date = new Date(year, month - 1, day);
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        return date > today;
    }

    function formatRecurrence(p: PaymentWithContributions): string {
        if (!p.is_recurring) return '';
        const type = p.recurrence_type || 'monthly';
        const interval = p.recurrence_interval || 1;

        // Enhanced patterns
        if (type === 'weekly' && p.recurrence_weekdays && interval <= 4) {
            try {
                const patterns: number[][] = JSON.parse(p.recurrence_weekdays);
                const allDays = new Set(patterns.flat());
                const dayNames = [...allDays].sort((a, b) => a - b).map(d => WEEKDAY_NAMES[d]);
                if (interval === 1) {
                    return `${dayNames.join(', ')} weekly`;
                } else {
                    return `${dayNames.join(', ')} every ${interval} weeks`;
                }
            } catch { /* fall through */ }
        }

        if (type === 'monthly' && p.recurrence_monthdays && interval === 1) {
            try {
                const days: number[] = JSON.parse(p.recurrence_monthdays);
                const formatted = days.map(d => ordinal(d)).join(', ');
                return `${formatted} monthly`;
            } catch { /* fall through */ }
        }

        if (type === 'yearly' && p.recurrence_months && interval === 1) {
            try {
                const months: number[] = JSON.parse(p.recurrence_months);
                const monthNames = months.map(m => MONTH_NAMES[m - 1]);
                return `${monthNames.join(', ')} yearly`;
            } catch { /* fall through */ }
        }

        // Fallback to simple display
        if (interval === 1) {
            return type;
        }
        return `every ${interval} ${type.replace('ly', '')}s`;
    }

    function openImageModal(image: string) {
        modalImage = image;
        showImageModal = true;
    }

    function closeImageModal() {
        showImageModal = false;
        modalImage = null;
    }
</script>

<h2>Payments</h2>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if $canEdit}
    <section class="card">
        <h3>{editingPaymentId !== null ? 'Edit Payment' : 'Add Payment'}</h3>

        {#if $participants.length === 0}
            <p class="warning">Add participants before creating payments.</p>
        {:else}
            <form onsubmit={handleSubmit}>
                <div class="form-row">
                    <div class="field">
                        <label for="payer">Paid by</label>
                        <select id="payer" bind:value={payerId}>
                            {#each $participants as p}
                                <option value={p.id}>{p.name}</option>
                            {/each}
                        </select>
                    </div>

                    <div class="field">
                        <label for="amount">Amount</label>
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
                        <label for="payment-date">Date</label>
                        <input
                            id="payment-date"
                            type="date"
                            bind:value={paymentDate}
                            required
                        />
                    </div>
                </div>

                <div class="field">
                    <label for="description">Description</label>
                    <input
                        id="description"
                        type="text"
                        bind:value={description}
                        placeholder="What was this for?"
                        required
                    />
                </div>

                <!-- Receipt Image -->
                <div class="field">
                    <label for="receipt-input">Receipt Image <span class="hint">(JPEG, PNG, GIF, WebP - max 5MB)</span></label>
                    <div class="receipt-upload">
                        <input
                            type="file"
                            accept="image/jpeg,image/png,image/gif,image/webp"
                            onchange={handleFileChange}
                            id="receipt-input"
                        />
                        {#if receiptPreview}
                            <div class="receipt-preview">
                                <img src={receiptPreview} alt="Receipt preview" />
                                <button type="button" class="clear-btn" onclick={clearReceipt}>&times;</button>
                            </div>
                        {/if}
                    </div>
                </div>

                <!-- Recurrence Section -->
                <div class="recurrence-section">
                    <label class="checkbox-label">
                        <input type="checkbox" bind:checked={isRecurring} />
                        Recurring payment
                    </label>

                    {#if isRecurring}
                        <div class="recurrence-options">
                            <div class="form-row">
                                <div class="field small">
                                    <label for="recurrence-interval">Every</label>
                                    <input id="recurrence-interval" type="number" bind:value={recurrenceInterval} min="1" />
                                </div>

                                <div class="field">
                                    <label for="recurrence-type">Period</label>
                                    <select id="recurrence-type" bind:value={recurrenceType}>
                                        <option value="daily">{recurrenceInterval === 1 ? 'Day' : 'Days'}</option>
                                        <option value="weekly">{recurrenceInterval === 1 ? 'Week' : 'Weeks'}</option>
                                        <option value="monthly">{recurrenceInterval === 1 ? 'Month' : 'Months'}</option>
                                        <option value="yearly">{recurrenceInterval === 1 ? 'Year' : 'Years'}</option>
                                    </select>
                                </div>
                            </div>

                            <!-- Weekly: Weekday selection (1-4 week cycles) -->
                            {#if showWeekdaySelector}
                                <div class="weekday-selector" role="group" aria-labelledby="weekday-selector-label">
                                    <span id="weekday-selector-label" class="selector-label">Select days of the week:</span>
                                    {#each Array(recurrenceInterval) as _, weekIdx}
                                        {#if recurrenceInterval > 1}
                                            <div class="week-label">Week {weekIdx + 1}:</div>
                                        {/if}
                                        <div class="weekday-row">
                                            {#each WEEKDAY_NAMES as day, dayIdx}
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

                            <!-- Monthly: Day of month selection -->
                            {#if showMonthdaySelector}
                                <div class="monthday-selector" role="group" aria-labelledby="monthday-selector-label">
                                    <span id="monthday-selector-label" class="selector-label">Select days of the month:</span>
                                    <div class="monthday-grid">
                                        {#each Array(31) as _, idx}
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
                                        <p class="warning-hint">
                                            Note: For months with fewer days, the last day of the month will be used.
                                        </p>
                                    {/if}
                                </div>
                            {/if}

                            <!-- Yearly: Month selection -->
                            {#if showMonthSelector}
                                <div class="month-selector" role="group" aria-labelledby="month-selector-label">
                                    <span id="month-selector-label" class="selector-label">Select months:</span>
                                    <div class="month-grid">
                                        {#each MONTH_NAMES as monthName, idx}
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

                            <div class="field">
                                <label for="end-date">End date (optional)</label>
                                <input
                                    id="end-date"
                                    type="date"
                                    bind:value={recurrenceEndDate}
                                    min={paymentDate}
                                />
                            </div>
                        </div>
                    {/if}
                </div>

                {#if isInternalTransfer}
                    {@const receiver = $participants.find(p => p.id === receiverAccountId)}
                    <div class="internal-transfer-banner">
                        <span class="transfer-icon">↗</span>
                        <span class="transfer-text">Internal Transfer to {receiver?.name ?? 'Unknown'}</span>
                        <button type="button" class="clear-transfer-btn" onclick={() => { receiverAccountId = null; includeAll(); }}>Cancel</button>
                    </div>
                {/if}

                <div class="split-header">
                    <h4>Split between</h4>
                    <button type="button" class="small-btn" onclick={includeAll}>Include all</button>
                </div>
                <table class="split-table">
                    <thead>
                        <tr>
                            <th>Participant</th>
                            <th>Include</th>
                            <th>Weight</th>
                            <th>Share</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each $participants as p}
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
                                        oninput={(e) => handleWeightChange(p.id, parseFloat(e.currentTarget.value) || 0)}
                                        min="0"
                                        step="0.5"
                                        disabled={!included[p.id]}
                                    />
                                </td>
                                <td class="share">${shares[p.id]?.toFixed(2) ?? '0.00'}</td>
                                <td>
                                    <button
                                        type="button"
                                        class="payback-btn transfer-btn"
                                        onclick={() => transferTo(p.id)}
                                        title={p.account_type === 'pool' ? 'Deposit to pool' : 'Transfer money to this person'}
                                    >{p.account_type === 'pool' ? 'Deposit' : 'Pay back'}</button>
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>

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
                                <input
                                    id="split-from-date"
                                    type="date"
                                    bind:value={splitFromDate}
                                    min={editingPaymentOriginal.payment_date.split('T')[0]}
                                    max={editingPaymentOriginal.recurrence_end_date?.split('T')[0] || ''}
                                />
                                <p class="split-hint">
                                    {#if splitFromDate}
                                        {@const lastBefore = getLastOccurrenceBefore(parseDate(editingPaymentOriginal.payment_date.split('T')[0]), addDays(parseDate(splitFromDate), -1), (recurrenceType || editingPaymentOriginal.recurrence_type || 'monthly'), (recurrenceInterval || editingPaymentOriginal.recurrence_interval || 1))}
                                        {@const firstFrom = getFirstOccurrenceFrom(parseDate(splitFromDate), parseDate(splitFromDate), recurrenceType, recurrenceInterval || 1)}
                                        {#if lastBefore}
                                            Original payment will end on {formatDate(getLocalDateString(lastBefore))}, new payment will start on {formatDate(getLocalDateString(firstFrom))}.
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
                        {submitting ? 'Saving...' : (editingPaymentId !== null ? (useSplitDate ? 'Split & Update' : 'Update Payment') : 'Add Payment')}
                    </button>
                    {#if editingPaymentId !== null}
                        <button type="button" class="cancel-btn" onclick={cancelEditing}>Cancel</button>
                    {/if}
                </div>
            </form>
        {/if}
    </section>
{/if}

<section class="card">
    <h3>Recent Payments</h3>

    {#if loading}
        <p>Loading...</p>
    {:else if payments.length === 0}
        <p class="empty">No payments yet.</p>
    {:else}
        <ul class="payments-list">
            {#each payments as p}
                <li class:editing={editingPaymentId === p.id}>
                    <div class="payment-header">
                        <div class="payment-title">
                            <strong>{p.description}</strong>
                            <div class="payment-icons">
                                {#if p.is_recurring}
                                    <span class="icon recurring" title={formatRecurrence(p)}>&#x21bb;</span>
                                {/if}
                                {#if p.receipt_image}
                                    <button
                                        class="icon-btn"
                                        title="View receipt"
                                        onclick={() => openImageModal(p.receipt_image!)}
                                    >&#x1F9FE;</button>
                                {/if}
                            </div>
                        </div>
                        <span class="amount-group">
                            <span class="amount">${p.amount.toFixed(2)}</span>
                            {#if $canEdit}
                                <button
                                    class="edit-btn"
                                    onclick={() => startEditing(p)}
                                    title="Edit payment"
                                    disabled={editingPaymentId !== null}
                                >&#x2699;</button>
                                <button class="delete-btn" onclick={() => handleDelete(p.id)} title="Delete payment">
                                    &times;
                                </button>
                            {/if}
                        </span>
                    </div>
                    <div class="payment-meta">
                        {#if p.receiver_account_id !== null}
                            {@const receiver = $participants.find(pr => pr.id === p.receiver_account_id)}
                            <span class="transfer-badge">Transfer</span>
                            {p.payer_name ?? 'Unknown'} → {receiver?.name ?? 'Unknown'}
                        {:else}
                            Paid by {p.payer_name ?? 'Unknown'}
                        {/if}
                        {#if p.is_recurring && p.recurrence_end_date}
                            from {formatDate(p.payment_date)} to {formatDate(p.recurrence_end_date)}
                        {:else}
                            {isFutureDate(p.payment_date) ? 'from' : 'on'} {formatDate(p.payment_date)}
                        {/if}
                        {#if p.is_recurring}
                            <span class="recurrence-badge">{formatRecurrence(p)}</span>
                        {/if}
                    </div>
                    <div class="payment-splits">
                        {#each p.contributions as c}
                            <span class="chip">
                                {c.participant_name}: ${c.amount.toFixed(2)}
                            </span>
                        {/each}
                    </div>
                </li>
            {/each}
        </ul>
    {/if}
</section>

<!-- Image Modal -->
{#if showImageModal && modalImage}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        onclick={closeImageModal}
        onkeydown={(e) => e.key === 'Escape' && closeImageModal()}
    >
        <div
            class="modal-content"
            role="button"
            tabindex="0"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
        >
            <button class="modal-close" onclick={closeImageModal}>&times;</button>
            <img src={modalImage} alt="Receipt" />
        </div>
    </div>
{/if}

<style>
    h2 {
        margin-bottom: 1.5rem;
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

    .checkbox-label input[type="checkbox"] {
        width: auto;
    }

    input, select {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
    }

    input:focus, select:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    /* Receipt upload */
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

    .split-date-section {
        background: #f9f9f9;
        border-radius: 8px;
        padding: 1rem;
        margin-bottom: 1rem;
    }

    .split-date-field {
        margin-top: 1rem;
        padding-top: 1rem;
        border-top: 1px solid #eee;
    }

    .split-hint {
        font-size: 0.85rem;
        color: #666;
        margin-top: 0.5rem;
    }

    .split-hint .warning {
        display: inline;
        padding: 0.15rem 0.4rem;
        margin: 0;
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

    .split-table input[type="number"] {
        width: 80px;
        padding: 0.5rem;
    }

    .split-table input[type="checkbox"] {
        width: auto;
    }

    .share {
        font-weight: 600;
        color: var(--accent, #7b61ff);
    }

    .payback-btn {
        padding: 0.2rem 0.5rem;
        background: #e9e4ff;
        border: 1px solid var(--accent, #7b61ff);
        border-radius: 4px;
        font-size: 0.7rem;
        color: var(--accent, #7b61ff);
        cursor: pointer;
    }

    .payback-btn:hover {
        background: var(--accent, #7b61ff);
        color: white;
    }

    .form-actions {
        display: flex;
        gap: 0.5rem;
    }

    button[type="submit"] {
        padding: 0.75rem 1.5rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    button[type="submit"]:hover:not(:disabled) {
        opacity: 0.9;
    }

    button[type="submit"]:disabled {
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

    .payments-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .payments-list li {
        padding: 1rem;
        border-bottom: 1px solid #eee;
    }

    .payments-list li:last-child {
        border-bottom: none;
    }

    .payments-list li.editing {
        background: #f0f8ff;
        border-radius: 8px;
    }

    .payment-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.25rem;
    }

    .payment-title {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .payment-icons {
        display: flex;
        gap: 0.25rem;
    }

    .icon {
        font-size: 0.9rem;
        color: #666;
    }

    .icon.recurring {
        color: var(--accent, #7b61ff);
    }

    .icon-btn {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 0.9rem;
        padding: 0;
    }

    .icon-btn:hover {
        opacity: 0.7;
    }

    .amount-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .amount {
        font-weight: 700;
        color: var(--accent, #7b61ff);
    }

    .edit-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        padding: 0;
        background: transparent;
        border: none;
        border-radius: 50%;
        font-size: 0.85rem;
        line-height: 1;
        color: #999;
        cursor: pointer;
    }

    .edit-btn:hover:not(:disabled) {
        background: #e9e4ff;
        color: var(--accent, #7b61ff);
    }

    .edit-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    .payment-meta {
        font-size: 0.85rem;
        color: #666;
        margin-bottom: 0.5rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .recurrence-badge {
        background: var(--accent, #7b61ff);
        color: white;
        padding: 0.15rem 0.5rem;
        border-radius: 4px;
        font-size: 0.75rem;
    }

    .payment-splits {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
    }

    .chip {
        background: #f0f0f0;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-size: 0.85rem;
    }

    .delete-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        padding: 0;
        background: transparent;
        border: none;
        border-radius: 50%;
        font-size: 1rem;
        line-height: 1;
        color: #999;
        cursor: pointer;
    }

    .delete-btn:hover {
        background: #fee;
        color: #c00;
    }

    .empty {
        color: #666;
        font-style: italic;
    }

    /* Modal */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.8);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }

    .modal-content {
        position: relative;
        max-width: 90vw;
        max-height: 90vh;
    }

    .modal-content img {
        max-width: 100%;
        max-height: 90vh;
        border-radius: 8px;
    }

    .modal-close {
        position: absolute;
        top: -12px;
        right: -12px;
        width: 32px;
        height: 32px;
        border-radius: 50%;
        background: white;
        border: none;
        cursor: pointer;
        font-size: 20px;
        line-height: 1;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
    }

    .modal-close:hover {
        background: #f0f0f0;
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

    .clear-transfer-btn {
        padding: 0.25rem 0.75rem;
        background: white;
        border: 1px solid #81c784;
        border-radius: 4px;
        color: #2e7d32;
        font-size: 0.85rem;
        cursor: pointer;
    }

    .clear-transfer-btn:hover {
        background: #f1f8e9;
    }

    .transfer-badge {
        background: linear-gradient(135deg, #4caf50 0%, #2e7d32 100%);
        color: white;
        padding: 0.15rem 0.5rem;
        border-radius: 4px;
        font-size: 0.75rem;
        font-weight: 600;
    }

    .transfer-btn {
        background: #e8f5e9 !important;
        color: #2e7d32 !important;
        border-color: #81c784 !important;
    }

    .transfer-btn:hover {
        background: #c8e6c9 !important;
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

    .split-date-field input[type="date"] {
        max-width: 200px;
    }

    .split-hint {
        margin-top: 0.5rem;
        margin-bottom: 0;
        font-size: 0.85rem;
        color: #6d4c41;
        font-style: italic;
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
        grid-template-columns: repeat(7, 1fr);
        gap: 0.25rem;
    }

    .monthday-btn {
        aspect-ratio: 1;
        padding: 0.25rem;
        min-width: 32px;
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
</style>
