import { writable } from 'svelte/store';
import { getActionableApprovals, getPendingMembersToApprove, getPendingRecoveries } from '$lib/api';

interface PendingApprovalsState {
  actionableCount: number;
  pendingMembersCount: number;
  pendingRecoveriesCount: number;
  showBanner: boolean;
  bannerDismissed: boolean;
}

const initialState: PendingApprovalsState = {
  actionableCount: 0,
  pendingMembersCount: 0,
  pendingRecoveriesCount: 0,
  showBanner: false,
  bannerDismissed: false
};

function createPendingApprovalsStore() {
  const { subscribe, set, update } = writable<PendingApprovalsState>(initialState);

  return {
    subscribe,

    async refresh() {
      try {
        const [approvals, pendingMembers, pendingRecoveries] = await Promise.all([
          getActionableApprovals(),
          getPendingMembersToApprove(),
          getPendingRecoveries()
        ]);

        // Only count recoveries where the user hasn't voted yet
        const actionableRecoveries = pendingRecoveries.filter((r) => !r.user_has_voted);

        update((state) => {
          const newActionableCount = approvals.length;
          const newPendingMembersCount = pendingMembers.length;
          const newPendingRecoveriesCount = actionableRecoveries.length;
          const totalCount =
            newActionableCount + newPendingMembersCount + newPendingRecoveriesCount;

          return {
            ...state,
            actionableCount: newActionableCount,
            pendingMembersCount: newPendingMembersCount,
            pendingRecoveriesCount: newPendingRecoveriesCount,
            // Show banner if there are items and it hasn't been dismissed
            showBanner: !state.bannerDismissed && totalCount > 0
          };
        });
      } catch {
        // Silently fail - approvals might not be accessible
        update((state) => ({
          ...state,
          actionableCount: 0,
          pendingMembersCount: 0,
          pendingRecoveriesCount: 0,
          showBanner: false
        }));
      }
    },

    dismissBanner() {
      update((state) => ({
        ...state,
        showBanner: false,
        bannerDismissed: true
      }));
    },

    reset() {
      set(initialState);
    },

    resetForNewUser() {
      set({
        ...initialState,
        bannerDismissed: false
      });
    }
  };
}

export const pendingApprovals = createPendingApprovalsStore();
