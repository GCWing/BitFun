/**
 * Who is moving the FlowChat viewport right now.
 *
 * Several things move it, and each of them has to know not to fight the
 * others. That was a hand-written predicate per writer, which is O(n²) pairs
 * and fails the moment one is forgotten: two otherwise valid movements can
 * fight because one writer was missing from another writer's predicate.
 *
 * The pairs collapse into one register. A writer claims the viewport for as
 * long as it is moving it, and everything else asks the register rather than
 * asking about each writer.
 *
 * The register decides *whether* someone may write. It never decides where the
 * viewport should go — targets stay with the writer that owns them, and the
 * correction that restores a reading position stays idempotent. Ordering
 * answers the question idempotency cannot: whether a movement was ours on
 * purpose or something to be undone.
 */

/**
 * Ordered most authoritative first. The order is the design; everything else
 * here is bookkeeping.
 *
 * - `user-gesture` — the reader's own hands, which nothing overrides.
 * - `one-shot-navigation` — a Turn, search hit, or focus request being reached.
 * - `follow-output` — the continuous writer that follows streaming output.
 * - `layout-correction` — the scroller's own box changed and a viewport nobody
 *   was following has to be re-aligned to the end it was resting against.
 *
 * The last is a correction rather than a movement: it holds the viewport for
 * the frame it acts in and yields to anything above it, because a movement made
 * on purpose is not a displacement to undo.
 *
 * **Repairing a displacement is not on this list at all.** Putting the reader
 * back after history arrived above them, or after a late measurement moved the
 * transcript under them, is `canShiftViewport` — a different question, with a
 * different answer for the reader's own gesture. See below.
 *
 * **The opening reveal is not here, and that is deliberate.** It is a phase
 * rather than a writer — the thing actually moving the viewport while the
 * transcript is hidden is follow-output — so ranking it above follow-output
 * would have follow refused by a claim standing in for itself. One slot cannot
 * hold a phase and a writer at the same time, so the reveal stays an explicit
 * condition where corrections are suppressed, alongside this register.
 */
export const FLOWCHAT_VIEWPORT_OWNERS = [
  'user-gesture',
  'one-shot-navigation',
  'follow-output',
  'layout-correction',
] as const;

export type FlowChatViewportOwner = (typeof FLOWCHAT_VIEWPORT_OWNERS)[number];

function priorityOf(owner: FlowChatViewportOwner): number {
  return FLOWCHAT_VIEWPORT_OWNERS.indexOf(owner);
}

/**
 * How long a one-shot navigation keeps the viewport after issuing its aim.
 *
 * The virtualizer re-aims while the measurements under the target move, and
 * those later writes are the same request — so they have to still be allowed
 * when they land. There is no signal for the aim settling, so this is a
 * backstop: long enough to cover the re-aim, short enough that a transcript
 * which then starts streaming is not held off for noticeably long. A gesture
 * ends it immediately regardless, which is the rule that matters.
 */
export const ONE_SHOT_NAVIGATION_HOLD_MS = 600;

/**
 * The owners that give the viewport back.
 *
 * This is the whole of who may hold it indefinitely, because a claim with no
 * expiry and no release is a viewport nobody below it can ever write again.
 * Measured, from the trail this register keeps: a library write at session open
 * took an unbounded claim, and follow-output was refused eleven times over half
 * a second while the transcript stood at the top of its loaded window — the
 * session opened on the wrong Turn and stayed there.
 *
 * Everything else states its own window. A gesture goes quiet, an animation may
 * never report completion, a re-aim runs while measurements settle: each of
 * those ends by wall clock, and saying nothing means the write is instantaneous
 * and owns only itself.
 */
const OWNERS_THAT_RELEASE: ReadonlySet<FlowChatViewportOwner> = new Set([
  'follow-output',
]);

/**
 * How long a claim stands when the writer does not say.
 *
 * Zero is not "no claim": the write still resolves against whoever holds the
 * viewport, which is the question that decides whether it happens at all. It
 * only means nothing is held afterwards.
 */
export function defaultViewportHoldMs(owner: FlowChatViewportOwner): number {
  return OWNERS_THAT_RELEASE.has(owner) ? Number.POSITIVE_INFINITY : 0;
}

export interface ViewportClaim {
  owner: FlowChatViewportOwner;
  claimedAtMs: number;
  /**
   * When the claim lapses without being renewed.
   *
   * Only an owner that releases carries `Infinity`; see `OWNERS_THAT_RELEASE`.
   * Anything whose end is a wall-clock fact carries its own expiry, so that a
   * missed release costs a bounded wait rather than a viewport nobody may
   * write.
   */
  expiresAtMs: number;
}

export interface ViewportClaimRequest {
  owner: FlowChatViewportOwner;
  nowMs: number;
  /** Omitted takes `defaultViewportHoldMs`, which only a releaser may hold. */
  holdForMs?: number;
}

export interface ViewportClaimOutcome {
  granted: boolean;
  /** The register afterwards, whether or not the claim was granted. */
  claim: ViewportClaim | null;
}

/** The claim still standing at `nowMs`, or null once it has lapsed. */
export function activeViewportClaim(
  claim: ViewportClaim | null,
  nowMs: number,
): ViewportClaim | null {
  if (!claim) return null;
  return nowMs < claim.expiresAtMs ? claim : null;
}

/**
 * Whether `owner` may write the viewport right now.
 *
 * An owner is never blocked by itself: renewing is how a continuous writer
 * holds on, and re-entering is how a correction runs on consecutive frames.
 */
export function canOwnViewport(
  claim: ViewportClaim | null,
  owner: FlowChatViewportOwner,
  nowMs: number,
): boolean {
  const held = activeViewportClaim(claim, nowMs);
  return held === null || priorityOf(owner) <= priorityOf(held.owner);
}

/**
 * Take the viewport, if the register allows it.
 *
 * A refused claim leaves the register untouched — the point of refusing is that
 * the writer does not act, and a refusal that still recorded something would
 * make the next writer's answer depend on who has been turned away.
 */
export function claimViewport(
  claim: ViewportClaim | null,
  request: ViewportClaimRequest,
): ViewportClaimOutcome {
  const held = activeViewportClaim(claim, request.nowMs);
  if (!canOwnViewport(claim, request.owner, request.nowMs)) {
    return { granted: false, claim: held };
  }
  const holdForMs = request.holdForMs ?? defaultViewportHoldMs(request.owner);
  return {
    granted: true,
    claim: {
      owner: request.owner,
      claimedAtMs: request.nowMs,
      expiresAtMs: request.nowMs + holdForMs,
    },
  };
}

/**
 * The owners that hold a target and re-assert it.
 *
 * `follow-output` recomputes its target every frame, and a navigation's aim is
 * re-taken by the virtualizer while the measurements under it settle. Both
 * already say where the reader belongs, so a displacement applied underneath
 * them is either redundant or a fight.
 * `user-gesture` is deliberately not one of them, and that is the whole point
 * of this being a separate question.
 */
const OWNERS_THAT_HOLD_A_TARGET: ReadonlySet<FlowChatViewportOwner> = new Set([
  'one-shot-navigation',
  'follow-output',
]);

/**
 * Whether the viewport may be shifted by a displacement right now.
 *
 * A shift is not a movement and does not go through the priority order. It says
 * "whatever the reader is looking at moved by this much, put it back" — content
 * arriving above them changes what their offset *means*, and restoring that
 * meaning is not competing with anyone for where the viewport should be.
 *
 * Which is why a gesture cannot refuse one. The reader chose a position in the
 * transcript, not a number of pixels; refusing on their behalf leaves them
 * somewhere they did not ask to be. Measured: history pages in only while the
 * reader scrolls up into it, so a gesture holding the viewport refused every
 * compensation there was — 2494px of history arrived, `scrollTop` stayed at 40,
 * and the transcript jumped back thirteen Turns. The boundary then never
 * re-armed, because the reader was still sitting at the head of the window they
 * had just been moved to.
 */
export function canShiftViewport(claim: ViewportClaim | null, nowMs: number): boolean {
  const held = activeViewportClaim(claim, nowMs);
  return held === null || !OWNERS_THAT_HOLD_A_TARGET.has(held.owner);
}

/**
 * Give the viewport back.
 *
 * Only the current holder can, so a writer that was preempted and finishes
 * late cannot clear the claim of whoever took it — that release would be
 * indistinguishable from the new owner having finished, and the register would
 * hand the viewport to the next corrector mid-movement.
 */
export function releaseViewport(
  claim: ViewportClaim | null,
  owner: FlowChatViewportOwner,
): ViewportClaim | null {
  return claim === null || claim.owner === owner ? null : claim;
}
