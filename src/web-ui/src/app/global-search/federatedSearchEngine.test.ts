import { describe, expect, it, vi } from 'vitest';
import { runFederatedSearch } from './federatedSearchEngine';
import type { GlobalSearchProvider, GlobalSearchRequest } from './types';

const request: GlobalSearchRequest = {
  rawQuery: 'search',
  query: 'search',
  scope: 'all',
  workspaces: [],
  currentWorkspace: null,
  limitPerGroup: 2,
  tCommon: (key) => key,
  tSettings: (key) => key,
};

function provider(
  id: string,
  search: GlobalSearchProvider['search'],
): GlobalSearchProvider {
  return { id, groups: ['actions'], search };
}

describe('runFederatedSearch', () => {
  it('keeps successful results when another provider fails', async () => {
    const updates = vi.fn();
    const result = await runFederatedSearch([
      provider('healthy', async () => ({
        items: [{
          id: 'action:healthy',
          providerId: 'healthy',
          group: 'actions',
          title: 'Healthy',
          score: 80,
          target: { kind: 'action', actionId: 'settings.open' },
        }],
      })),
      provider('broken', async () => {
        throw new Error('offline');
      }),
    ], request, new AbortController().signal, updates);

    expect(result.items.map((item) => item.id)).toEqual(['action:healthy']);
    expect(result.providerStatus).toEqual({ healthy: 'ready', broken: 'error' });
    expect(result.diagnostics).toEqual([
      expect.objectContaining({ providerId: 'broken', code: 'provider_unavailable' }),
    ]);
    expect(updates).toHaveBeenCalled();
  });

  it('sorts deterministically, deduplicates by stable id, and enforces group budgets', async () => {
    const result = await runFederatedSearch([
      provider('first', () => ({
        items: [
          {
            id: 'action:a', providerId: 'first', group: 'actions', title: 'A', score: 20,
            target: { kind: 'action', actionId: 'settings.open' },
          },
          {
            id: 'action:b', providerId: 'first', group: 'actions', title: 'B', score: 90,
            target: { kind: 'action', actionId: 'settings.shortcuts.open' },
          },
          {
            id: 'action:c', providerId: 'first', group: 'actions', title: 'C', score: 80,
            target: { kind: 'action', actionId: 'surface.browser.open' },
          },
        ],
      })),
      provider('second', () => ({
        items: [{
          id: 'action:a', providerId: 'second', group: 'actions', title: 'A improved', score: 1000,
          target: { kind: 'action', actionId: 'settings.open' },
        }],
      })),
    ], request, new AbortController().signal, () => {});

    expect(result.items.map((item) => item.id)).toEqual(['action:a', 'action:b']);
    expect(result.items[0]?.score).toBe(100);
  });

  it('does not publish provider completions after cancellation', async () => {
    const controller = new AbortController();
    const updates = vi.fn();
    const delayed = provider('delayed', async () => {
      controller.abort();
      return { items: [] };
    });

    const result = await runFederatedSearch([delayed], request, controller.signal, updates);

    expect(updates).toHaveBeenCalledTimes(1);
    expect(result.isSearching).toBe(true);
  });

  it('treats cross-realm AbortError values as cancellation', async () => {
    const updates = vi.fn();
    const result = await runFederatedSearch([
      provider('cancelled', async () => {
        throw Object.assign(new Error('cancelled'), { name: 'AbortError' });
      }),
    ], request, new AbortController().signal, updates);

    expect(result.providerStatus.cancelled).toBe('ready');
    expect(result.isSearching).toBe(false);
    expect(result.diagnostics).toEqual([]);
    expect(updates).toHaveBeenCalledTimes(2);
  });
});
