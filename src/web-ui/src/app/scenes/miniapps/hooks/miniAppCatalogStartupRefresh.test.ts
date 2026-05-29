import { describe, expect, it, vi } from 'vitest';

import { scheduleMiniAppCatalogStartupRefresh } from './miniAppCatalogStartupRefresh';

describe('scheduleMiniAppCatalogStartupRefresh', () => {
  it('defers the initial Mini App catalog refresh to idle work', async () => {
    let scheduledTask: ((signal: AbortSignal) => Promise<void>) | null = null;
    const schedule = vi.fn((task: (signal: AbortSignal) => Promise<void>, options) => {
      scheduledTask = task;
      return {
        promise: Promise.resolve(),
        cancel: vi.fn(),
      };
    });
    const refreshApps = vi.fn(async () => undefined);
    const refreshRunningWorkers = vi.fn(async () => undefined);

    scheduleMiniAppCatalogStartupRefresh({
      scheduler: { schedule },
      refreshApps,
      refreshRunningWorkers,
    });

    expect(schedule).toHaveBeenCalledTimes(1);
    expect(schedule.mock.calls[0][1]).toMatchObject({
      idle: true,
      priority: 'low',
      inFlightKey: 'miniapp:startup-catalog-refresh',
    });
    expect(refreshApps).not.toHaveBeenCalled();
    expect(refreshRunningWorkers).not.toHaveBeenCalled();

    await scheduledTask?.(new AbortController().signal);

    expect(refreshApps).toHaveBeenCalledTimes(1);
    expect(refreshRunningWorkers).toHaveBeenCalledTimes(1);
  });
});
