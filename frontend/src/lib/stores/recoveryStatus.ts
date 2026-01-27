import { writable } from 'svelte/store';
import { browser } from '$app/environment';
import { getRecoveryStatus, type RecoveryStatus } from '$lib/api';

const DONT_SHOW_AGAIN_KEY = 'bonscompte_recovery_warning_hidden';

// Recovery status store - can be updated from any component
export const recoveryStatus = writable<RecoveryStatus | null>(null);

// Check if user permanently dismissed the warning
export function isDontShowAgain(): boolean {
  if (!browser) return false;
  return localStorage.getItem(DONT_SHOW_AGAIN_KEY) === 'true';
}

// Set "don't show again" preference
export function setDontShowAgain(value: boolean): void {
  if (!browser) return;
  if (value) {
    localStorage.setItem(DONT_SHOW_AGAIN_KEY, 'true');
  } else {
    localStorage.removeItem(DONT_SHOW_AGAIN_KEY);
  }
}

// Clear "don't show again" on logout
export function clearDontShowAgain(): void {
  if (!browser) return;
  localStorage.removeItem(DONT_SHOW_AGAIN_KEY);
}

// Fetch and update the recovery status
export async function refreshRecoveryStatus(): Promise<void> {
  try {
    const status = await getRecoveryStatus();
    recoveryStatus.set(status);
  } catch {
    recoveryStatus.set(null);
  }
}

// Reset on logout
export function resetRecoveryStatus(): void {
  recoveryStatus.set(null);
}
