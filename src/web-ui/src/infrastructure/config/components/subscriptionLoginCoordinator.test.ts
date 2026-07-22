import { describe, expect, it } from 'vitest';
import {
  settleSubscriptionLoginStart,
  SubscriptionLoginCoordinator,
} from './subscriptionLoginCoordinator';

describe('SubscriptionLoginCoordinator', () => {
  it('allows only one provider authorization at a time', () => {
    const coordinator = new SubscriptionLoginCoordinator();
    const codex = coordinator.begin('codex');

    expect(codex).not.toBeNull();
    expect(coordinator.begin('opencode')).toBeNull();
    expect(coordinator.current()).toBe(codex);
  });

  it('does not let stale completion clear a newer operation', () => {
    const coordinator = new SubscriptionLoginCoordinator();
    const codex = coordinator.begin('codex')!;
    coordinator.requestCancel('codex');

    expect(coordinator.begin('opencode')).toBeNull();
    expect(coordinator.current()).toBe(codex);
    expect(coordinator.complete(codex)).toBe(true);
    const opencode = coordinator.begin('opencode')!;
    expect(coordinator.complete(codex)).toBe(false);
    expect(coordinator.current()).toBe(opencode);
    expect(coordinator.complete(opencode)).toBe(true);
    expect(coordinator.current()).toBeNull();
  });

  it('cancels only the matching active provider', () => {
    const coordinator = new SubscriptionLoginCoordinator();
    const active = coordinator.begin('antigravity')!;

    expect(coordinator.requestCancel('codex')).toBeNull();
    expect(coordinator.isCurrent(active)).toBe(true);
    expect(coordinator.requestCancel('antigravity')).toBe(active);
    expect(active.cancelled).toBe(true);
    expect(coordinator.current()).toBe(active);
  });

  it('cancels a backend session created after an early UI cancellation', async () => {
    const coordinator = new SubscriptionLoginCoordinator();
    const operation = coordinator.begin('opencode')!;
    coordinator.requestCancel('opencode');
    let backendCancels = 0;

    const settlement = await settleSubscriptionLoginStart(
      coordinator,
      operation,
      async () => { backendCancels += 1; },
    );

    expect(settlement).toEqual({ shouldContinue: false });
    expect(operation.startSettled).toBe(true);
    expect(backendCancels).toBe(1);
    expect(coordinator.current()).toBe(operation);
  });

  it('continues a start that has not been cancelled', async () => {
    const coordinator = new SubscriptionLoginCoordinator();
    const operation = coordinator.begin('codex')!;

    const settlement = await settleSubscriptionLoginStart(
      coordinator,
      operation,
      async () => { throw new Error('must not cancel'); },
    );

    expect(settlement).toEqual({ shouldContinue: true });
    expect(operation.startSettled).toBe(true);
  });
});
