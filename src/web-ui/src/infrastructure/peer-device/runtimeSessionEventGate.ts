/**
 * Cursor fence for attaching a UI Surface to a running Runtime Session.
 *
 * A restore request and the live event stream race by definition. While the
 * Runtime snapshot is in flight, live events for that Surface/Session are held
 * here. Snapshot replay establishes the state at `cursor`; finishing the
 * attachment drops already-covered events and releases only newer ones.
 */

import {
  surfaceScopedKey,
  type DeviceSurfaceId,
} from './deviceSurface';

/** Keep in sync with the Rust `SessionEventJournal` delivery-envelope keys. */
export const RUNTIME_EVENT_STREAM_ID_KEY = '__bitfunRuntimeStreamId';
export const RUNTIME_EVENT_CURSOR_KEY = '__bitfunRuntimeEventCursor';

interface QueuedRuntimeEvent {
  sequence: number;
  streamId?: string;
  cursor?: number;
  deliver: () => void;
}

interface RuntimeSessionAttachment {
  generation: number;
  queued: QueuedRuntimeEvent[];
}

interface RuntimeSessionProgress {
  streamId: string;
  cursor: number;
  hasGap: boolean;
}

const attachments = new Map<string, RuntimeSessionAttachment>();
const progress = new Map<string, RuntimeSessionProgress>();
const gapListeners = new Set<(surfaceId: DeviceSurfaceId, sessionId: string) => void>();
let nextGeneration = 0;
let nextSequence = 0;

function attachmentKey(surfaceId: DeviceSurfaceId, sessionId: string): string {
  return surfaceScopedKey(surfaceId, 'runtime-session-attachment', sessionId);
}

function readEventMetadata(payload: unknown): {
  streamId?: string;
  cursor?: number;
} {
  if (!payload || typeof payload !== 'object') {
    return {};
  }
  const record = payload as Record<string, unknown>;
  const streamId = record[RUNTIME_EVENT_STREAM_ID_KEY];
  const cursor = record[RUNTIME_EVENT_CURSOR_KEY];
  return {
    ...(typeof streamId === 'string' && streamId ? { streamId } : {}),
    ...(typeof cursor === 'number' && Number.isSafeInteger(cursor) && cursor >= 0
      ? { cursor }
      : {}),
  };
}

function stripEventMetadata<T>(payload: T): T {
  if (!payload || typeof payload !== 'object') {
    return payload;
  }
  const record = payload as Record<string, unknown>;
  if (
    !(RUNTIME_EVENT_STREAM_ID_KEY in record) &&
    !(RUNTIME_EVENT_CURSOR_KEY in record)
  ) {
    return payload;
  }
  const {
    [RUNTIME_EVENT_STREAM_ID_KEY]: _streamId,
    [RUNTIME_EVENT_CURSOR_KEY]: _cursor,
    ...productPayload
  } = record;
  return productPayload as T;
}

function advanceProgress(
  key: string,
  metadata: { streamId?: string; cursor?: number },
): boolean {
  if (metadata.streamId === undefined || metadata.cursor === undefined) {
    return false;
  }
  const current = progress.get(key);
  if (!current || current.streamId !== metadata.streamId) {
    const hasGap = metadata.cursor > 1;
    progress.set(key, {
      streamId: metadata.streamId,
      cursor: metadata.cursor,
      hasGap,
    });
    return hasGap;
  }
  if (metadata.cursor > current.cursor) {
    const detectedGap = !current.hasGap && metadata.cursor > current.cursor + 1;
    progress.set(key, {
      streamId: current.streamId,
      cursor: metadata.cursor,
      hasGap: current.hasGap || detectedGap,
    });
    return detectedGap;
  }
  return false;
}

function drain(events: QueuedRuntimeEvent[], shouldDeliver: (event: QueuedRuntimeEvent) => boolean) {
  events.sort((left, right) => left.sequence - right.sequence);
  for (const event of events) {
    if (shouldDeliver(event)) {
      event.deliver();
    }
  }
}

export interface RuntimeSessionAttachmentHandle {
  isCurrent(): boolean;
  requiresReplay(snapshot: { streamId: string; cursor: number }): boolean;
  finish(
    snapshot: { streamId: string; cursor: number },
    options?: { projectionCaughtUp?: boolean },
  ): void;
  abort(options?: { discard?: boolean }): void;
}

export function subscribeRuntimeSessionEventGaps(
  listener: (surfaceId: DeviceSurfaceId, sessionId: string) => void,
): () => void {
  gapListeners.add(listener);
  return () => gapListeners.delete(listener);
}

/**
 * Mark the rendered projection as behind the Host journal.
 *
 * Delivery to a product listener is not acceptance. A TextChunk / ToolEvent
 * that the state machine drops still advances the live cursor, and a later
 * snapshot at that same cursor would otherwise skip replay — leaving
 * in-progress tool cards frozen while the Host has moved on.
 */
export function markRuntimeSessionProjectionStale(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): void {
  const key = attachmentKey(surfaceId, sessionId);
  const current = progress.get(key);
  const alreadyStale = current?.hasGap === true;
  progress.set(key, current
    ? { ...current, hasGap: true }
    : { streamId: '', cursor: 0, hasGap: true });
  if (alreadyStale) {
    return;
  }
  for (const listener of gapListeners) {
    listener(surfaceId, sessionId);
  }
}

/**
 * The stream position this Surface/Session has already applied.
 *
 * `null` when nothing usable has been observed yet — no live event carried a
 * cursor, or the position came from a projection marked stale without one. A
 * caller can only ask the Host for an incremental delta when it can name the
 * exact cursor it is contiguous with.
 */
export function readRuntimeSessionProgress(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): { streamId: string; cursor: number } | null {
  const current = progress.get(attachmentKey(surfaceId, sessionId));
  if (!current || !current.streamId) {
    return null;
  }
  return { streamId: current.streamId, cursor: current.cursor };
}

export function isRuntimeSessionAttachmentInFlight(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): boolean {
  return attachments.has(attachmentKey(surfaceId, sessionId));
}

export function isRuntimeSessionProjectionStale(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): boolean {
  return progress.get(attachmentKey(surfaceId, sessionId))?.hasGap === true;
}

export function beginRuntimeSessionAttachment(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): RuntimeSessionAttachmentHandle {
  const key = attachmentKey(surfaceId, sessionId);
  const previous = attachments.get(key);
  // Transfer the fence. Delivering the previous queue here would apply
  // events against a state machine that the newer attach is about to reset,
  // then the new finish() would cover those same cursors and skip replay.
  const carried = previous?.queued ?? [];
  if (previous) {
    attachments.delete(key);
  }

  const generation = ++nextGeneration;
  const attachment: RuntimeSessionAttachment = { generation, queued: carried };
  attachments.set(key, attachment);

  const takeOwnedQueue = (): QueuedRuntimeEvent[] | null => {
    const current = attachments.get(key);
    if (!current || current.generation !== generation) {
      return null;
    }
    attachments.delete(key);
    return current.queued;
  };

  return {
    isCurrent() {
      const current = attachments.get(key);
      return Boolean(current && current.generation === generation);
    },
    requiresReplay(snapshot) {
      const current = progress.get(key);
      return (
        !current ||
        current.streamId !== snapshot.streamId ||
        current.hasGap ||
        current.cursor < snapshot.cursor
      );
    },
    finish(snapshot, options) {
      const queued = takeOwnedQueue();
      if (!queued) {
        return;
      }
      const projectionCaughtUp = options?.projectionCaughtUp !== false;
      progress.set(key, { ...snapshot, hasGap: !projectionCaughtUp });
      drain(queued, event => (
        event.streamId !== snapshot.streamId ||
        event.cursor === undefined ||
        event.cursor > snapshot.cursor
      ));
    },
    abort(options) {
      const queued = takeOwnedQueue();
      if (!queued || options?.discard === true) {
        return;
      }
      drain(queued, () => true);
    },
  };
}

/**
 * Route one already surface-selected transport event through an in-flight
 * Session attachment. Product listeners never see the reserved cursor keys.
 */
export function routeRuntimeSessionEvent<T>(
  surfaceId: DeviceSurfaceId,
  eventName: string,
  payload: T,
  deliver: (payload: T) => void,
): void {
  const productPayload = stripEventMetadata(payload);
  if (!eventName.startsWith('agentic://') && eventName !== 'session_title_generated') {
    deliver(productPayload);
    return;
  }
  if (!payload || typeof payload !== 'object') {
    deliver(productPayload);
    return;
  }
  const sessionId = (payload as Record<string, unknown>).sessionId;
  if (typeof sessionId !== 'string' || !sessionId) {
    deliver(productPayload);
    return;
  }

  const attachment = attachments.get(attachmentKey(surfaceId, sessionId));
  const key = attachmentKey(surfaceId, sessionId);
  const metadata = readEventMetadata(payload);
  const deliverWithProgress = (): void => {
    const gapDetected = advanceProgress(key, metadata);
    deliver(productPayload);
    if (gapDetected) {
      for (const listener of gapListeners) {
        listener(surfaceId, sessionId);
      }
    }
  };
  if (!attachment) {
    deliverWithProgress();
    return;
  }

  attachment.queued.push({
    sequence: ++nextSequence,
    ...metadata,
    deliver: deliverWithProgress,
  });
}

export function resetRuntimeSessionEventGateForTest(): void {
  attachments.clear();
  progress.clear();
  gapListeners.clear();
  nextGeneration = 0;
  nextSequence = 0;
}
