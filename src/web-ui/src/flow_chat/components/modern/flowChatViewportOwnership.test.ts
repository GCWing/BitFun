import { describe, expect, it } from 'vitest';
import {
  activeViewportClaim,
  canOwnViewport,
  claimViewport,
  FLOWCHAT_VIEWPORT_OWNERS,
  releaseViewport,
  type ViewportClaim,
} from './flowChatViewportOwnership';

const NOW = 1_000;

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
      'snap-back',
      'follow-output',
      'layout-correction',
      'anchor-correction',
    ]);
  });

  it('does not rank the opening reveal, which is a phase and not a writer', () => {
    // Follow-output is what moves the viewport while the transcript is hidden.
    // A claim standing in for the reveal would outrank it and refuse it.
    expect(FLOWCHAT_VIEWPORT_OWNERS).not.toContain('opening-reveal');
  });
});

describe('claimViewport', () => {
  it('grants an unheld viewport to anyone', () => {
    const outcome = claimViewport(null, { owner: 'anchor-correction', nowMs: NOW });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('anchor-correction');
  });

  it('lets a gesture take the viewport from the follow loop', () => {
    const outcome = claimViewport(heldBy('follow-output'), {
      owner: 'user-gesture',
      nowMs: NOW,
    });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('user-gesture');
  });

  it('refuses the anchor while a snap back is animating', () => {
    // The failure this register exists for: the anchor read the snap's own
    // travel as a displacement, and the write cancelled the animation.
    const outcome = claimViewport(heldBy('snap-back'), {
      owner: 'anchor-correction',
      nowMs: NOW,
    });
    expect(outcome.granted).toBe(false);
    expect(outcome.claim?.owner).toBe('snap-back');
  });

  it('leaves the register untouched when it refuses', () => {
    const held = heldBy('one-shot-navigation');
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
      owner: 'anchor-correction',
      nowMs: NOW + 201,
    });
    expect(outcome.granted).toBe(true);
    expect(outcome.claim?.owner).toBe('anchor-correction');
  });

  it('holds a claim without an expiry until it is released', () => {
    const outcome = claimViewport(null, { owner: 'follow-output', nowMs: NOW });
    expect(outcome.claim?.expiresAtMs).toBe(Number.POSITIVE_INFINITY);
    expect(canOwnViewport(outcome.claim, 'anchor-correction', NOW + 1_000_000)).toBe(false);
  });
});

describe('activeViewportClaim', () => {
  it('reports a claim that has not lapsed', () => {
    const claim = heldBy('user-gesture', { holdForMs: 200 });
    expect(activeViewportClaim(claim, NOW + 199)).toBe(claim);
  });

  it('reports nothing once the hold has elapsed', () => {
    // A missed release costs a bounded wait, never a viewport nobody may write.
    expect(activeViewportClaim(heldBy('snap-back', { holdForMs: 200 }), NOW + 200)).toBeNull();
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
