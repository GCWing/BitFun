/**
 * Positions in a Session's ordered stream.
 *
 * Contract: [`docs/architecture/session-projection.md`](../../../../../docs/architecture/session-projection.md).
 *
 * A position is the only thing that decides whether a write may be applied.
 * Nothing here inspects projected content — that inspection is precisely what
 * this replaces.
 */

/**
 * A point in one Runtime process's delivery stream.
 *
 * `streamId` identifies the process. Cursors minted by different processes
 * describe unrelated orderings, so they are never compared; a new `streamId`
 * is a new stream, not progress within the old one.
 */
export interface RuntimePosition {
  streamId: string;
  cursor: number;
}

/** Ordering of two positions in the same stream. */
export type PositionOrder =
  /** `candidate` continues directly from `applied`. */
  | 'next'
  /** `candidate` is ahead, but events between it and `applied` are missing. */
  | 'ahead-with-gap'
  /** `candidate` is at or behind `applied`; it has already been accounted for. */
  | 'not-ahead'
  /** Different streams: no ordering exists between them. */
  | 'unrelated';

export function isRuntimePosition(value: unknown): value is RuntimePosition {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<RuntimePosition>;
  return (
    typeof candidate.streamId === 'string' &&
    candidate.streamId.length > 0 &&
    typeof candidate.cursor === 'number' &&
    Number.isSafeInteger(candidate.cursor) &&
    candidate.cursor >= 0
  );
}

/**
 * Where `candidate` sits relative to what has already been applied.
 *
 * `applied` being `null` adopts `candidate` as the baseline rather than
 * reporting a gap. Cursors are per Session and never reset, so a window that
 * opens an existing Session sees its first live event at a high cursor — that
 * is not a discontinuity in anything this client applied, and treating it as
 * one made every freshly opened Session schedule a repair, fence its own live
 * events behind that request, and appear to stall until the response released
 * them (regression: 2026-08-17).
 *
 * What came before the baseline is history, and the hydrate path owns it.
 */
export function comparePositions(
  applied: RuntimePosition | null,
  candidate: RuntimePosition,
): PositionOrder {
  if (!applied) {
    return 'next';
  }
  if (applied.streamId !== candidate.streamId) {
    return 'unrelated';
  }
  if (candidate.cursor <= applied.cursor) {
    return 'not-ahead';
  }
  return candidate.cursor === applied.cursor + 1 ? 'next' : 'ahead-with-gap';
}

/**
 * The position to record after applying `candidate`.
 *
 * Monotonic by construction: a position that is not ahead leaves the applied
 * position unchanged, so the projection cannot regress no matter which source
 * produced the write.
 */
export function advancePosition(
  applied: RuntimePosition | null,
  candidate: RuntimePosition,
): RuntimePosition {
  switch (comparePositions(applied, candidate)) {
    case 'not-ahead':
      return applied as RuntimePosition;
    case 'unrelated':
      // A different Runtime process supersedes the old ordering wholesale.
      return candidate;
    default:
      return candidate;
  }
}

export function positionsEqual(
  left: RuntimePosition | null,
  right: RuntimePosition | null,
): boolean {
  if (!left || !right) {
    return left === right;
  }
  return left.streamId === right.streamId && left.cursor === right.cursor;
}

export function formatPosition(position: RuntimePosition | null): string {
  return position ? `${position.streamId}@${position.cursor}` : 'unset';
}
