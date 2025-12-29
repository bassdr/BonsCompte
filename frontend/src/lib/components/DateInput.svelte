<script lang="ts">
	import { preferences } from '$lib/stores/preferences';

	interface Props {
		id?: string;
		value: string; // ISO format YYYY-MM-DD
		required?: boolean;
		disabled?: boolean;
		min?: string; // ISO format YYYY-MM-DD
		max?: string; // ISO format YYYY-MM-DD
		class?: string;
		onchange?: (value: string) => void;
	}

	let { id, value = $bindable(''), required = false, disabled = false, min, max, class: className, onchange }: Props = $props();

	let displayValue = $state('');
	let isValid = $state(true);

	// Update display value when ISO value or format preference changes
	$effect(() => {
		displayValue = formatDateForDisplay(value);
	});

	function formatDateForDisplay(isoDate: string): string {
		if (!isoDate) return '';

		const match = isoDate.match(/^(\d{4})-(\d{2})-(\d{2})$/);
		if (!match) return isoDate;

		const [, year, month, day] = match;
		const format = $preferences.date_format;

		switch (format) {
			case 'mdy':
				return `${month}/${day}/${year}`;
			case 'dmy':
				return `${day}/${month}/${year}`;
			case 'ymd':
				return `${year}/${month}/${day}`;
			case 'iso':
			default:
				return isoDate;
		}
	}

	function parseDisplayDate(display: string): string | null {
		if (!display) return null;

		const format = $preferences.date_format;
		let year: string, month: string, day: string;

		// Try to parse based on format
		const parts = display.split(/[-\/]/);
		if (parts.length !== 3) return null;

		switch (format) {
			case 'mdy':
				[month, day, year] = parts;
				break;
			case 'dmy':
				[day, month, year] = parts;
				break;
			case 'ymd':
				[year, month, day] = parts;
				break;
			case 'iso':
			default:
				[year, month, day] = parts;
				break;
		}

		// Validate
		const y = parseInt(year, 10);
		const m = parseInt(month, 10);
		const d = parseInt(day, 10);

		if (isNaN(y) || isNaN(m) || isNaN(d)) return null;
		if (m < 1 || m > 12) return null;
		if (d < 1 || d > 31) return null;
		if (y < 1900 || y > 2100) return null;

		// Check if date is valid (e.g., not Feb 30)
		const date = new Date(y, m - 1, d);
		if (date.getFullYear() !== y || date.getMonth() !== m - 1 || date.getDate() !== d) {
			return null;
		}

		// Return ISO format
		return `${y.toString().padStart(4, '0')}-${m.toString().padStart(2, '0')}-${d.toString().padStart(2, '0')}`;
	}

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		displayValue = target.value;

		const isoDate = parseDisplayDate(displayValue);
		if (isoDate) {
			// Validate min/max
			if (min && isoDate < min) {
				isValid = false;
				return;
			}
			if (max && isoDate > max) {
				isValid = false;
				return;
			}

			isValid = true;
			value = isoDate;
			onchange?.(isoDate);
		} else if (!displayValue) {
			// Empty is valid if not required
			isValid = !required;
			value = '';
			onchange?.('');
		} else {
			isValid = false;
		}
	}

	function handleBlur() {
		// Reformat on blur if valid
		if (isValid && value) {
			displayValue = formatDateForDisplay(value);
		}
	}

	// Get placeholder based on format
	let placeholder = $derived(() => {
		switch ($preferences.date_format) {
			case 'mdy':
				return 'MM/DD/YYYY';
			case 'dmy':
				return 'DD/MM/YYYY';
			case 'ymd':
				return 'YYYY/MM/DD';
			case 'iso':
			default:
				return 'YYYY-MM-DD';
		}
	});
</script>

<input
	{id}
	type="text"
	bind:value={displayValue}
	oninput={handleInput}
	onblur={handleBlur}
	{required}
	{disabled}
	placeholder={placeholder()}
	class="{className || ''}"
	class:invalid={!isValid}
	inputmode="numeric"
/>

<style>
	input {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid #ddd;
		border-radius: 4px;
		font-family: inherit;
		font-size: 1rem;
	}

	input:focus {
		outline: none;
		border-color: var(--accent, #7b61ff);
	}

	input.invalid {
		border-color: #dc3545;
	}

	input:disabled {
		background: #f5f5f5;
		cursor: not-allowed;
	}
</style>
