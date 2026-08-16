/**
 * Device surface identity, activation epochs, and cancellation.
 *
 * The frontend was built for exactly one host, and multi-device was retrofitted
 * by mutating that host underneath it. Nothing carried a device dimension, so
 * caches keyed by workspace path or session id collided across devices, and
 * every switch was a destroy-and-rebuild whose in-flight work resumed against a
 * wiped world.
 *
 * This module is the single source of truth for three things every other layer
 * needs:
 *
 * 1. **Identity** — a `DeviceSurfaceId` that names *which device* a piece of
 *    state, cache entry, or negotiated capability belongs to. Anything keyed by
 *    path or session id must be scoped by it.
 * 2. **Epoch** — a monotonic activation number. Switching surfaces activates a
 *    new epoch and aborts the previous one, so stale work can be recognised
 *    instead of racing.
 * 3. **Cancellation** — an `AbortSignal` per activation plus a typed
 *    `SurfaceChangedError`, so callers unwind through one shared boundary
 *    rather than a guard at every call site.
 *
 * Treat this file as a frozen contract: other layers import it, none of them
 * reimplement it.
 */

/** `local` is this machine; any other value is a peer device id. */
export type DeviceSurfaceId = string;

export const LOCAL_SURFACE_ID: DeviceSurfaceId = 'local';

export function surfaceIdForDevice(deviceId: string | null | undefined): DeviceSurfaceId {
  const trimmed = deviceId?.trim();
  return trimmed ? trimmed : LOCAL_SURFACE_ID;
}

export function isLocalSurface(surfaceId: DeviceSurfaceId): boolean {
  return surfaceId === LOCAL_SURFACE_ID;
}

/**
 * Raised when work started under one surface activation is observed after that
 * activation ended. It is a control-flow signal, not a failure: callers must
 * abandon quietly rather than report a product error.
 */
export class SurfaceChangedError extends Error {
  readonly isSurfaceChangedError = true;
  readonly surfaceId: DeviceSurfaceId;
  readonly epoch: number;

  constructor(surfaceId: DeviceSurfaceId, epoch: number, action?: string) {
    super(
      action
        ? `Device surface changed while '${action}' was in flight (${surfaceId}#${epoch})`
        : `Device surface changed (${surfaceId}#${epoch})`,
    );
    this.name = 'SurfaceChangedError';
    this.surfaceId = surfaceId;
    this.epoch = epoch;
  }
}

export function isSurfaceChangedError(error: unknown): error is SurfaceChangedError {
  return (
    error instanceof SurfaceChangedError
    || (typeof error === 'object'
      && error !== null
      && (error as { isSurfaceChangedError?: boolean }).isSurfaceChangedError === true)
  );
}

/** One activation of one device surface. */
export interface SurfaceScope {
  readonly surfaceId: DeviceSurfaceId;
  /** Monotonic across the whole window, not per surface. */
  readonly epoch: number;
  readonly signal: AbortSignal;
  /** Whether this activation is still the rendered one. */
  isCurrent(): boolean;
  /** Throw `SurfaceChangedError` unless this activation is still current. */
  assertCurrent(action?: string): void;
  /** Scope a cache/request key to this activation's surface. */
  key(...parts: Array<string | number | undefined | null>): string;
}

interface MutableScope extends SurfaceScope {
  abort(): void;
}

let epochCounter = 0;
let activeScope: MutableScope = createScope(LOCAL_SURFACE_ID);

type SurfaceChangeListener = (scope: SurfaceScope) => void;
const listeners = new Set<SurfaceChangeListener>();

function createScope(surfaceId: DeviceSurfaceId): MutableScope {
  const epoch = ++epochCounter;
  const controller = new AbortController();
  const scope: MutableScope = {
    surfaceId,
    epoch,
    signal: controller.signal,
    isCurrent: () => activeScope.epoch === epoch,
    assertCurrent: (action?: string) => {
      if (activeScope.epoch !== epoch) {
        throw new SurfaceChangedError(surfaceId, epoch, action);
      }
    },
    key: (...parts) => surfaceScopedKey(surfaceId, ...parts),
    abort: () => controller.abort(),
  };
  return scope;
}

/**
 * Key any per-device cache entry, request dedup entry, or capability record.
 *
 * Workspace paths and session ids are **not** unique across devices: the same
 * repository is routinely checked out at the same path on two machines, and a
 * capability negotiated with one host says nothing about another.
 */
export function surfaceScopedKey(
  surfaceId: DeviceSurfaceId,
  ...parts: Array<string | number | undefined | null>
): string {
  return JSON.stringify([surfaceId, ...parts.map(part => part ?? '')]);
}

export function getActiveSurfaceScope(): SurfaceScope {
  return activeScope;
}

export function getActiveSurfaceId(): DeviceSurfaceId {
  return activeScope.surfaceId;
}

/**
 * Begin rendering `surfaceId`. Aborts the previous activation first, so work
 * belonging to it fails fast instead of completing into a surface that no
 * longer wants it. Re-activating the same surface still produces a new epoch.
 *
 * `beforeNotify` is the synchronous commit seam for the transport and event
 * router. The new scope is already current while it runs, but subscribers are
 * notified only after it returns. This keeps observers from ever seeing a new
 * state container while product requests still use the previous device.
 *
 * The callback must be synchronous and non-throwing: it is the small atomic
 * assignment portion of a switch, never asynchronous activation work.
 */
export function activateSurface(
  surfaceId: DeviceSurfaceId,
  beforeNotify?: (scope: SurfaceScope) => void,
): SurfaceScope {
  const previous = activeScope;
  const next = createScope(surfaceId);
  activeScope = next;
  previous.abort();
  beforeNotify?.(next);
  for (const listener of Array.from(listeners)) {
    try {
      listener(next);
    } catch {
      // A listener must never prevent activation from completing.
    }
  }
  return next;
}

export function onSurfaceActivated(listener: SurfaceChangeListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Run `task` under the current activation and drop its result if the surface
 * moved on. Use at boundaries that write shared state after an await.
 */
export async function runInSurfaceScope<T>(
  scope: SurfaceScope,
  action: string,
  task: () => Promise<T>,
): Promise<T> {
  scope.assertCurrent(action);
  const result = await task();
  scope.assertCurrent(action);
  return result;
}

/** Test-only: return the registry to a pristine local activation. */
export function resetDeviceSurfaceForTest(): void {
  epochCounter = 0;
  activeScope = createScope(LOCAL_SURFACE_ID);
  listeners.clear();
}
