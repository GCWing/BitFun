/**
 * Per-device agent activity, derived from the surface event stream.
 *
 * Attached peers keep streaming turn lifecycle events even while the UI
 * renders another device, so the device switcher can show which machines are
 * still working without polling any of them. Local activity uses the same
 * events with a `null` source device.
 *
 * This is a display hint, not an authority: a device the controller is not
 * attached to reports nothing, and a turn that started before attach is only
 * observed once its first lifecycle event arrives.
 */

import { observeSurfaceEvents } from './deviceSurfaceRouting';

/** `null` is the local device. */
export type DeviceActivityKey = string | null;

const TURN_STARTED = 'agentic://dialog-turn-started';
const TURN_ENDED = new Set([
  'agentic://dialog-turn-completed',
  'agentic://dialog-turn-failed',
  'agentic://dialog-turn-cancelled',
  'agentic://dialog-turn-interrupted',
]);

/** Device key -> set of session ids known to be running on it. */
const runningSessionsByDevice = new Map<string, Set<string>>();
const listeners = new Set<() => void>();

/**
 * Map key for the local device. Peers are keyed by their device id, which
 * is always a UUID, so an '@' prefix cannot collide with one.
 */
const LOCAL_KEY = '@local';

function keyOf(deviceId: DeviceActivityKey): string {
  return deviceId === null ? LOCAL_KEY : deviceId;
}

function readSessionId(payload: unknown): string | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }
  const record = payload as Record<string, unknown>;
  const sessionId = record.sessionId ?? record.session_id;
  return typeof sessionId === 'string' && sessionId.length > 0 ? sessionId : null;
}

function notify(): void {
  for (const listener of listeners) {
    try {
      listener();
    } catch {
      // A subscriber must never break activity tracking for the others.
    }
  }
}

function setRunning(deviceId: DeviceActivityKey, sessionId: string, running: boolean): void {
  const key = keyOf(deviceId);
  const sessions = runningSessionsByDevice.get(key);
  if (running) {
    if (sessions?.has(sessionId)) {
      return;
    }
    if (sessions) {
      sessions.add(sessionId);
    } else {
      runningSessionsByDevice.set(key, new Set([sessionId]));
    }
    notify();
    return;
  }
  if (!sessions?.delete(sessionId)) {
    return;
  }
  if (sessions.size === 0) {
    runningSessionsByDevice.delete(key);
  }
  notify();
}

export function isDeviceBusy(deviceId: DeviceActivityKey): boolean {
  return (runningSessionsByDevice.get(keyOf(deviceId))?.size ?? 0) > 0;
}

/** Drop tracked work for a device that went offline or was disconnected. */
export function clearDeviceActivity(deviceId: DeviceActivityKey): void {
  if (runningSessionsByDevice.delete(keyOf(deviceId))) {
    notify();
  }
}

export function subscribeDeviceActivity(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

let installed: (() => void) | null = null;

/** Start deriving activity from the surface event stream. Idempotent. */
export function installDeviceActivityTracking(): () => void {
  if (installed) {
    return installed;
  }
  const dispose = observeSurfaceEvents((event, sourceDeviceId, payload) => {
    if (event !== TURN_STARTED && !TURN_ENDED.has(event)) {
      return;
    }
    const sessionId = readSessionId(payload);
    if (!sessionId) {
      return;
    }
    setRunning(sourceDeviceId, sessionId, event === TURN_STARTED);
  });
  installed = () => {
    dispose();
    installed = null;
    runningSessionsByDevice.clear();
  };
  return installed;
}
