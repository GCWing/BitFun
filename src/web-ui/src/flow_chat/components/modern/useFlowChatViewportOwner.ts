/**
 * The register, wired to a live scroller.
 *
 * `flowChatViewportOwnership.ts` decides who may write; this holds the current
 * claim and performs the write. The two are one call on purpose: taking
 * ownership and moving the viewport being the same action is what keeps a new
 * writer from being added without the rest of FlowChat knowing about it.
 *
 * Everything here is refs. A claim changes many times per frame — the follow
 * loop renews on every one — and none of it belongs in a render.
 */

import { useCallback, useMemo, useRef, type RefObject } from 'react';
import {
  isViewportDiagnosticsEnabled,
  roundViewportPx,
  traceViewportRepeating,
} from '@/infrastructure/diagnostics/flowChatViewportDiagnostics';
import {
  activeViewportClaim,
  canOwnViewport,
  claimViewport,
  releaseViewport,
  type FlowChatViewportOwner,
  type ViewportClaim,
} from './flowChatViewportOwnership';

export interface ViewportWriteRequest {
  owner: FlowChatViewportOwner;
  /** Offset in scroller coordinates. */
  topPx: number;
  behavior?: ScrollBehavior;
  /**
   * How long ownership stands after this write.
   *
   * Omitted holds it for the frame the write happens in, which is what a
   * correction wants. An animated write has to outlast its own animation, or
   * the first frame of travel is read as a displacement by whatever corrects
   * next — see the snap back in `FLOWCHAT_SCROLL_STABILITY.md`.
   */
  holdForMs?: number;
}

export interface FlowChatViewportOwnerApi {
  /**
   * Take the viewport without writing it yet, for a writer whose movement is
   * issued by something else — a virtualizer aiming at an item, or a browser
   * animation. Returns whether the claim was granted.
   */
  claim: (owner: FlowChatViewportOwner, options?: { holdForMs?: number }) => boolean;
  /** Give it back. A writer that was preempted cannot release the new owner. */
  release: (owner: FlowChatViewportOwner) => void;
  /** Whether something more authoritative is moving the viewport right now. */
  isHeldByOther: (owner: FlowChatViewportOwner) => boolean;
  /** The owner holding the viewport, for diagnostics. Null when it is free. */
  currentOwner: () => FlowChatViewportOwner | null;
  /**
   * Move the viewport, if the register allows it. False means the write did
   * not happen — the caller has been outranked and must not fall back to
   * writing the scroller itself.
   */
  write: (request: ViewportWriteRequest) => boolean;
}

export function useFlowChatViewportOwner(
  scrollerRef: RefObject<HTMLElement | null>,
): FlowChatViewportOwnerApi {
  const claimRef = useRef<ViewportClaim | null>(null);

  /**
   * Take the viewport and report who was holding it, which the register drops
   * once the outcome is known. Both public entry points go through this so a
   * write is one line in the log rather than a claim and a write.
   */
  const takeViewport = useCallback((
    owner: FlowChatViewportOwner,
    holdForMs: number | undefined,
  ): { granted: boolean; heldBy: FlowChatViewportOwner | null } => {
    const nowMs = performance.now();
    // Read before the claim resolves: on a refusal this is who refused, and on
    // a grant it is who was preempted, which is the same question either way.
    const heldBy = activeViewportClaim(claimRef.current, nowMs)?.owner ?? null;
    const outcome = claimViewport(claimRef.current, { owner, nowMs, holdForMs });
    claimRef.current = outcome.claim;
    return { granted: outcome.granted, heldBy };
  }, []);

  const claim = useCallback((
    owner: FlowChatViewportOwner,
    options?: { holdForMs?: number },
  ): boolean => {
    const { granted, heldBy } = takeViewport(owner, options?.holdForMs);
    if (isViewportDiagnosticsEnabled()) {
      traceViewportRepeating(`claim|${owner}|${granted}|${heldBy}`, {
        location: 'viewportOwner.claim',
        message: granted ? 'took the viewport' : 'refused the viewport',
        data: () => ({ owner, granted, heldBy, holdForMs: options?.holdForMs ?? null }),
      });
    }
    return granted;
  }, [takeViewport]);

  const release = useCallback((owner: FlowChatViewportOwner) => {
    const heldBy = claimRef.current?.owner ?? null;
    claimRef.current = releaseViewport(claimRef.current, owner);
    if (isViewportDiagnosticsEnabled() && heldBy !== null) {
      traceViewportRepeating(`release|${owner}|${heldBy}`, {
        location: 'viewportOwner.release',
        message: heldBy === owner
          ? 'gave the viewport back'
          : 'released nothing, having already been preempted',
        data: () => ({ owner, heldBy }),
      });
    }
  }, []);

  const isHeldByOther = useCallback((owner: FlowChatViewportOwner) => (
    !canOwnViewport(claimRef.current, owner, performance.now())
  ), []);

  const currentOwner = useCallback(() => {
    const nowMs = performance.now();
    const held = claimRef.current;
    return held && nowMs < held.expiresAtMs ? held.owner : null;
  }, []);

  const write = useCallback((request: ViewportWriteRequest): boolean => {
    const scroller = scrollerRef.current;
    if (!scroller) return false;
    const { granted, heldBy } = takeViewport(request.owner, request.holdForMs);
    /*
     * The one place that sees every deliberate movement, and the only place a
     * refusal is visible at all — a writer that was outranked leaves no trace
     * anywhere else, and "nothing moved" is the more common complaint.
     */
    if (isViewportDiagnosticsEnabled()) {
      const fromPx = scroller.scrollTop;
      traceViewportRepeating(`write|${request.owner}|${granted}|${heldBy}`, {
        location: 'viewportOwner.write',
        message: granted ? 'moved the viewport' : 'was refused the viewport',
        travelPx: request.topPx - fromPx,
        data: () => ({
          owner: request.owner,
          granted,
          heldBy,
          fromPx: roundViewportPx(fromPx),
          toPx: roundViewportPx(request.topPx),
          behavior: request.behavior ?? 'auto',
          holdForMs: request.holdForMs ?? null,
        }),
      });
    }
    if (!granted) return false;
    if (request.behavior === 'smooth') {
      scroller.scrollTo({ top: request.topPx, behavior: 'smooth' });
    } else {
      // Assigned rather than `scrollTo({behavior:'auto'})`: an assignment also
      // cancels any animation still running, which is what a writer taking the
      // viewport from an animated one means to do.
      scroller.scrollTop = request.topPx;
    }
    return true;
  }, [scrollerRef, takeViewport]);

  return useMemo(() => ({
    claim,
    release,
    isHeldByOther,
    currentOwner,
    write,
  }), [claim, currentOwner, isHeldByOther, release, write]);
}
