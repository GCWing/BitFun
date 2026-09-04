/**
 * Transport-side adapter onto a Session's ordered stream.
 *
 * Contract: [`docs/architecture/session-projection.md`](../../../../../docs/architecture/session-projection.md).
 *
 * This module used to own its own copy of the stream position, its own gap
 * flag, and its own fence. All three now live on `SessionStream`, the single
 * owner of what has been applied for one `(surface, session)`. What remains
 * here is the transport concern: reading the reserved cursor keys off a
 * delivery envelope, and stripping them before product listeners see them.
 */

import type { DeviceSurfaceId } from './deviceSurface';
import {
  peekSessionStream,
  resetSessionStreamsForTest,
  sessionStream,
  type SessionStreamRead,
} from '@/flow_chat/session-stream/SessionStream';
import {
  isRuntimePosition,
  type RuntimePosition,
} from '@/flow_chat/session-stream/position';

/** Keep in sync with the Rust `SessionEventJournal` delivery-envelope keys. */
export const RUNTIME_EVENT_STREAM_ID_KEY = '__openbitfunRuntimeStreamId';
export const RUNTIME_EVENT_CURSOR_KEY = '__openbitfunRuntimeEventCursor';

const gapListeners = new Set<(surfaceId: DeviceSurfaceId, sessionId: string) => void>();

function readEventPosition(payload: unknown): RuntimePosition | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }
  const record = payload as Record<string, unknown>;
  const candidate = {
    streamId: record[RUNTIME_EVENT_STREAM_ID_KEY],
    cursor: record[RUNTIME_EVENT_CURSOR_KEY],
  };
  return isRuntimePosition(candidate) ? candidate : null;
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

function notifyGap(surfaceId: DeviceSurfaceId, sessionId: string): void {
  for (const listener of gapListeners) {
    listener(surfaceId, sessionId);
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
 * Report that the painter refused content the stream admitted.
 *
 * Delivery is not acceptance: a TextChunk or ToolEvent the state machine drops
 * still advanced the position, so without this the position would claim a
 * screen state that never happened. Recording it as a gap routes the repair
 * through the same path a lost event uses.
 */
export function markRuntimeSessionProjectionStale(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): void {
  const stream = sessionStream(surfaceId, sessionId);
  if (stream.hasGap()) {
    return;
  }
  stream.markProjectionBehind();
  notifyGap(surfaceId, sessionId);
}

export function isRuntimeSessionProjectionStale(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): boolean {
  return peekSessionStream(surfaceId, sessionId)?.hasGap() === true;
}

/**
 * The position this Surface/Session has applied, or `null` when nothing
 * positioned has been seen — a caller can only ask a Host for a delta when it
 * can name the exact position it is contiguous with.
 */
export function readRuntimeSessionProgress(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): RuntimePosition | null {
  return peekSessionStream(surfaceId, sessionId)?.appliedPosition() ?? null;
}

const inFlightReads = new Map<string, SessionStreamRead>();

function readKey(surfaceId: DeviceSurfaceId, sessionId: string): string {
  return JSON.stringify([surfaceId, sessionId]);
}

export function isRuntimeSessionAttachmentInFlight(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): boolean {
  return inFlightReads.get(readKey(surfaceId, sessionId))?.isCurrent() === true;
}

export function beginRuntimeSessionAttachment(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): RuntimeSessionAttachmentHandle {
  const stream = sessionStream(surfaceId, sessionId);
  const read = stream.beginRead();
  inFlightReads.set(readKey(surfaceId, sessionId), read);

  return {
    isCurrent: () => read.isCurrent(),
    requiresReplay(snapshot) {
      // Under the contract this is ordering, not inspection: replay when the
      // snapshot reaches past what is applied, when it belongs to another
      // Runtime process, or when the painter reported it fell behind.
      const applied = stream.appliedPosition();
      if (stream.hasGap() || !applied) {
        return true;
      }
      return applied.streamId !== snapshot.streamId || applied.cursor < snapshot.cursor;
    },
    finish(snapshot, options) {
      read.settle(isRuntimePosition(snapshot) ? snapshot : null);
      if (options?.projectionCaughtUp === false) {
        // Mark only. Notifying here would schedule a repair for a read that
        // just ran, and the caller reports it through
        // `markRuntimeSessionProjectionStale` — which dedupes against this
        // same flag and correctly stays silent.
        stream.markProjectionBehind();
      }
    },
    abort(options) {
      if (options?.discard === true) {
        // Held writes belong to whichever read supersedes this one; releasing
        // them here would apply them against a projection it is rebuilding.
        read.settle(null);
        return;
      }
      read.abandon();
    },
  };
}

/**
 * Route one already surface-selected transport event through its Session
 * stream. Product listeners never see the reserved cursor keys.
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

  const stream = sessionStream(surfaceId, sessionId);
  const hadGap = stream.hasGap();
  const admission = stream.offer(
    eventName,
    payload,
    readEventPosition(payload),
    () => deliver(productPayload),
  );
  if (admission === 'apply') {
    deliver(productPayload);
  }
  if (!hadGap && stream.hasGap()) {
    notifyGap(surfaceId, sessionId);
  }
}

export function resetRuntimeSessionEventGateForTest(): void {
  gapListeners.clear();
  inFlightReads.clear();
  // Positions live on the streams now, so clearing only this module's maps
  // would leak applied state between tests.
  resetSessionStreamsForTest();
}
