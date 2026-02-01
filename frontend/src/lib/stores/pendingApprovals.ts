import { writable } from 'svelte/store';
import { getActionableApprovals, getPendingMembersToApprove } from '$lib/api';

interface PendingApprovalsState {
  actionableCount: number;
  pendingMembersCount: number;
  showBanner: boolean;
  bannerDismissed: boolean;
}

const initialState: PendingApprovalsState = {
  actionableCount: 0,
  pendingMembersCount: 0,
  showBanner: false,
  bannerDismissed: false
};

function createPendingApprovalsStore() {
  const { subscribe, set, update } = writable<PendingApprovalsState>(initialState);

  return {
    subscribe,

    async refresh() {
      try {
        const [approvals, pendingMembers] = await Promise.all([
          getActionableApprovals(),
          getPendingMembersToApprove()
        ]);

        update((state) => {
          const newActionableCount = approvals.length;
          const newPendingMembersCount = pendingMembers.length;
          const totalCount = newActionableCount + newPendingMembersCount;

          return {
            ...state,
            actionableCount: newActionableCount,
            pendingMembersCount: newPendingMembersCount,
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
