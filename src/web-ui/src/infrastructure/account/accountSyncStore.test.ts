import { afterEach, describe, expect, it } from 'vitest';
import { useAccountSyncStore } from './accountSyncStore';

describe('accountSyncStore retry direction', () => {
  afterEach(() => {
    useAccountSyncStore.getState().clear();
  });

  it('retains an upload direction after failure so retry is safe', () => {
    useAccountSyncStore.getState().setSyncing(true);
    useAccountSyncStore.getState().setFailed('relay unavailable');

    const state = useAccountSyncStore.getState();
    expect(state.status).toBe('failed');
    expect(state.lastSyncIsFirstLogin).toBe(true);
  });

  it('retains a download direction and clears it on logout/reset', () => {
    useAccountSyncStore.getState().setSyncing(false);
    useAccountSyncStore.getState().setFailed('relay unavailable');
    expect(useAccountSyncStore.getState().lastSyncIsFirstLogin).toBe(false);

    useAccountSyncStore.getState().clear();
    expect(useAccountSyncStore.getState().lastSyncIsFirstLogin).toBeNull();
    expect(useAccountSyncStore.getState().status).toBe('idle');
  });

  it('invalidates detached completions when a sync is cleared or replaced', () => {
    useAccountSyncStore.getState().setSyncing(false);
    const firstOperation = useAccountSyncStore.getState().operationId;

    useAccountSyncStore.getState().clear();
    expect(useAccountSyncStore.getState().operationId).toBeGreaterThan(firstOperation);

    const clearedOperation = useAccountSyncStore.getState().operationId;
    useAccountSyncStore.getState().setSyncing(true);
    expect(useAccountSyncStore.getState().operationId).toBeGreaterThan(clearedOperation);
  });

  it('ignores progress emitted by an older operation', () => {
    useAccountSyncStore.getState().setSyncing(false);
    const operationId = useAccountSyncStore.getState().operationId;

    useAccountSyncStore.getState().applyProgress({
      operation_id: operationId - 1,
      phase: 'exporting_sessions',
      percent: 90,
    });
    expect(useAccountSyncStore.getState().progress.percent).toBe(0);

    useAccountSyncStore.getState().applyProgress({
      operation_id: operationId,
      phase: 'exporting_sessions',
      percent: 40,
    });
    expect(useAccountSyncStore.getState().progress.percent).toBe(40);
  });
});
