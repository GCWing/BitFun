import { describe, expect, it, vi } from 'vitest';
import { GlobalSearchRegistry } from './globalSearchRegistry';
import type { GlobalSearchProvider } from './types';

const testProvider: GlobalSearchProvider = {
  id: 'test',
  groups: ['actions'],
  search: () => ({ items: [] }),
};

describe('GlobalSearchRegistry', () => {
  it('publishes stable provider snapshots and unregisters contributions', () => {
    const registry = new GlobalSearchRegistry();
    const listener = vi.fn();
    registry.subscribe(listener);

    const unregister = registry.register(testProvider);
    expect(registry.getSnapshot()).toEqual([testProvider]);
    expect(listener).toHaveBeenCalledTimes(1);

    unregister();
    unregister();
    expect(registry.getSnapshot()).toEqual([]);
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it('rejects duplicate provider ids', () => {
    const registry = new GlobalSearchRegistry([testProvider]);
    expect(() => registry.register({ ...testProvider })).toThrow(/already registered/);
  });
});
