import { afterEach, describe, expect, it } from 'vitest';
import {
  routeSurfaceEvent,
  setActiveSurfaceDeviceId,
} from './deviceSurfaceRouting';
import {
  clearDeviceActivity,
  installDeviceActivityTracking,
  isDeviceBusy,
} from './deviceActivity';

let uninstall: (() => void) | null = null;

function track(): void {
  uninstall = installDeviceActivityTracking();
}

afterEach(() => {
  uninstall?.();
  uninstall = null;
  setActiveSurfaceDeviceId(null);
});

function start(deviceId: string | null, sessionId: string): void {
  routeSurfaceEvent('agentic://dialog-turn-started', {
    sessionId,
    ...(deviceId ? { __openbitfunSourceDeviceId: deviceId } : {}),
  });
}

function finish(deviceId: string | null, sessionId: string): void {
  routeSurfaceEvent('agentic://dialog-turn-completed', {
    sessionId,
    ...(deviceId ? { __openbitfunSourceDeviceId: deviceId } : {}),
  });
}

describe('deviceActivity', () => {
  it('tracks work on a device the UI is not rendering', () => {
    track();
    setActiveSurfaceDeviceId(null);

    start('device-b', 's1');
    expect(isDeviceBusy('device-b')).toBe(true);
    expect(isDeviceBusy(null)).toBe(false);

    finish('device-b', 's1');
    expect(isDeviceBusy('device-b')).toBe(false);
  });

  it('tracks local and remote work at the same time', () => {
    track();
    setActiveSurfaceDeviceId('device-b');

    start(null, 'local-1');
    start('device-b', 'peer-1');

    expect(isDeviceBusy(null)).toBe(true);
    expect(isDeviceBusy('device-b')).toBe(true);
  });

  it('stays busy until every running session on a device ends', () => {
    track();
    start('device-b', 's1');
    start('device-b', 's2');

    finish('device-b', 's1');
    expect(isDeviceBusy('device-b')).toBe(true);

    finish('device-b', 's2');
    expect(isDeviceBusy('device-b')).toBe(false);
  });

  it('drops tracked work for a disconnected device', () => {
    track();
    start('device-b', 's1');
    clearDeviceActivity('device-b');
    expect(isDeviceBusy('device-b')).toBe(false);
  });
});
