import { describe, expect, it } from 'vitest';
import { ModelDiscoveryCoordinator } from './modelDiscoveryCoordinator';

describe('model discovery', () => {
  it('allows retry after failure and explicit refresh after success', () => {
    const coordinator = new ModelDiscoveryCoordinator();
    const failed = coordinator.begin('account-a')!;
    coordinator.complete(failed, false);
    const retry = coordinator.begin('account-a')!;
    expect(retry).not.toBeNull();
    coordinator.complete(retry, true);
    expect(coordinator.begin('account-a')).toBeNull();
    expect(coordinator.begin('account-a', true)).not.toBeNull();
  });

  it('rejects old responses even when a reset reopens the same account', () => {
    const coordinator = new ModelDiscoveryCoordinator();
    const first = coordinator.begin('account-a')!;
    coordinator.reset();
    const second = coordinator.begin('account-a')!;
    expect(coordinator.isCurrent(first)).toBe(false);
    expect(coordinator.complete(first, true)).toBe(false);
    expect(coordinator.isCurrent(second)).toBe(true);
    const third = coordinator.begin('account-b')!;
    expect(coordinator.complete(second, false)).toBe(false);
    expect(coordinator.isCurrent(third)).toBe(true);
  });

});
