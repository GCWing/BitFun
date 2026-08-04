import { describe, expect, it, vi } from 'vitest';
import {
  confirmCriticalOperationExit,
  registerCriticalOperationExitGuard,
} from './criticalOperationExitGuard';

describe('criticalOperationExitGuard', () => {
  it('allows exit when there are no active operations', async () => {
    await expect(confirmCriticalOperationExit()).resolves.toBe(true);
  });

  it('blocks exit while an operation asks the user to keep waiting', async () => {
    const unregister = registerCriticalOperationExitGuard(async () => false);
    await expect(confirmCriticalOperationExit()).resolves.toBe(false);
    unregister();
    await expect(confirmCriticalOperationExit()).resolves.toBe(true);
  });

  it('stops after the first operation blocks exit', async () => {
    const later = vi.fn(async () => true);
    const unregisterFirst = registerCriticalOperationExitGuard(async () => false);
    const unregisterLater = registerCriticalOperationExitGuard(later);
    await expect(confirmCriticalOperationExit()).resolves.toBe(false);
    expect(later).not.toHaveBeenCalled();
    unregisterFirst();
    unregisterLater();
  });
});
