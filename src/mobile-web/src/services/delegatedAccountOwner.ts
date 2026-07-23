import type { DelegatedAccountOwnerChange } from './RelayHttpClient';
import { useMobileStore } from './store';

/**
 * Reconcile a transport-level delegated-account commit with the mobile UI.
 * Returns true when cached workspace/session/chat state belonged to a previous
 * owner and callers should leave any detail page that may still reference it.
 */
export function reconcileDelegatedAccountOwner(
  change: DelegatedAccountOwnerChange,
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
    // longer describes the active owner. Wait for a new authenticated pairing
    // before showing an account label again.
    store.setAuthenticatedUserLabel(null);
  }

  if (change.kind === 'unavailable') {
    store.setAuthenticatedUserId(null);
    store.setControlTarget(null);
    return true;
  }

  if (change.userId !== null || change.kind === 'replacement') {
    // A current Desktop reports userId. Legacy responses may omit it; only a
    // confirmed replacement is allowed to clear an already known old owner.
    store.setAuthenticatedUserId(change.userId);
  }

  if (ownerWasReplaced || store.controlTarget === null) {
    store.setControlTarget(change.homeDeviceId ? {
      deviceId: change.homeDeviceId,
      deviceName: null,
      isHome: true,
    } : null);
  }

  return ownerWasReplaced;
}
