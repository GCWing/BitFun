/**
 * One ordered stream per `(surface, session)`.
 *
 * Contract: [`docs/architecture/session-projection.md`](../../../../../docs/architecture/session-projection.md).
 *
 * This object owns the applied position, the pending queue, and Turn
 * ownership. It decides whether a write may be applied; the caller performs
 * the painting. That split is deliberate for the migration — writers move onto
 * the contract one at a time without their rendering code moving with them.
 *
 * Nothing here reads projected content. A write is admitted because of where
 * it sits in the order, never because of what is currently on screen.
 */

import {
  surfaceScopedKey,
  type DeviceSurfaceId,
} from '@/infrastructure/peer-device/deviceSurface';
import {
  advancePosition,
  comparePositions,
  type RuntimePosition,
} from './position';
import { eventTurnId, TurnOwnership } from './turnOwnership';

/** What the caller should do with an offered write. */
export type StreamAdmission =
  /** Apply it now. */
  | 'apply'
  /** A prefix read is in flight; the stream is holding it and will release it. */
  | 'hold'
  /** Already accounted for, or its Turn is settled. Applying it would regress. */
  | 'drop';

interface HeldWrite {
  sequence: number;
  position: RuntimePosition | null;
  release: () => void;
}

let nextSequence = 0;

export class SessionStream {
  private applied: RuntimePosition | null = null;
  private held: HeldWrite[] | null = null;
  private prefixGeneration = 0;
  private readonly ownership = new TurnOwnership();
  /** Set when a position skipped ahead; cleared when the skipped range arrives. */
  private gapAt: RuntimePosition | null = null;

  constructor(
    readonly surfaceId: DeviceSurfaceId,
    readonly sessionId: string,
  ) {}

  appliedPosition(): RuntimePosition | null {
    return this.applied;
  }

  /**
   * Whether events between the applied position and what has arrived are
   * missing.
   *
   * This is a fact derived from positions, not a flag someone sets after
   * noticing the screen looks wrong.
   */
  hasGap(): boolean {
    return this.gapAt !== null;
  }

  executingTurnIds(): string[] {
    return this.ownership.executingTurnIds();
  }

  /**
   * Offer a positioned write.
   *
   * An unpositioned write (an older Host, or a Session-scoped event that
   * carries no cursor) is always applied: there is no ordering to violate, and
   * refusing it would hide content rather than protect any.
   */
  offer(
    eventName: string,
    payload: unknown,
    position: RuntimePosition | null,
    release: () => void,
  ): StreamAdmission {
    if (this.held) {
      this.held.push({ sequence: ++nextSequence, position, release });
      return 'hold';
    }
    return this.admit(eventName, payload, position);
  }

  private admit(
    eventName: string,
    payload: unknown,
    position: RuntimePosition | null,
  ): StreamAdmission {
    const turnId = eventTurnId(payload);
    const settledHere = (): boolean =>
      turnId !== null && !this.ownership.runtimeMayWrite(turnId);

    if (!position) {
      if (settledHere()) {
        return 'drop';
      }
      this.ownership.observe(eventName, turnId);
      return 'apply';
    }

    // Order first. Ownership facts belong to a Runtime process, so which
    // process this position came from has to be settled before they can be
    // consulted at all.
    const order = comparePositions(this.applied, position);
    if (order === 'not-ahead') {
      return 'drop';
    }
    if (order === 'unrelated') {
      // A new Runtime process. Its ordering supersedes the old one outright,
      // and every ownership fact from the old process is void — a Turn id it
      // reuses is a different Turn as far as this stream is concerned.
      this.ownership.reset();
      this.gapAt = position.cursor > 1 ? position : null;
    } else if (order === 'ahead-with-gap') {
      // A real discontinuity, independent of whether this particular event is
      // admitted below.
      this.gapAt = position;
    } else {
      this.gapAt = null;
    }

    if (settledHere()) {
      // The Turn settled in this same stream. A straggler for it would repaint
      // content the persisted record now owns.
      return 'drop';
    }
    this.applied = advancePosition(this.applied, position);
    this.ownership.observe(eventName, turnId);
    return 'apply';
  }

  /**
   * Begin reading a prefix (a snapshot) or a suffix (a backfill).
   *
   * Writes offered while the read is in flight are held rather than applied,
   * so the read's result cannot interleave with live events that overlap it.
   * An overlapping read supersedes the previous one and inherits its held
   * writes; releasing them here would apply them against a projection the new
   * read is about to establish.
   */
  beginRead(): SessionStreamRead {
    const generation = ++this.prefixGeneration;
    this.held = this.held ?? [];

    const isCurrent = (): boolean => this.prefixGeneration === generation;
    const takeHeld = (): HeldWrite[] | null => {
      if (!isCurrent()) {
        return null;
      }
      const held = this.held ?? [];
      this.held = null;
      return held;
    };

    return {
      isCurrent,
      settle: (position: RuntimePosition | null) => {
        const held = takeHeld();
        if (!held) {
          return;
        }
        if (position) {
          this.applied = advancePosition(this.applied, position);
          this.gapAt = null;
        }
        this.releaseHeld(held);
      },
      abandon: () => {
        const held = takeHeld();
        if (held) {
          this.releaseHeld(held);
        }
      },
    };
  }

  private releaseHeld(held: HeldWrite[]): void {
    held.sort((left, right) => left.sequence - right.sequence);
    for (const write of held) {
      if (!write.position) {
        write.release();
        continue;
      }
      if (comparePositions(this.applied, write.position) === 'not-ahead') {
        continue;
      }
      this.applied = advancePosition(this.applied, write.position);
      write.release();
    }
  }

  /**
   * Record a position reached by applying content the stream did not route —
   * a snapshot prefix or a backfill suffix the caller painted itself.
   */
  commitAppliedPosition(position: RuntimePosition): void {
    this.applied = advancePosition(this.applied, position);
    this.gapAt = null;
  }

  /** Observe ownership for content applied outside `offer`, in order. */
  observeApplied(eventName: string, payload: unknown): void {
    this.ownership.observe(eventName, eventTurnId(payload));
  }

  /**
   * The painter refused content this stream admitted.
   *
   * The one report that does not come from the order. Painting happens outside
   * this object, so a projection can fall behind a position that was legally
   * admitted — a dropped ToolEnd leaves a tool card frozen while the position
   * says everything arrived. Recording it as a gap routes the repair through
   * the same path a lost event uses, instead of a second staleness concept.
   */
  markProjectionBehind(): void {
    this.gapAt = this.applied ?? { streamId: '', cursor: 0 };
  }
}

export interface SessionStreamRead {
  /** False once a newer read superseded this one. */
  isCurrent(): boolean;
  /** Finish at `position` and release everything ahead of it, in order. */
  settle(position: RuntimePosition | null): void;
  /** Give up without advancing; held writes are released unchanged. */
  abandon(): void;
}

const streams = new Map<string, SessionStream>();

function streamKey(surfaceId: DeviceSurfaceId, sessionId: string): string {
  return surfaceScopedKey(surfaceId, 'session-stream', sessionId);
}

export function sessionStream(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): SessionStream {
  const key = streamKey(surfaceId, sessionId);
  const existing = streams.get(key);
  if (existing) {
    return existing;
  }
  const created = new SessionStream(surfaceId, sessionId);
  streams.set(key, created);
  return created;
}

export function peekSessionStream(
  surfaceId: DeviceSurfaceId,
  sessionId: string,
): SessionStream | undefined {
  return streams.get(streamKey(surfaceId, sessionId));
}

/**
 * Drop a surface's streams when its attachment is disposed.
 *
 * Not on a surface switch: a switch changes what is drawn, and the peer keeps
 * running our work. Discarding the stream would forget the position we would
 * need to catch up with on return.
 */
export function discardSessionStreams(surfaceId: DeviceSurfaceId): void {
  for (const [key, stream] of [...streams.entries()]) {
    if (stream.surfaceId === surfaceId) {
      streams.delete(key);
    }
  }
}

export function resetSessionStreamsForTest(): void {
  streams.clear();
  nextSequence = 0;
}
