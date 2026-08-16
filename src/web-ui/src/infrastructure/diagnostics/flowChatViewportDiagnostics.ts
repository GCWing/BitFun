/**
 * What moved the FlowChat viewport, recorded for after the fact.
 *
 * Every viewport fault this transcript has produced is intermittent, invisible
 * in the DOM once it is over, and identical in the UI to two or three other
 * causes: a Turn that lands and is then dragged away looks exactly like a Turn
 * that never landed. Reproducing one with temporary probes takes a round trip
 * through the user, and the reproduction is rarely the same twice.
 *
 * So the trail is permanent and gated on `app.logging.flow_chat_diagnostics`,
 * the same switch history paging uses. Nothing here evaluates a payload while
 * the switch is off.
 *
 * Two kinds of thing are worth recording, and they are deliberately different:
 *
 * - **Writes**, at the register. Who moved the viewport, where to, and who was
 *   refused. One place, because every deliberate write goes through one place.
 * - **Decisions**, at each writer. Why it wanted to move, and — more
 *   useful — why it did not. A write that never happened leaves nothing at the
 *   register to find, and "nothing happened" has been the symptom more often
 *   than a wrong move has.
 *
 * Both run at frame rate in the worst case, so repeated identical events
 * collapse into one entry per interval carrying what it stands for. The point
 * of the log is the transitions; a thousand copies of a steady state only bury
 * them.
 */

import { flowChatDiagnostics, isFlowChatDiagnosticsEnabled } from './flowChatDiagnostics';

/** Probe tag for everything about the viewport in `flowchat.log`. */
export const FLOWCHAT_VIEWPORT_PROBE = 'viewport';

/** Shortest gap between two emissions of the same repeating event. */
export const VIEWPORT_TRACE_REPEAT_INTERVAL_MS = 500;

/**
 * How long after a placement its result is read back.
 *
 * Long enough for the writers that undo a placement to have done it — the
 * anchor's settle window and the virtualizer's re-aim both act within a few
 * frames — and short enough that the reader's next gesture is not what the
 * sample ends up describing.
 */
export const VIEWPORT_PLACEMENT_SETTLE_MS = 400;

/** Repeat keys tracked at once, past which the least recently seen is dropped. */
const MAX_TRACKED_REPEAT_KEYS = 64;

export interface ViewportTraceProbe {
  /** `subject.event`, e.g. `viewportOwner.write` or `followOutput.enter`. */
  location: string;
  message: string;
  data?: () => Record<string, unknown>;
}

/** Round to a tenth. Anchor corrections that mattered have been 0.7px. */
export function roundViewportPx(value: number): number {
  return Math.round(value * 10) / 10;
}

export function isViewportDiagnosticsEnabled(): boolean {
  return isFlowChatDiagnosticsEnabled();
}

export function traceViewport(probe: ViewportTraceProbe): void {
  flowChatDiagnostics.trace({ hypothesis: FLOWCHAT_VIEWPORT_PROBE, ...probe });
}

export interface ViewportRepeatState {
  lastEmitAtMs: number;
  suppressedCount: number;
  suppressedTravelPx: number;
  firstSuppressedAtMs: number;
}

/** What an emission stands for besides itself. Absent when it stands alone. */
export interface ViewportRepeatSummary {
  suppressedCount: number;
  suppressedTravelPx: number;
  suppressedForMs: number;
}

export interface ViewportRepeatDecision {
  state: ViewportRepeatState;
  /** Null when this sample is folded into a later emission instead. */
  emit: ViewportRepeatSummary | null;
}

/**
 * Whether a repeating event is emitted now or folded into the next one.
 *
 * An event never seen before always emits: the first of a run is the
 * transition, which is the part worth having. Everything after it waits out the
 * interval, and what waited is reported when the interval ends — a count and
 * the travel it accounted for, so that a run of 60 one-pixel corrections is
 * legible as exactly that rather than as one correction.
 *
 * Callers key by the fields that distinguish one run from another, so a change
 * of owner, or of outcome, is a new key and emits at once.
 */
export function coalesceViewportRepeat(
  state: ViewportRepeatState | null,
  sample: { nowMs: number; travelPx?: number },
  intervalMs: number = VIEWPORT_TRACE_REPEAT_INTERVAL_MS,
): ViewportRepeatDecision {
  const fresh: ViewportRepeatState = {
    lastEmitAtMs: sample.nowMs,
    suppressedCount: 0,
    suppressedTravelPx: 0,
    firstSuppressedAtMs: sample.nowMs,
  };

  if (state === null) {
    return {
      state: fresh,
      emit: { suppressedCount: 0, suppressedTravelPx: 0, suppressedForMs: 0 },
    };
  }

  if (sample.nowMs - state.lastEmitAtMs >= intervalMs) {
    return {
      state: fresh,
      emit: {
        suppressedCount: state.suppressedCount,
        suppressedTravelPx: roundViewportPx(state.suppressedTravelPx),
        suppressedForMs: state.suppressedCount === 0
          ? 0
          : Math.round(sample.nowMs - state.firstSuppressedAtMs),
      },
    };
  }

  return {
    state: {
      lastEmitAtMs: state.lastEmitAtMs,
      suppressedCount: state.suppressedCount + 1,
      suppressedTravelPx: state.suppressedTravelPx + Math.abs(sample.travelPx ?? 0),
      firstSuppressedAtMs: state.suppressedCount === 0
        ? sample.nowMs
        : state.firstSuppressedAtMs,
    },
    emit: null,
  };
}

const repeatStates = new Map<string, ViewportRepeatState>();

/**
 * Record something that can happen on every frame.
 *
 * `key` is the run's identity: include whatever makes one run a different run —
 * the owner, the outcome, the direction — and leave out the measurements, which
 * belong in `data`.
 */
export function traceViewportRepeating(
  key: string,
  probe: ViewportTraceProbe & { travelPx?: number },
): void {
  if (!isFlowChatDiagnosticsEnabled()) return;

  const decision = coalesceViewportRepeat(repeatStates.get(key) ?? null, {
    nowMs: performance.now(),
    travelPx: probe.travelPx,
  });
  // Re-inserted so the map's insertion order is recency order.
  repeatStates.delete(key);
  repeatStates.set(key, decision.state);
  if (repeatStates.size > MAX_TRACKED_REPEAT_KEYS) {
    const oldest = repeatStates.keys().next().value;
    if (oldest !== undefined) repeatStates.delete(oldest);
  }

  const summary = decision.emit;
  if (!summary) return;
  traceViewport({
    location: probe.location,
    message: probe.message,
    data: () => ({
      ...(probe.data?.() ?? {}),
      ...(summary.suppressedCount > 0 ? { repeated: summary } : {}),
    }),
  });
}

/**
 * Record a placement together with what became of it.
 *
 * A placement that is immediately undone and one that never happened are the
 * same complaint from the reader's side, and the difference is only visible in
 * the frames afterwards. So the offset is sampled on the next frame and again
 * once things have settled, and the drift from the target is reported with it.
 * Reading that against the register's own writes says who took it away.
 *
 * The placement runs whether or not diagnostics are on; only the sampling is
 * conditional.
 */
export function traceViewportPlacement(
  scroller: HTMLElement | null,
  probe: ViewportTraceProbe & {
    /** Where the placement was aiming, when that is known separately. */
    targetPx?: number;
    /** Longer for an animated placement, which is still travelling at 400ms. */
    settleAfterMs?: number;
  },
  place: () => void,
): void {
  if (!isFlowChatDiagnosticsEnabled() || !scroller) {
    place();
    return;
  }

  const beforePx = scroller.scrollTop;
  place();
  const placedPx = scroller.scrollTop;
  traceViewport({
    location: probe.location,
    message: probe.message,
    data: () => ({
      ...(probe.data?.() ?? {}),
      beforePx: roundViewportPx(beforePx),
      placedPx: roundViewportPx(placedPx),
      ...(probe.targetPx === undefined ? {} : { targetPx: roundViewportPx(probe.targetPx) }),
    }),
  });

  const targetPx = probe.targetPx ?? placedPx;
  let nextFramePx: number | null = null;
  requestAnimationFrame(() => {
    nextFramePx = scroller.scrollTop;
  });
  window.setTimeout(() => {
    const settledPx = scroller.scrollTop;
    traceViewport({
      location: `${probe.location}.outcome`,
      message: 'where the placement ended up',
      data: () => ({
        targetPx: roundViewportPx(targetPx),
        nextFramePx: nextFramePx === null ? null : roundViewportPx(nextFramePx),
        settledPx: roundViewportPx(settledPx),
        driftPx: roundViewportPx(settledPx - targetPx),
      }),
    });
  }, probe.settleAfterMs ?? VIEWPORT_PLACEMENT_SETTLE_MS);
}

export function resetViewportDiagnosticsForTests(): void {
  repeatStates.clear();
}
