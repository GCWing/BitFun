import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';
import { ProcessingStatusManager } from './ProcessingStatusManager';

describe('ProcessingStatusManager device surfaces', () => {
  let manager: ProcessingStatusManager;

  beforeEach(() => {
    activateSurface(LOCAL_SURFACE_ID);
    manager = new ProcessingStatusManager();
  });

  it('selects only the rendered surface and notifies with an empty projection', () => {
    manager.registerStatus({
      sessionId: 'same-session',
      status: 'thinking',
      message: 'local',
    });
    const listener = vi.fn();
    manager.addListener(listener);

    activateSurface('peer-b');
    expect(manager.getAllStatuses()).toEqual([]);
    expect(listener).toHaveBeenLastCalledWith([]);
    manager.registerStatus({
      sessionId: 'same-session',
      status: 'thinking',
      message: 'peer',
    });

    activateSurface(LOCAL_SURFACE_ID);
    expect(manager.getSessionStatuses('same-session')).toMatchObject([
      { message: 'local' },
    ]);
  });

  it('clears one detached surface without deleting the local status', () => {
    manager.registerStatus({
      sessionId: 'same-session',
      status: 'thinking',
      message: 'local',
    });
    activateSurface('peer-b');
    manager.registerStatus({
      sessionId: 'same-session',
      status: 'thinking',
      message: 'peer',
    });

    manager.clearSurface('peer-b');
    expect(manager.getAllStatuses()).toEqual([]);
    activateSurface(LOCAL_SURFACE_ID);
    expect(manager.getAllStatuses()).toMatchObject([{ message: 'local' }]);
  });
});
