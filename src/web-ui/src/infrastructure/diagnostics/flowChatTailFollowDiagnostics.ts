/**
 * What following the tail cost, and how rough it looked.
 *
 * Streaming output moves a viewport towards its own bottom, and there are two
 * such viewports in FlowChat: the message list, and the thinking card's own
 * scroll box once its content passes `max-height`. Both face the same question,
 * and the answers differ enough that guessing is not safe:
 *
 * - **How rough is it?** Markdown reflows a line at a time, so an unsmoothed
 *   follow moves in line-sized steps. Whether a smoothing attempt actually
 *   helped is a question about the *distribution* of step sizes, not the mean:
 *   a follow that is silky for a second and then jumps a line reads as jumpy.
 * - **What did it cost?** Changing the height of a list item forces the
 *   virtualizer to re-measure, which re-renders `VirtualMessageList`. Scrolling
 *   *inside* an item does not. That difference decides whether smoothing is
 *   affordable at all, and it is invisible from the outside — the two look
 *   identical on screen and differ by an order of magnitude in commits.
 *
 * So both are recorded together, over the same window, gated on
 * `app.logging.flow_chat_diagnostics`. Read against each other they answer the
 * only question worth asking about a smoothing change: did the steps get
 * smaller, and what did that buy or cost in list commits.
 *
 * Nothing here evaluates a payload while the switch is off, and a follow
 * running at frame rate reports once per window rather than once per frame —
 * sixty entries a second would bury the transition they exist to show.
 */

import { flowChatDiagnostics, isFlowChatDiagnosticsEnabled } from './flowChatDiagnostics';

/** Probe tag for everything about tail following in `flowchat.log`. */
export const FLOWCHAT_TAIL_FOLLOW_PROBE = 'tailFollow';

/**
 * How much following is summarised into one entry.
 *
 * A second is long enough for a rate to mean something and short enough to keep
 * a burst distinguishable from the steady state around it.
 */
export const TAIL_FOLLOW_WINDOW_MS = 1000;

/**
 * Step-size buckets, cut at fractions of a line rather than round numbers.
 *
 * FlowChat body text sits around a 24px line, and a line is the step an
 * unsmoothed follow produces. So the boundaries say something a reader can act
 * on: `over28` is worse than not smoothing at all, `to28` is no better than it,
 * `to12` is a genuine improvement, and `under4` is below what the eye resolves
 * as a step.
 */
export const TAIL_FOLLOW_BUCKET_BOUNDS_PX = [4, 12, 28] as const;

export interface TailFollowStepBuckets {
  under4: number;
  to12: number;
  to28: number;
  over28: number;
}

/** Which follow this is. One window is kept per subject. */
export type TailFollowSubject = 'thinking';

export interface TailFollowStepSample {
  /** How far the scroll offset actually moved on this write. */
  stepPx: number;
  /** How far it was from the tail before the write. Equals `stepPx` when the follow jumps. */
  lagPx: number;
  /**
   * The box was already scrolling internally, so this write changed no layout.
   * False means the element is still growing and every step costs a re-measure.
   */
  innerScroll: boolean;
  /** The write skipped whatever smoothing is in force and went straight to the tail. */
  snapped: boolean;
}

export interface TailFollowWindow {
  startedAtMs: number;
  listCommitsAtStart: number;
  steps: number;
  travelPx: number;
  maxStepPx: number;
  maxLagPx: number;
  snaps: number;
  innerScrollSteps: number;
  buckets: TailFollowStepBuckets;
}

export interface TailFollowWindowSummary {
  forMs: number;
  steps: number;
  stepsPerSec: number;
  travelPx: number;
  avgStepPx: number;
  maxStepPx: number;
  maxLagPx: number;
  snaps: number;
  innerScrollSteps: number;
  listCommits: number;
  listCommitsPerSec: number;
  buckets: TailFollowStepBuckets;
}

/** Round to a tenth, matching the viewport probes. */
export function roundTailFollowPx(value: number): number {
  return Math.round(value * 10) / 10;
}

/**
 * Whether a caller should bother measuring.
 *
 * Sampling a follow means reading `scrollTop` on both sides of the write, and a
 * follow runs every frame. `noteTailFollowStep` refuses the same way, but a
 * caller that would pay for the reads should ask first.
 */
export function isTailFollowDiagnosticsEnabled(): boolean {
  return isFlowChatDiagnosticsEnabled();
}

export function tailFollowStepBucket(stepPx: number): keyof TailFollowStepBuckets {
  const [small, medium, large] = TAIL_FOLLOW_BUCKET_BOUNDS_PX;
  const magnitudePx = Math.abs(stepPx);
  if (magnitudePx < small) return 'under4';
  if (magnitudePx < medium) return 'to12';
  if (magnitudePx < large) return 'to28';
  return 'over28';
}

export function createTailFollowWindow(nowMs: number, listCommits: number): TailFollowWindow {
  return {
    startedAtMs: nowMs,
    listCommitsAtStart: listCommits,
    steps: 0,
    travelPx: 0,
    maxStepPx: 0,
    maxLagPx: 0,
    snaps: 0,
    innerScrollSteps: 0,
    buckets: { under4: 0, to12: 0, to28: 0, over28: 0 },
  };
}

export function accumulateTailFollowStep(
  window: TailFollowWindow,
  sample: TailFollowStepSample,
): TailFollowWindow {
  const bucket = tailFollowStepBucket(sample.stepPx);
  return {
    ...window,
    steps: window.steps + 1,
    travelPx: window.travelPx + Math.abs(sample.stepPx),
    maxStepPx: Math.max(window.maxStepPx, Math.abs(sample.stepPx)),
    maxLagPx: Math.max(window.maxLagPx, Math.abs(sample.lagPx)),
    snaps: window.snaps + (sample.snapped ? 1 : 0),
    innerScrollSteps: window.innerScrollSteps + (sample.innerScroll ? 1 : 0),
    buckets: { ...window.buckets, [bucket]: window.buckets[bucket] + 1 },
  };
}

/**
 * Turn a window into what gets logged.
 *
 * Rates are per second rather than per window so that a short trailing window —
 * the one a stream ends on, which is often the interesting one — is comparable
 * with the full ones before it.
 */
export function summariseTailFollowWindow(
  window: TailFollowWindow,
  nowMs: number,
  listCommits: number,
): TailFollowWindowSummary {
  const forMs = Math.max(1, nowMs - window.startedAtMs);
  const perSec = 1000 / forMs;
  const windowListCommits = listCommits - window.listCommitsAtStart;
  return {
    forMs: Math.round(forMs),
    steps: window.steps,
    stepsPerSec: roundTailFollowPx(window.steps * perSec),
    travelPx: roundTailFollowPx(window.travelPx),
    avgStepPx: window.steps === 0 ? 0 : roundTailFollowPx(window.travelPx / window.steps),
    maxStepPx: roundTailFollowPx(window.maxStepPx),
    maxLagPx: roundTailFollowPx(window.maxLagPx),
    snaps: window.snaps,
    innerScrollSteps: window.innerScrollSteps,
    listCommits: windowListCommits,
    listCommitsPerSec: roundTailFollowPx(windowListCommits * perSec),
    buckets: window.buckets,
  };
}

let listCommits = 0;
const windows = new Map<TailFollowSubject, TailFollowWindow>();
let sweepTimer: number | null = null;

/**
 * One render of the virtualized message list.
 *
 * This is the cost a follow pays for changing an item's height, and the number
 * a follow that scrolls inside an item is claiming not to move. Counted rather
 * than logged: it only means something next to a window of steps.
 */
export function noteFlowListCommit(): void {
  if (!isFlowChatDiagnosticsEnabled()) return;
  listCommits += 1;
}

function emitTailFollowWindow(
  subject: TailFollowSubject,
  window: TailFollowWindow,
  nowMs: number,
): void {
  const summary = summariseTailFollowWindow(window, nowMs, listCommits);
  flowChatDiagnostics.trace({
    hypothesis: FLOWCHAT_TAIL_FOLLOW_PROBE,
    location: `tailFollow.${subject}`,
    message: 'a window of tail following',
    data: () => ({ subject, ...summary }),
  });
}

/**
 * Close out windows nothing has written to for a while.
 *
 * Without this the last window of a stream is never reported, and the end of a
 * stream is where a smoothing change is most likely to misbehave.
 */
function scheduleTailFollowSweep(): void {
  if (sweepTimer !== null || typeof globalThis.window === 'undefined') return;
  sweepTimer = globalThis.window.setTimeout(() => {
    sweepTimer = null;
    const nowMs = performance.now();
    for (const [subject, pending] of [...windows]) {
      if (nowMs - pending.startedAtMs < TAIL_FOLLOW_WINDOW_MS) continue;
      emitTailFollowWindow(subject, pending, nowMs);
      windows.delete(subject);
    }
    if (windows.size > 0) scheduleTailFollowSweep();
  }, TAIL_FOLLOW_WINDOW_MS);
}

/** Record one write of a following scroll offset. */
export function noteTailFollowStep(
  subject: TailFollowSubject,
  sample: TailFollowStepSample,
): void {
  if (!isFlowChatDiagnosticsEnabled()) return;

  const nowMs = performance.now();
  let pending = windows.get(subject);
  if (pending && nowMs - pending.startedAtMs >= TAIL_FOLLOW_WINDOW_MS) {
    emitTailFollowWindow(subject, pending, nowMs);
    pending = undefined;
  }
  windows.set(
    subject,
    accumulateTailFollowStep(pending ?? createTailFollowWindow(nowMs, listCommits), sample),
  );
  scheduleTailFollowSweep();
}

export function resetTailFollowDiagnosticsForTests(): void {
  listCommits = 0;
  windows.clear();
  if (sweepTimer !== null && typeof globalThis.window !== 'undefined') {
    globalThis.window.clearTimeout(sweepTimer);
  }
  sweepTimer = null;
}
