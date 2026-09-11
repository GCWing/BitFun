import type { AccountOwnerChange } from './RelayHttpClient';
import { useMobileStore } from './store';

/**
 * Reconcile a transport-level account commit with the mobile UI.
 * Returns true when cached workspace/session/chat state belonged to a previous
 * owner and callers should leave any detail page that may still reference it.
 */
export function reconcileAccountOwner(
  change: AccountOwnerChange,
): boolean {
  const store = useMobileStore.getState();
  const initialOwnerConflicts = change.kind === 'initial'
    && change.userId !== null
    && store.authenticatedUserId !== null
    && store.authenticatedUserId !== change.userId;
  const ownerWasReplaced = change.kind === 'replacement'
    || change.kind === 'unavailable'
    || initialOwnerConflicts;

  if (ownerWasReplaced) {
    store.resetForDeviceSwitch();
    // A canonical account id is not user-facing, and the previous username no
    // longer describes the active owner. Wait for a new account login
    // before showing an account label again.
    store.setAuthenticatedUserLabel(null);
  }

  if (change.kind === 'unavailable') {
    store.setAuthenticatedUserId(null);
    store.setControlTarget(null);
    return true;
  }

  store.setAuthenticatedUserId(change.userId);
  if (ownerWasReplaced) store.setControlTarget(null);
  return ownerWasReplaced;
}
