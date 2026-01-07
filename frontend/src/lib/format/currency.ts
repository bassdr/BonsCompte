import { preferences } from '$lib/stores/preferences';

/**
 * Format a number with the user's decimal separator preference
 */
export function formatNumber(value: number, decimals: number = 2): string {
  const prefs = preferences.get();
  const separator = prefs.decimal_separator || '.';

  const fixed = Math.abs(value).toFixed(decimals);

  if (separator === ',') {
    // European style: comma for decimal, space for thousands
    return fixed.replace('.', ',').replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
  }

  // Default: period for decimal, comma for thousands
  return fixed.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/**
 * Format a currency amount according to user preferences
 */
export function formatCurrency(amount: number): string {
  const prefs = preferences.get();

  const formattedNumber = formatNumber(Math.abs(amount), 2);
  const symbol = prefs.currency_symbol || '$';
  const position = prefs.currency_symbol_position || 'before';
  const isNegative = amount < 0;

  let result: string;
  if (position === 'before') {
    result = `${symbol}${formattedNumber}`;
  } else {
    result = `${formattedNumber} ${symbol}`;
  }

  return isNegative ? `-${result}` : result;
}

/**
 * Format a signed currency amount (for balances)
 * Shows + for positive, - for negative
 */
export function formatSignedCurrency(amount: number): string {
  const formatted = formatCurrency(Math.abs(amount));

  if (amount > 0) return `+${formatted}`;
  if (amount < 0) return `-${formatted}`;
  return formatted;
}

/**
 * Format a currency amount without the sign (absolute value display)
 */
export function formatCurrencyAbs(amount: number): string {
  return formatCurrency(Math.abs(amount));
}
