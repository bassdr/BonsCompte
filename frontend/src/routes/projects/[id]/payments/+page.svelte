<script lang="ts">
	import { page } from '$app/stores';
	import {
		getPayments,
		createPayment,
		updatePayment,
		deletePayment,
		type PaymentWithContributions,
		type CreatePaymentInput
	} from '$lib/api';
	import { participants, canEdit, members } from '$lib/stores/project';
	import { auth } from '$lib/auth';
	import { _, locale } from '$lib/i18n';
	import { get } from 'svelte/store';
	import { formatCurrency } from '$lib/format/currency';
	import { SvelteSet, SvelteDate } from 'svelte/reactivity';

	let payments: PaymentWithContributions[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Edit mode state
	let editingPaymentId = $state<number | null>(null);
	let editingPaymentOriginal = $state<PaymentWithContributions | null>(null);

	// Filter state
	let searchText = $state('');
	let filterPayerId = $state<number | null>(null);
	let filterContributorId = $state<number | null>(null);
	let filterPaymentType = $state<string>(''); // '', 'expense', 'transfer', 'recurring'
	let filterDateFrom = $state('');
	let filterDateTo = $state('');

	// Pagination state
	let paymentsToShow = $state(30);
	const PAYMENTS_PER_PAGE = 30;

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
	let recurrenceCount = $state<number | null>(null);

	// Enhanced recurrence patterns
	// For weekly: array of arrays, one per week in cycle - each inner array contains selected weekdays (0=Sun, 6=Sat)
	let recurrenceWeekdays = $state<number[][]>([]);
	// For monthly: array of selected day numbers (1-31)
	let recurrenceMonthdays = $state<number[]>([]);
	// For yearly: array of selected month numbers (1-12)
	let recurrenceMonths = $state<number[]>([]);

	// Track if user has manually modified recurrence selections
	let userModifiedWeekdays = $state(false);
	let userModifiedMonthdays = $state(false);

	// Pre-computed i18n labels for use in nested blocks (Svelte 5 limitation)
	let weekLabel = $derived($_('payments.week'));
	let userModifiedMonths = $state(false);

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

	// Determine which end constraint wins (most restrictive = earlier date)
	let effectiveEndDate = $derived.by(() => {
		if (!recurrenceEndDate && !endDateFromCount) return null;
		if (!recurrenceEndDate) return endDateFromCount;
		if (!endDateFromCount) return recurrenceEndDate;
		// Return the earlier (more restrictive) date
		return recurrenceEndDate < endDateFromCount ? recurrenceEndDate : endDateFromCount;
	});

	// Show warning when both constraints are set and they differ
	let endDateConflict = $derived.by(() => {
		if (!recurrenceEndDate || !endDateFromCount) return null;
		if (recurrenceEndDate === endDateFromCount) return null;
		// Return which one wins
		return recurrenceEndDate < endDateFromCount ? 'date' : 'count';
	});

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
	function getLocalDateString(date: Date = new SvelteDate()): string {
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

	// Apply all filters
	let filteredPayments = $derived.by(() => {
		let result = payments;

		// Text search (case-insensitive description)
		if (searchText.trim()) {
			const search = searchText.toLowerCase();
			result = result.filter((p) => p.description.toLowerCase().includes(search));
		}

		// Filter by payer
		if (filterPayerId !== null) {
			result = result.filter((p) => p.payer_id === filterPayerId);
		}

		// Filter by contributor (payments where participant contributed)
		if (filterContributorId !== null) {
			result = result.filter((p) =>
				p.contributions.some((c) => c.participant_id === filterContributorId)
			);
		}

		// Filter by payment type
		if (filterPaymentType) {
			if (filterPaymentType === 'expense') {
				result = result.filter((p) => p.receiver_account_id === null);
			} else if (filterPaymentType === 'transfer') {
				result = result.filter((p) => p.receiver_account_id !== null);
			} else if (filterPaymentType === 'recurring') {
				result = result.filter((p) => p.is_recurring);
			}
		}

		// Filter by date range
		if (filterDateFrom) {
			result = result.filter((p) => {
				const paymentDate = p.payment_date.split('T')[0];
				return paymentDate >= filterDateFrom;
			});
		}
		if (filterDateTo) {
			result = result.filter((p) => {
				const paymentDate = p.payment_date.split('T')[0];
				return paymentDate <= filterDateTo;
			});
		}

		return result;
	});

	// Check if any filters are active
	let hasActiveFilters = $derived(
		searchText.trim() !== '' ||
			filterPayerId !== null ||
			filterContributorId !== null ||
			filterPaymentType !== '' ||
			filterDateFrom !== '' ||
			filterDateTo !== ''
	);

	// Count of active filters
	let activeFilterCount = $derived.by(() => {
		let count = 0;
		if (searchText.trim()) count++;
		if (filterPayerId !== null) count++;
		if (filterContributorId !== null) count++;
		if (filterPaymentType) count++;
		if (filterDateFrom) count++;
		if (filterDateTo) count++;
		return count;
	});

	// Apply pagination (unless editing or filters active)
	let visiblePayments = $derived.by(() => {
		// If editing, only show the payment being edited
		if (editingPaymentId !== null) {
			const editingPayment = filteredPayments.find((p) => p.id === editingPaymentId);
			return editingPayment ? [editingPayment] : [];
		}

		// If filters are active, show all filtered results (ignore pagination)
		if (hasActiveFilters) {
			return filteredPayments;
		}

		// Otherwise, apply pagination
		return filteredPayments.slice(0, paymentsToShow);
	});

	// Check if there are more payments to load
	let hasMorePayments = $derived(!hasActiveFilters && filteredPayments.length > paymentsToShow);

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

	function clearFilters() {
		searchText = '';
		filterPayerId = null;
		filterContributorId = null;
		filterPaymentType = '';
		filterDateFrom = '';
		filterDateTo = '';
		paymentsToShow = PAYMENTS_PER_PAGE; // Reset pagination too
	}

	function loadMorePayments() {
		paymentsToShow += PAYMENTS_PER_PAGE;
	}

	// Reset pagination when filters change
	$effect(() => {
		// Watch filter changes
		const _filterDeps = [
			searchText,
			filterPayerId,
			filterContributorId,
			filterPaymentType,
			filterDateFrom,
			filterDateTo
		];
		// Reset to initial page
		paymentsToShow = PAYMENTS_PER_PAGE;
	});

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
				const currentUserMember = $members.find((m) => m.user_id === $auth.user?.id);
				if (currentUserMember?.participant_id) {
					payerId = currentUserMember.participant_id;
				} else if ($participants.length > 0) {
					// Fallback to first participant if user doesn't have a linked participant
					payerId = $participants[0].id;
				}
			}
		}
	});

	// Pre-fill form from URL parameters (for cashflow recommendations)
	$effect(() => {
		const params = $page.url.searchParams;

		if (params.get('prefill') === 'true') {
			const payerIdParam = params.get('payer_id');
			const amountParam = params.get('amount');
			const recurringParam = params.get('recurring');
			const descParam = params.get('description');
			const recurrenceTypeParam = params.get('recurrence_type');
			const recurrenceIntervalParam = params.get('recurrence_interval');

			if (payerIdParam) {
				const id = parseInt(payerIdParam);
				if (!isNaN(id)) {
					payerId = id;
				}
			}

			if (amountParam) {
				amount = amountParam;
			}

			if (recurringParam === 'true') {
				isRecurring = true;
			}

			if (descParam) {
				description = decodeURIComponent(descParam.replace(/\+/g, ' '));
			}

			if (
				recurrenceTypeParam &&
				['daily', 'weekly', 'monthly', 'yearly'].includes(recurrenceTypeParam)
			) {
				recurrenceType = recurrenceTypeParam as 'daily' | 'weekly' | 'monthly' | 'yearly';
			}

			if (recurrenceIntervalParam) {
				const interval = parseInt(recurrenceIntervalParam);
				if (!isNaN(interval) && interval > 0) {
					recurrenceInterval = interval;
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
			// Initialize weekday arrays if empty, wrong size, or date changed and user hasn't modified
			if (recurrenceWeekdays.length !== recurrenceInterval) {
				recurrenceWeekdays = initializeWeekdayArrays(recurrenceInterval, defaultWeekday);
				userModifiedWeekdays = false;
			} else if (!userModifiedWeekdays) {
				// Only update if the value actually changed (avoid infinite loop)
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
				// Only update if the value actually changed (avoid infinite loop)
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
				// Only update if the value actually changed (avoid infinite loop)
				if (recurrenceMonths[0] !== defaultMonth) {
					recurrenceMonths = [defaultMonth];
				}
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
			const participant = $participants.find((p) => p.id === participantId);
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
	let _receiverName = $derived.by(() => {
		if (receiverAccountId === null) return null;
		return $participants.find((p) => p.id === receiverAccountId)?.name ?? 'Unknown';
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
			if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return true;
			// PNG: 89 50 4E 47
			if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47)
				return true;
			// GIF: 47 49 46 (GIF)
			if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return true;
			// WebP: 52 49 46 46 ... 57 45 42 50 (RIFF...WEBP)
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
		recurrenceCount = null;
		recurrenceWeekdays = [];
		recurrenceMonthdays = [];
		recurrenceMonths = [];
		userModifiedWeekdays = false;
		userModifiedMonthdays = false;
		userModifiedMonths = false;
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
			recurrenceType =
				(payment.recurrence_type as 'daily' | 'weekly' | 'monthly' | 'yearly') || 'monthly';
			recurrenceInterval = payment.recurrence_interval || 1;
			recurrenceEndDate = payment.recurrence_end_date || '';

			// Parse enhanced patterns from JSON
			if (payment.recurrence_weekdays) {
				try {
					recurrenceWeekdays = JSON.parse(payment.recurrence_weekdays);
					userModifiedWeekdays = true; // Treat loaded patterns as user-set
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
		userModifiedWeekdays = true;
		const week = recurrenceWeekdays[weekIndex] || [];
		const idx = week.indexOf(day);
		if (idx >= 0) {
			// Don't allow deselecting if it's the only one
			if (week.length > 1) {
				recurrenceWeekdays[weekIndex] = week.filter((d) => d !== day);
			}
		} else {
			recurrenceWeekdays[weekIndex] = [...week, day].sort((a, b) => a - b);
		}
		recurrenceWeekdays = [...recurrenceWeekdays]; // Trigger reactivity
	}

	// Toggle month day
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

	// Toggle month
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

	// Ordinal suffix for day numbers (1st, 2nd, 3rd, etc.)
	function ordinal(n: number): string {
		// For French, just return the number (no suffix except for 1er which we handle separately)
		const currentLocale = get(locale);
		if (currentLocale === 'fr') {
			return n === 1 ? '1er' : n.toString();
		}
		// For English, use traditional ordinal suffixes
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
			return new SvelteDate(startDate);
		}

		// Calculate how many full intervals have passed
		const fullIntervals = Math.ceil(daysFromStart / dayInterval);
		return addDays(startDate, fullIntervals * dayInterval);
	}

	// Add months to a date, clamping to valid day if needed
	function addMonths(date: Date, months: number): Date {
		const result = new SvelteDate(date);
		const targetMonth = result.getMonth() + months;
		result.setMonth(targetMonth);
		// Handle month overflow (e.g., Jan 31 + 1 month should be Feb 28/29, not Mar 3)
		if (result.getMonth() !== ((targetMonth % 12) + 12) % 12) {
			result.setDate(0); // Go to last day of previous month
		}
		return result;
	}

	// Add years to a date, handling Feb 29 -> Feb 28 for non-leap years
	function addYears(date: Date, years: number): Date {
		const result = new SvelteDate(date);
		result.setFullYear(result.getFullYear() + years);
		// Handle Feb 29 in non-leap years
		if (date.getMonth() === 1 && date.getDate() === 29 && result.getDate() !== 29) {
			result.setDate(28);
		}
		return result;
	}

	// Compute the date of the Nth occurrence (1-indexed: count=1 means the start date itself)
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

		// For enhanced patterns, we need to iterate through occurrences
		if (type === 'weekly' && weekdays && weekdays.length > 0 && interval <= 4) {
			return computeNthWeeklyOccurrence(startDate, count, interval, weekdays);
		}

		if (type === 'monthly' && monthdays && monthdays.length > 0 && interval === 1) {
			return computeNthMonthlyOccurrence(startDate, count, monthdays);
		}

		if (type === 'yearly' && months && months.length > 0 && interval === 1) {
			return computeNthYearlyOccurrence(startDate, count, months);
		}

		// Simple recurrence: just add (count-1) intervals
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

	// Compute Nth occurrence for weekly pattern with specific weekdays
	function computeNthWeeklyOccurrence(
		startDate: Date,
		count: number,
		interval: number,
		weekdays: number[][]
	): string {
		let occurrenceCount = 0;
		let currentWeekStart = new SvelteDate(startDate);
		// Move to start of week (Sunday)
		currentWeekStart.setDate(currentWeekStart.getDate() - currentWeekStart.getDay());
		let cycleWeek = 0;

		// Iterate through weeks
		for (let weeks = 0; weeks < 1000; weeks++) {
			// Safety limit
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
			// Move to next week in cycle
			currentWeekStart = addDays(currentWeekStart, 7);
			cycleWeek = (cycleWeek + 1) % interval;
		}
		// Fallback (shouldn't reach here)
		return getLocalDateString(addDays(startDate, count * 7));
	}

	// Compute Nth occurrence for monthly pattern with specific days
	function computeNthMonthlyOccurrence(
		startDate: Date,
		count: number,
		monthdays: number[]
	): string {
		let occurrenceCount = 0;
		let currentYear = startDate.getFullYear();
		let currentMonth = startDate.getMonth();

		for (let m = 0; m < 1000; m++) {
			// Safety limit
			const daysInMonth = new SvelteDate(currentYear, currentMonth + 1, 0).getDate();
			for (const day of monthdays.sort((a, b) => a - b)) {
				const effectiveDay = Math.min(day, daysInMonth);
				const occurrenceDate = new SvelteDate(currentYear, currentMonth, effectiveDay);
				// Only count if >= start date
				if (occurrenceDate >= startDate) {
					occurrenceCount++;
					if (occurrenceCount === count) {
						return getLocalDateString(occurrenceDate);
					}
				}
			}
			// Move to next month
			currentMonth++;
			if (currentMonth > 11) {
				currentMonth = 0;
				currentYear++;
			}
		}
		// Fallback
		return getLocalDateString(addMonths(startDate, count));
	}

	// Compute Nth occurrence for yearly pattern with specific months
	function computeNthYearlyOccurrence(startDate: Date, count: number, months: number[]): string {
		let occurrenceCount = 0;
		const dayOfMonth = startDate.getDate();
		let currentYear = startDate.getFullYear();

		for (let y = 0; y < 1000; y++) {
			// Safety limit
			for (const month of months.sort((a, b) => a - b)) {
				const monthIndex = month - 1; // Convert 1-12 to 0-11
				const daysInMonth = new SvelteDate(currentYear, monthIndex + 1, 0).getDate();
				const effectiveDay = Math.min(dayOfMonth, daysInMonth);
				const occurrenceDate = new SvelteDate(currentYear, monthIndex, effectiveDay);
				// Only count if >= start date
				if (occurrenceDate >= startDate) {
					occurrenceCount++;
					if (occurrenceCount === count) {
						return getLocalDateString(occurrenceDate);
					}
				}
			}
			currentYear++;
		}
		// Fallback
		return getLocalDateString(addYears(startDate, count));
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!amount || parseFloat(amount) <= 0) return;

		submitting = true;
		error = '';

		try {
			const contributions = $participants
				.filter((p) => included[p.id] !== false)
				.map((p) => ({
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
				receiver_account_id: receiverAccountId // Internal transfer support
			};

			if (isRecurring) {
				payload.recurrence_type = recurrenceType;
				payload.recurrence_interval = recurrenceInterval;

				// Use effectiveEndDate which respects both end date and count (most restrictive wins)
				if (effectiveEndDate) {
					payload.recurrence_end_date = effectiveEndDate;
				}

				// Enhanced patterns - only include when applicable
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
				// Enhanced split logic: calculate proper recurrence boundaries
				const originalStartDate = parseDate(editingPaymentOriginal.payment_date.split('T')[0]);
				const splitDate = parseDate(splitFromDate);
				const originalEndDate = editingPaymentOriginal.recurrence_end_date
					? parseDate(editingPaymentOriginal.recurrence_end_date.split('T')[0])
					: null;

				// Use edited recurrence settings if changing, otherwise use original
				const splitRecurrenceType =
					payload.recurrence_type || (editingPaymentOriginal.recurrence_type as string);
				const splitRecurrenceInterval =
					payload.recurrence_interval || editingPaymentOriginal.recurrence_interval || 1;

				// Validate: split date must be on or after first occurrence
				if (splitDate < originalStartDate) {
					error = 'Split date cannot be before the original payment start date';
					submitting = false;
					return;
				}

				// Validate: split date must be on or before current end date (if edited)
				const newEndDate = payload.recurrence_end_date
					? parseDate(payload.recurrence_end_date)
					: originalEndDate;
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
					error =
						'Split date is before the first recurrence. Please choose a date on or after the first occurrence.';
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
					contributions: editingPaymentOriginal.contributions.map((c) => ({
						participant_id: c.participant_id,
						weight: c.weight
					})),
					receipt_image: editingPaymentOriginal.receipt_image ?? undefined,
					is_recurring: originalShouldRecur,
					receiver_account_id: editingPaymentOriginal.receiver_account_id
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
		const date = new SvelteDate(year, month - 1, day);
		return date.toLocaleDateString();
	}

	function isFutureDate(dateStr: string): boolean {
		const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
		const date = new SvelteDate(year, month - 1, day);
		const today = new SvelteDate();
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
				const allDays = new SvelteSet(patterns.flat());
				const dayNames = [...allDays].sort((a, b) => a - b).map((d) => WEEKDAY_NAMES[d]);
				if (interval === 1) {
					return `${dayNames.join(', ')} ${$_('payments.recurrence.weekly')}`;
				} else {
					return `${dayNames.join(', ')} ${$_('payments.recurrence.everyNWeeks', { values: { n: interval } })}`;
				}
			} catch {
				/* fall through */
			}
		}

		if (type === 'monthly' && p.recurrence_monthdays && interval === 1) {
			try {
				const days: number[] = JSON.parse(p.recurrence_monthdays);
				const formatted = days.map((d) => ordinal(d)).join(', ');
				return `${formatted} ${$_('payments.recurrence.monthly')}`;
			} catch {
				/* fall through */
			}
		}

		if (type === 'yearly' && p.recurrence_months && interval === 1) {
			try {
				const months: number[] = JSON.parse(p.recurrence_months);
				const monthNames = months.map((m) => MONTH_NAMES[m - 1]);
				return `${monthNames.join(', ')} ${$_('payments.recurrence.yearly')}`;
			} catch {
				/* fall through */
			}
		}

		// Fallback to simple display
		if (interval === 1) {
			return $_(`payments.recurrence.${type}`);
		}
		// Use proper noun form with correct gender agreement
		const everyKey =
			type === 'daily'
				? 'payments.recurrence.everyNDays'
				: type === 'weekly'
					? 'payments.recurrence.everyNWeeks'
					: type === 'monthly'
						? 'payments.recurrence.everyNMonths'
						: 'payments.recurrence.everyNYears';
		return $_(everyKey, { values: { n: interval } });
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

<h2>{$_('payments.title')}</h2>

{#if error}
	<div class="error">{error}</div>
{/if}

{#if $canEdit}
	<section class="card" class:editing-card={editingPaymentId !== null}>
		<h3>{editingPaymentId !== null ? $_('payments.editPayment') : $_('payments.addPayment')}</h3>

		{#if $participants.length === 0}
			<p class="warning">{$_('payments.addParticipantsFirst')}</p>
		{:else}
			<form onsubmit={handleSubmit}>
				<div class="form-row">
					<div class="field">
						<label for="payer">{$_('payments.paidBy')}</label>
						<select id="payer" bind:value={payerId}>
							{#each $participants as p (p.id)}
								<option value={p.id}>{p.name}</option>
							{/each}
						</select>
					</div>

					<div class="field">
						<label for="amount">{$_('payments.amount')}</label>
						<input id="amount" type="number" bind:value={amount} min="0.01" step="0.01" required />
					</div>

					<div class="field">
						<label for="payment-date">{$_('payments.date')}</label>
						<input id="payment-date" type="date" bind:value={paymentDate} required />
					</div>
				</div>

				<div class="field">
					<label for="description">{$_('payments.description')}</label>
					<input
						id="description"
						type="text"
						bind:value={description}
						placeholder={$_('payments.descriptionPlaceholder')}
						required
					/>
				</div>

				<!-- Receipt Image -->
				<div class="field">
					<label for="receipt-input"
						>{$_('payments.receiptImage')}
						<span class="hint">{$_('payments.imageFormatsHint')}</span></label
					>
					<div class="receipt-upload">
						<input
							type="file"
							accept="image/jpeg,image/png,image/gif,image/webp"
							onchange={handleFileChange}
							id="receipt-input"
						/>
						{#if receiptPreview}
							<div class="receipt-preview">
								<img src={receiptPreview} alt={$_('payments.receiptPreview')} />
								<button type="button" class="clear-btn" onclick={clearReceipt}>&times;</button>
							</div>
						{/if}
					</div>
				</div>

				{#if isInternalTransfer}
					{@const receiver = $participants.find((p) => p.id === receiverAccountId)}
					<div class="internal-transfer-banner">
						<span class="transfer-icon">↗</span>
						<span class="transfer-text"
							>{$_('payments.internalTransferTo')} {receiver?.name ?? $_('common.unknown')}</span
						>
						<button
							type="button"
							class="clear-transfer-btn"
							onclick={() => {
								receiverAccountId = null;
								includeAll();
							}}>{$_('common.cancel')}</button
						>
					</div>
				{/if}

				<div class="split-header">
					<h4>{$_('payments.splitBetween')}</h4>
					<button type="button" class="small-btn" onclick={includeAll}
						>{$_('payments.includeAll')}</button
					>
				</div>
				<table class="split-table">
					<thead>
						<tr>
							<th>{$_('payments.participant')}</th>
							<th>{$_('payments.include')}</th>
							<th>{$_('payments.weight')}</th>
							<th>{$_('payments.share')}</th>
							<th></th>
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
								<td>
									<button
										type="button"
										class="payback-btn transfer-btn"
										onclick={() => transferTo(p.id)}
										title={p.account_type === 'pool'
											? $_('payments.depositToPool')
											: $_('payments.transferToThisPerson')}
										>{p.account_type === 'pool'
											? $_('payments.deposit')
											: $_('payments.payBack')}</button
									>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>

				<!-- Recurrence Section -->
				<div class="recurrence-section">
					<label class="checkbox-label">
						<input type="checkbox" bind:checked={isRecurring} />
						{$_('payments.recurringPayment')}
					</label>

					{#if isRecurring}
						<div class="recurrence-options">
							<div class="form-row">
								<div class="field small">
									<label for="recurrence-interval">{$_('payments.every')}</label>
									<input
										id="recurrence-interval"
										type="number"
										bind:value={recurrenceInterval}
										min="1"
									/>
								</div>

								<div class="field">
									<label for="recurrence-type">{$_('payments.period')}</label>
									<select id="recurrence-type" bind:value={recurrenceType}>
										<option value="daily"
											>{recurrenceInterval === 1 ? $_('payments.day') : $_('payments.days')}</option
										>
										<option value="weekly"
											>{recurrenceInterval === 1
												? $_('payments.week')
												: $_('payments.weeks')}</option
										>
										<option value="monthly"
											>{recurrenceInterval === 1
												? $_('payments.month')
												: $_('payments.months')}</option
										>
										<option value="yearly"
											>{recurrenceInterval === 1
												? $_('payments.year')
												: $_('payments.years')}</option
										>
									</select>
								</div>
							</div>

							<!-- Weekly: Weekday selection (1-4 week cycles) -->
							{#if showWeekdaySelector}
								<div class="weekday-selector" role="group" aria-labelledby="weekday-selector-label">
									<span id="weekday-selector-label" class="selector-label"
										>{$_('payments.selectDaysOfWeek')}</span
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

							<!-- Monthly: Day of month selection -->
							{#if showMonthdaySelector}
								<div
									class="monthday-selector"
									role="group"
									aria-labelledby="monthday-selector-label"
								>
									<span id="monthday-selector-label" class="selector-label"
										>{$_('payments.selectDaysOfMonth')}</span
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
										<p class="warning-hint">
											{$_('payments.monthdayWarning')}
										</p>
									{/if}
								</div>
							{/if}

							<!-- Yearly: Month selection -->
							{#if showMonthSelector}
								<div class="month-selector" role="group" aria-labelledby="month-selector-label">
									<span id="month-selector-label" class="selector-label"
										>{$_('payments.selectMonths')}</span
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
									<label for="end-date">{$_('payments.endDateOptional')}</label>
									<input
										id="end-date"
										type="date"
										bind:value={recurrenceEndDate}
										min={paymentDate}
									/>
								</div>

								<div class="field">
									<label for="recurrence-count">{$_('payments.occurrencesOptional')}</label>
									<input
										id="recurrence-count"
										type="number"
										bind:value={recurrenceCount}
										min="1"
										placeholder={$_('payments.occurrencesPlaceholder')}
									/>
									{#if endDateFromCount}
										<span class="count-hint"
											>{$_('payments.lastOccurrence')}: {formatDate(endDateFromCount)}</span
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
								<input
									id="split-from-date"
									type="date"
									bind:value={splitFromDate}
									min={editingPaymentOriginal.payment_date.split('T')[0]}
									max={editingPaymentOriginal.recurrence_end_date?.split('T')[0] || ''}
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
											Original payment will end on {formatDate(getLocalDateString(lastBefore))}, new
											payment will start on {formatDate(getLocalDateString(firstFrom))}.
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
									? $_('payments.splitAndUpdate')
									: $_('payments.updatePayment')
								: $_('payments.addPayment')}
					</button>
				</div>
			</form>
		{/if}
	</section>
{/if}

<!-- Filters Section -->
{#if !editingPaymentId}
	<section class="card filters">
		<div class="filters-header">
			<h3>{$_('payments.filters')}</h3>
			{#if activeFilterCount > 0}
				<button type="button" class="clear-filters-btn" onclick={clearFilters}>
					{$_('common.clear')} ({activeFilterCount})
				</button>
			{/if}
		</div>

		<div class="filters-grid">
			<!-- Text Search -->
			<div class="filter-field">
				<label for="search-text">{$_('payments.searchDescription')}</label>
				<input
					id="search-text"
					type="text"
					bind:value={searchText}
					placeholder={$_('payments.searchPlaceholder')}
				/>
			</div>

			<!-- Payer Filter -->
			<div class="filter-field">
				<label for="filter-payer">{$_('payments.filterByPayer')}</label>
				<select id="filter-payer" bind:value={filterPayerId}>
					<option value={null}>{$_('payments.allPayers')}</option>
					{#each $participants as p (p.id)}
						<option value={p.id}>{p.name}</option>
					{/each}
				</select>
			</div>

			<!-- Contributor Filter -->
			<div class="filter-field">
				<label for="filter-contributor">{$_('payments.filterByContributor')}</label>
				<select id="filter-contributor" bind:value={filterContributorId}>
					<option value={null}>{$_('payments.allContributors')}</option>
					{#each $participants as p (p.id)}
						<option value={p.id}>{p.name}</option>
					{/each}
				</select>
			</div>

			<!-- Payment Type Filter -->
			<div class="filter-field">
				<label for="filter-type">{$_('payments.filterByType')}</label>
				<select id="filter-type" bind:value={filterPaymentType}>
					<option value="">{$_('payments.allTypes')}</option>
					<option value="expense">{$_('payments.typeExpense')}</option>
					<option value="transfer">{$_('payments.typeTransfer')}</option>
					<option value="recurring">{$_('payments.typeRecurring')}</option>
				</select>
			</div>

			<!-- Date From -->
			<div class="filter-field">
				<label for="filter-date-from">{$_('payments.dateFrom')}</label>
				<input id="filter-date-from" type="date" bind:value={filterDateFrom} />
			</div>

			<!-- Date To -->
			<div class="filter-field">
				<label for="filter-date-to">{$_('payments.dateTo')}</label>
				<input id="filter-date-to" type="date" bind:value={filterDateTo} />
			</div>
		</div>

		{#if activeFilterCount > 0}
			<div class="filter-summary">
				{$_('payments.showingFiltered')}
				{filteredPayments.length} / {payments.length}
			</div>
		{/if}
	</section>
{/if}

<section class="card">
	<h3>{$_('payments.recentPayments')}</h3>

	{#if loading}
		<p>{$_('common.loading')}</p>
	{:else if payments.length === 0}
		<p class="empty">{$_('payments.noPaymentsYet')}</p>
	{:else if filteredPayments.length === 0}
		<div class="empty-state">
			<p class="empty">{$_('payments.noMatchingPayments')}</p>
			<button type="button" class="clear-filters-btn" onclick={clearFilters}>
				{$_('common.clear')}
			</button>
		</div>
	{:else}
		<ul class="payments-list">
			{#each visiblePayments as p (p.id)}
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
										title={$_('payments.viewReceipt')}
										onclick={() => openImageModal(p.receipt_image!)}>&#x1F9FE;</button
									>
								{/if}
							</div>
						</div>
						<span class="amount-group">
							<span class="amount">{formatCurrency(p.amount)}</span>
							{#if $canEdit}
								<button
									class="edit-btn"
									onclick={() => startEditing(p)}
									title={$_('payments.editPayment')}
									disabled={editingPaymentId !== null}>&#x2699;</button
								>
								<button
									class="delete-btn"
									onclick={() => handleDelete(p.id)}
									title={$_('payments.deletePayment')}
								>
									&times;
								</button>
							{/if}
						</span>
					</div>
					<div class="payment-meta">
						{#if p.receiver_account_id !== null}
							{@const receiver = $participants.find((pr) => pr.id === p.receiver_account_id)}
							<span class="transfer-badge">{$_('payments.transfer')}</span>
							{p.payer_name ?? $_('common.unknown')} → {receiver?.name ?? $_('common.unknown')}
						{:else}
							{$_('payments.paidBy')} {p.payer_name ?? $_('common.unknown')}
						{/if}
						{#if p.is_recurring && p.recurrence_end_date}
							{$_('payments.dateRangeFromTo', {
								values: {
									startDate: formatDate(p.payment_date),
									endDate: formatDate(p.recurrence_end_date)
								}
							})}
						{:else if isFutureDate(p.payment_date)}
							{$_('payments.startingFrom', { values: { date: formatDate(p.payment_date) } })}
						{:else}
							{$_('payments.occurringOn', { values: { date: formatDate(p.payment_date) } })}
						{/if}
						{#if p.is_recurring}
							<span class="recurrence-badge">{formatRecurrence(p)}</span>
						{/if}
					</div>
					<div class="payment-splits">
						{#each p.contributions as c (c.participant_id)}
							<span class="chip">
								{c.participant_name}: {formatCurrency(c.amount)}
							</span>
						{/each}
					</div>
				</li>
			{/each}
		</ul>

		<!-- Load More Button -->
		{#if hasMorePayments && !editingPaymentId}
			<div class="load-more-section">
				<button type="button" class="load-more-btn" onclick={loadMorePayments}>
					{$_('payments.loadMore')}
					({paymentsToShow} / {filteredPayments.length})
				</button>
			</div>
		{/if}
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

	.checkbox-label input[type='checkbox'] {
		width: auto;
	}

	input,
	select {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #ddd;
		border-radius: 8px;
		font-size: 1rem;
	}

	input:focus,
	select:focus {
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

	.split-date-field input[type='date'] {
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

	/* Filters Section */
	.filters {
		margin-bottom: 1.5rem;
	}

	.filters-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
	}

	.filters-header h3 {
		margin: 0;
	}

	.clear-filters-btn {
		padding: 0.5rem 1rem;
		background: #f5f5f5;
		border: 1px solid #ddd;
		border-radius: 8px;
		cursor: pointer;
		font-size: 0.9rem;
		color: #666;
		transition: all 0.2s;
	}

	.clear-filters-btn:hover {
		background: #eee;
		color: #333;
	}

	.filters-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: 1rem;
	}

	.filter-field {
		display: flex;
		flex-direction: column;
	}

	.filter-field label {
		font-size: 0.9rem;
		font-weight: 600;
		margin-bottom: 0.5rem;
		color: #666;
	}

	.filter-field input,
	.filter-field select {
		padding: 0.75rem;
		border: 1px solid #ddd;
		border-radius: 8px;
		font-size: 0.9rem;
		background: white;
	}

	.filter-field input:focus,
	.filter-field select:focus {
		outline: none;
		border-color: var(--accent, #7b61ff);
	}

	.filter-summary {
		margin-top: 1rem;
		padding: 0.75rem;
		background: #f9f9f9;
		border-radius: 8px;
		font-size: 0.9rem;
		color: #666;
		text-align: center;
	}

	/* Load More Button */
	.load-more-section {
		display: flex;
		justify-content: center;
		margin-top: 1.5rem;
		padding-top: 1.5rem;
		border-top: 1px solid #eee;
	}

	.load-more-btn {
		padding: 0.75rem 1.5rem;
		background: var(--accent, #7b61ff);
		color: white;
		border: none;
		border-radius: 8px;
		font-size: 0.9rem;
		cursor: pointer;
		transition: opacity 0.2s;
	}

	.load-more-btn:hover {
		opacity: 0.9;
	}

	/* Editing Card Highlight */
	.editing-card {
		border: 2px solid var(--accent, #7b61ff);
		background: rgba(123, 97, 255, 0.05);
	}

	/* Empty State */
	.empty-state {
		text-align: center;
		padding: 2rem 1rem;
	}

	.empty-state .clear-filters-btn {
		margin-top: 1rem;
	}

	/* Tablet responsive styles */
	@media (max-width: 768px) {
		.filters-grid {
			grid-template-columns: 1fr;
		}

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

		.payback-btn {
			padding: 0.2rem 0.4rem;
			font-size: 0.65rem;
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

		.payback-btn {
			padding: 0.3rem 0.3rem;
			font-size: 0.6rem;
			white-space: nowrap;
		}

		.share {
			font-size: 0.8rem;
		}
	}
</style>
