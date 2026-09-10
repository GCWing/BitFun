import { describe, expect, it } from 'vitest';
import { ModelDiscoveryCoordinator, openCodeOfferingModels, openCodeModelOffering } from './modelDiscoveryCoordinator';
import type { SubscriptionApiOffering } from '@/infrastructure/api/service-api/AIApi';

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

  it('lists a plan across wires and infers protocol from the chosen model', () => {
    const offerings: SubscriptionApiOffering[] = [
      { plan: 'zen', format: 'openai', base_url: '', suggested_model: '', models: [{ id: 'zen-chat' }] },
      { plan: 'go', format: 'openai', base_url: '', suggested_model: '', models: [{ id: 'go-chat' }] },
      { plan: 'go', format: 'anthropic', base_url: '', suggested_model: '', models: [{ id: 'go-messages' }] },
      { plan: 'zen', format: 'responses', base_url: '', suggested_model: '', models: [{ id: 'zen-responses' }] },
    ];
    expect(openCodeOfferingModels(offerings, 'go')).toEqual([{ id: 'go-chat' }, { id: 'go-messages' }]);
    expect(openCodeOfferingModels(offerings, undefined)).toEqual([{ id: 'zen-chat' }, { id: 'zen-responses' }]);
    expect(openCodeModelOffering(offerings, 'go', 'go-messages', 'openai')?.format).toBe('anthropic');
    expect(openCodeModelOffering(offerings, undefined, 'zen-responses')?.format).toBe('responses');
    expect(openCodeModelOffering(offerings, 'go', 'zen-responses')).toBeUndefined();
    expect(openCodeModelOffering(offerings, undefined, 'manual-legacy-model')).toBeUndefined();
  });
});
