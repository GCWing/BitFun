/**
 * Who is moving the FlowChat viewport right now.
 *
 * Several things move it, and each of them has to know not to fight the
 * others. That was a hand-written predicate per writer, which is O(n²) pairs
 * and fails the moment one is forgotten: a snap back was missing from the
 * anchor's copy, so the anchor undid the snap's own 0.7px of travel, the write
 * cancelled the animation, the cancellation read as a gesture coming to rest,
 * and the snap was re-issued 958 times over 20 seconds without arriving.
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
 * - `snap-back` — returning from the reserved blank to the follow target.
 * - `follow-output` — the continuous writer that follows streaming output.
 * - `layout-correction` — the box or the content around the reader changed:
 *   history arrived above them, or the scroller was resized. One owner because
 *   both are the same act, putting the reader back where the change found them.
 * - `anchor-correction` — a late measurement moved things; put them back.
 *
 * The last two are corrections rather than movements: they hold the viewport
 * for the frame they act in and yield to anything above them, because a
 * movement made on purpose is not a displacement to undo.
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
  'snap-back',
  'follow-output',
  'layout-correction',
  'anchor-correction',
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
 * How long a snap back holds the viewport while it animates.
 *
 * Released on the settle that follows; this is only the backstop for a
 * `scrollend` that never comes, because a snap that is never released would
 * leave the viewport unwritable by everything below it.
 */
export const SNAP_BACK_HOLD_MS = 1_200;

export interface ViewportClaim {
  owner: FlowChatViewportOwner;
  claimedAtMs: number;
  /**
   * When the claim lapses without being renewed.
   *
   * A claim that has to be released explicitly uses `Infinity`. Anything whose
   * end is a wall-clock fact — a gesture going quiet, an animation that may
   * never report completion — carries its own expiry instead, so that a missed
   * release costs a bounded wait rather than a viewport nobody may write.
   */
  expiresAtMs: number;
}

export interface ViewportClaimRequest {
  owner: FlowChatViewportOwner;
  nowMs: number;
  /** Omitted means the claim stands until released. */
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
  return {
    granted: true,
    claim: {
      owner: request.owner,
      claimedAtMs: request.nowMs,
      expiresAtMs: request.holdForMs === undefined
        ? Number.POSITIVE_INFINITY
        : request.nowMs + request.holdForMs,
    },
  };
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
