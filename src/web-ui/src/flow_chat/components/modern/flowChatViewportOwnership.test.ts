import { describe, expect, it } from 'vitest';
import {
  activeViewportClaim,
  canOwnViewport,
  canShiftViewport,
  claimViewport,
  defaultViewportHoldMs,
  FLOWCHAT_VIEWPORT_OWNERS,
  releaseViewport,
  type ViewportClaim,
} from './flowChatViewportOwnership';

const NOW = 1_000;

/**
 * A claim already standing. An omitted hold is the shape only an owner that
 * releases can have — see `defaultViewportHoldMs`.
 */
function heldBy(
  owner: ViewportClaim['owner'],
  options?: { holdForMs?: number; atMs?: number },
): ViewportClaim {
  const claimedAtMs = options?.atMs ?? NOW;
  return {
    owner,
    claimedAtMs,
    expiresAtMs: options?.holdForMs === undefined
      ? Number.POSITIVE_INFINITY
      : claimedAtMs + options.holdForMs,
  };
}

describe('the order of viewport owners', () => {
  it('puts the reader first and the corrections last', () => {
    // The order is the whole design, so it is asserted rather than assumed.
    expect(FLOWCHAT_VIEWPORT_OWNERS).toEqual([
      'user-gesture',
      'one-shot-navigation',
      'follow-output',
      'layout-correction',
    ]);
  });

  it('does not rank a displacement repair, which is a different question', () => {
    // Putting the reader back where a prepend or a late measurement moved them
    // is `canShiftViewport`, and its answer for the reader's own gesture is the
    // opposite of what any ranking here could give it.
    expect(FLOWCHAT_VIEWPORT_OWNERS).not.toContain('anchor-correction');
  });

  it('does not rank the opening reveal, which is a phase and not a writer', () => {
    // Follow-output is what moves the viewport while the transcript is hidden.
    // A claim standing in for the reveal would outrank it and refuse it.
    expect(FLOWCHAT_VIEWPORT_OWNERS).not.toContain('opening-reveal');
  });
});

describe('claimViewport', () => {
  it('grants an unheld viewport to anyone', () => {
    const outcome = claimViewport(null, { owner: 'layout-correction', nowMs: NOW });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('layout-correction');
  });

  it('lets a gesture take the viewport from the follow loop', () => {
    const outcome = claimViewport(heldBy('follow-output'), {
      owner: 'user-gesture',
      nowMs: NOW,
    });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('user-gesture');
  });

  it('leaves the register untouched when it refuses', () => {
    const held = heldBy('one-shot-navigation', { holdForMs: 600 });
    const outcome = claimViewport(held, { owner: 'follow-output', nowMs: NOW });
    expect(outcome.granted).toBe(false);
    expect(outcome.claim).toEqual(held);
  });

  it('lets an owner renew its own claim', () => {
    // How a continuous writer holds on, and how a correction runs on
    // consecutive frames.
    const outcome = claimViewport(heldBy('follow-output'), {
      owner: 'follow-output',
      nowMs: NOW + 16,
    });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.claimedAtMs).toBe(NOW + 16);
  });

  it('grants a lapsed viewport to a lower priority', () => {
    const gesture = heldBy('user-gesture', { holdForMs: 200 });
    const outcome = claimViewport(gesture, {
      owner: 'layout-correction',
      nowMs: NOW + 201,
    });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('layout-correction');
  });

  it('holds a claim without an expiry until it is released', () => {
    const outcome = claimViewport(null, { owner: 'follow-output', nowMs: NOW });
    expect(outcome.claim?.expiresAtMs).toBe(Number.POSITIVE_INFINITY);
    expect(canOwnViewport(outcome.claim, 'layout-correction', NOW + 1_000_000)).toBe(false);
  });

  it('only lets an owner that releases hold the viewport indefinitely', () => {
    // Follow-output is the one writer with an `exitFollowOutput`. Everything
    // else that said nothing about a window would hold the viewport for the
    // rest of the session, and nothing below it could ever write again.
    for (const owner of FLOWCHAT_VIEWPORT_OWNERS) {
      expect(defaultViewportHoldMs(owner)).toBe(
        owner === 'follow-output' ? Number.POSITIVE_INFINITY : 0,
      );
    }
  });

  it('lapses a one-shot claim that named no window, rather than starving follow', () => {
    /*
     * Measured on session open: the virtualizer placed the scroller at 0 before
     * any aim of ours, the write took an unbounded claim, and follow-output was
     * refused for the whole opening reveal — the transcript stayed at the top
     * of its loaded window instead of at the newest Turn.
     */
    const stray = claimViewport(null, { owner: 'one-shot-navigation', nowMs: NOW });
    expect(stray.granted).toBe(true);

    const follow = claimViewport(stray.claim, { owner: 'follow-output', nowMs: NOW });
    expect(follow.granted).toBe(true);
    expect(follow.claim?.owner).toBe('follow-output');
  });

  it('still resolves a correction against whoever holds the viewport', () => {
    // A zero-length hold is not the absence of a claim: the question it answers
    // is whether the write happens at all.
    const outcome = claimViewport(null, {
      owner: 'layout-correction',
      nowMs: NOW + 16,
    });
    expect(outcome.granted).toBe(true);
  });
});

describe('canShiftViewport', () => {
  it('lets a displacement through while the reader holds the viewport', () => {
    /*
     * The failure this separates out: history pages in only while the reader
     * scrolls up into it, so a gesture refusing the compensation refused all of
     * them. 2494px of history arrived, `scrollTop` stayed at 40, and the
     * transcript jumped back thirteen Turns.
     */
    expect(canShiftViewport(heldBy('user-gesture', { holdForMs: 200 }), NOW)).toBe(true);
  });

  it('leaves the displacement to navigation and follow targets', () => {
    // Those writers re-assert a position of their own. Snap-back is different:
    // a late history measurement invalidates its target coordinate, so the
    // displacement must cancel the stale animation and be reconsidered.
    expect(canShiftViewport(heldBy('follow-output'), NOW)).toBe(false);
    expect(canShiftViewport(heldBy('one-shot-navigation', { holdForMs: 600 }), NOW)).toBe(false);
    expect(canShiftViewport(null, NOW)).toBe(true);
  });

  it('shifts an unheld viewport, and one held only by a correction', () => {
    expect(canShiftViewport(null, NOW)).toBe(true);
    expect(canShiftViewport(heldBy('layout-correction', { holdForMs: 0 }), NOW)).toBe(true);
  });

  it('shifts once a target holder has lapsed', () => {
    expect(canShiftViewport(heldBy('one-shot-navigation', { holdForMs: 600 }), NOW + 601))
      .toBe(true);
  });
});

describe('activeViewportClaim', () => {
  it('reports a claim that has not lapsed', () => {
    const claim = heldBy('user-gesture', { holdForMs: 200 });
    expect(activeViewportClaim(claim, NOW + 199)).toBe(claim);
  });

  it('reports nothing once the hold has elapsed', () => {
    // A missed release costs a bounded wait, never a viewport nobody may write.
    expect(activeViewportClaim(heldBy('user-gesture', { holdForMs: 200 }), NOW + 200)).toBeNull();
  });
});

describe('releaseViewport', () => {
  it('gives the viewport back', () => {
    expect(releaseViewport(heldBy('follow-output'), 'follow-output')).toBeNull();
  });

  it('ignores a release from an owner that was preempted', () => {
    // Finishing late must not clear the claim of whoever took over: that
    // release is indistinguishable from the new owner having finished, and it
    // would hand the viewport to the next corrector mid-movement.
    const held = heldBy('user-gesture');
    expect(releaseViewport(held, 'follow-output')).toEqual(held);
  });
});
