import { flowChatDiagnostics } from '@/infrastructure/diagnostics/flowChatDiagnostics';

export type FlowChatViewportAnchorMode =
  | 'idle'
  | 'pinned-item'
  | 'following-tail'
  | 'preserving-element';

export interface FlowChatViewportRangeHost {
  ensureBottomRange(options: {
    additionalPx: number;
    mode: Extract<FlowChatViewportAnchorMode, 'pinned-item' | 'preserving-element'>;
    source: string;
  }): boolean;
}

type ElementAnchor = {
  element: HTMLElement;
  scroller: HTMLElement;
  offsetFromScrollerTop: number;
  preservationPhase: 'active' | 'retained' | null;
};

const ELEMENT_ANCHOR_EPSILON_PX = 0.5;
const ELEMENT_ANCHOR_RANGE_GUARD_PX = 1;

export function canHandoffPinnedItemToTail(options: {
  pinReservationPx: number;
  collapseReservationPx: number;
  hasPendingCollapseIntent: boolean;
}): boolean {
  return (
    options.pinReservationPx <= ELEMENT_ANCHOR_EPSILON_PX &&
    options.collapseReservationPx <= ELEMENT_ANCHOR_EPSILON_PX &&
    !options.hasPendingCollapseIntent
  );
}

/** Owns anchor priority independently from the virtualizer implementation. */
export class FlowChatViewportCoordinator {
  private mode: FlowChatViewportAnchorMode = 'idle';
  private elementAnchor: ElementAnchor | null = null;
  private anchorGuardFrame: number | null = null;
  private rangeHost: FlowChatViewportRangeHost | null = null;

  setRangeHost(host: FlowChatViewportRangeHost | null): void {
    this.rangeHost = host;
  }

  getMode(): FlowChatViewportAnchorMode {
    this.validateElementAnchor('get-mode');
    return this.mode;
  }

  ownsElementAnchor(): boolean {
    this.validateElementAnchor('owns-element-anchor');
    return Boolean(
      this.elementAnchor &&
      (this.mode === 'pinned-item' || this.mode === 'preserving-element'),
    );
  }

  pinItem(reason = 'unspecified'): void {
    const previousMode = this.mode;
    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'pinned-item';
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'B',
        location: 'FlowChatViewportCoordinator.pinItem',
        message: 'Viewport coordinator entered pinned item mode',
        data: () => ({ previousMode, reason }),
      });
    }
  }

  pinElement(element: HTMLElement | null | undefined): boolean {
    return this.captureElement(element, 'pinned-item');
  }

  followTail(options?: { force?: boolean }): boolean {
    this.validateElementAnchor('follow-tail');
    const hasActiveElementPreservation = (
      this.mode === 'preserving-element' &&
      this.elementAnchor?.preservationPhase === 'active'
    );
    if (hasActiveElementPreservation && !options?.force) {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'B',
          location: 'FlowChatViewportCoordinator.followTail',
          message: 'Tail follow rejected during active element preservation',
          data: () => ({
            mode: this.mode,
            preservationPhase: this.elementAnchor?.preservationPhase ?? null,
            force: options?.force === true,
          }),
        });
      }
      return false;
    }

    const previousMode = this.mode;
    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'following-tail';
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'B',
        location: 'FlowChatViewportCoordinator.followTail',
        message: 'Viewport coordinator entered tail follow mode',
        data: () => ({ previousMode, force: options?.force === true }),
      });
    }
    return true;
  }

  preserveElement(element: HTMLElement | null | undefined): boolean {
    this.validateElementAnchor('preserve-element');
    if (!element || this.mode === 'following-tail' || this.mode === 'pinned-item') {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'E',
          location: 'FlowChatViewportCoordinator.preserveElement',
          message: 'Element preservation request rejected',
          data: () => ({ hasElement: Boolean(element), mode: this.mode }),
        });
      }
      return false;
    }

    return this.captureElement(
      element,
      'preserving-element',
    );
  }

  settleElementPreservation(source = 'unspecified'): boolean {
    this.validateElementAnchor('settle-element-preservation');
    const anchor = this.elementAnchor;
    if (!anchor || this.mode !== 'preserving-element') {
      return false;
    }

    const previousPhase = anchor.preservationPhase;
    anchor.preservationPhase = 'retained';
    this.stopAnchorGuard();
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'E',
        location: 'FlowChatViewportCoordinator.settleElementPreservation',
        message: 'Element preservation retained after layout settlement',
        data: () => ({ previousPhase, source }),
      });
    }
    return true;
  }

  private captureElement(
    element: HTMLElement | null | undefined,
    mode: 'pinned-item' | 'preserving-element',
  ): boolean {
    if (!element) {
      return false;
    }

    const scroller = element.closest<HTMLElement>('[data-virtuoso-scroller="true"]');
    if (!scroller) {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'B',
          location: 'FlowChatViewportCoordinator.captureElement',
          message: 'Element anchor capture failed without a scroller',
          data: () => ({ mode }),
        });
      }
      return false;
    }

    const elementRect = element.getBoundingClientRect();
    const scrollerRect = scroller.getBoundingClientRect();
    this.elementAnchor = {
      element,
      scroller,
      offsetFromScrollerTop: elementRect.top - scrollerRect.top,
      preservationPhase: mode === 'preserving-element' ? 'active' : null,
    };
    this.mode = mode;
    this.startAnchorGuard();
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: mode === 'preserving-element' ? 'E' : 'B',
        location: 'FlowChatViewportCoordinator.captureElement',
        message: 'Semantic element anchor captured',
        data: () => ({
          mode,
          preservationPhase: this.elementAnchor?.preservationPhase ?? null,
          elementConnected: element.isConnected,
          offsetFromScrollerTop: this.elementAnchor?.offsetFromScrollerTop ?? null,
          scrollTop: scroller.scrollTop,
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
        }),
      });
    }
    return true;
  }

  restoreElementAnchor(scroller: HTMLElement, source = 'external'): boolean {
    this.validateElementAnchor(`restore:${source}`);
    const anchor = this.elementAnchor;
    if (!anchor || (this.mode !== 'preserving-element' && this.mode !== 'pinned-item')) {
      return false;
    }

    const readCorrection = () => {
      const elementRect = anchor.element.getBoundingClientRect();
      const scrollerRect = scroller.getBoundingClientRect();
      return elementRect.top - scrollerRect.top - anchor.offsetFromScrollerTop;
    };
    const applyCorrection = (correction: number) => {
      const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      const desiredScrollTop = scroller.scrollTop + correction;
      const requestedScrollTop = Math.min(maxScrollTop, Math.max(0, desiredScrollTop));
      scroller.scrollTop = requestedScrollTop;
    };

    const initialCorrection = readCorrection();
    if (Math.abs(initialCorrection) <= ELEMENT_ANCHOR_EPSILON_PX) {
      return false;
    }

    const diagnosticsEnabled = flowChatDiagnostics.isEnabled();
    const scrollTopBefore = diagnosticsEnabled ? scroller.scrollTop : null;
    applyCorrection(initialCorrection);
    let remainingCorrection = readCorrection();
    if (diagnosticsEnabled) {
      flowChatDiagnostics.trace({
        hypothesis: 'B',
        location: 'FlowChatViewportCoordinator.restoreElementAnchor',
        message: 'Semantic anchor correction applied',
        data: () => ({
          mode: this.mode,
          source,
          initialCorrection,
          remainingCorrection,
          scrollTopBefore,
          scrollTopAfter: scroller.scrollTop,
          maxScrollTop: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
        }),
      });
    }

    if (
      remainingCorrection > ELEMENT_ANCHOR_EPSILON_PX &&
      this.rangeHost &&
      (this.mode === 'pinned-item' || this.mode === 'preserving-element')
    ) {
      const rangeExtended = this.rangeHost.ensureBottomRange({
        additionalPx: remainingCorrection + ELEMENT_ANCHOR_RANGE_GUARD_PX,
        mode: this.mode,
        source,
      });
      if (rangeExtended) {
        void scroller.scrollHeight;
        remainingCorrection = readCorrection();
        if (Math.abs(remainingCorrection) > ELEMENT_ANCHOR_EPSILON_PX) {
          applyCorrection(remainingCorrection);
        }
      }
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'C',
          location: 'FlowChatViewportCoordinator.restoreElementAnchor',
          message: 'Semantic anchor requested additional bottom range',
          data: () => ({
            mode: this.mode,
            source,
            rangeExtended,
            remainingCorrection,
            scrollTop: scroller.scrollTop,
            scrollHeight: scroller.scrollHeight,
            clientHeight: scroller.clientHeight,
          }),
        });
      }
    }
    return true;
  }

  restoreScrollPositionOnce(
    scroller: HTMLElement,
    targetScrollTop: number,
    source = 'unspecified',
  ): boolean {
    if (this.ownsElementAnchor()) {
      this.restoreElementAnchor(scroller, source);
      return true;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const previousScrollTop = scroller.scrollTop;
    const nextScrollTop = Math.min(maxScrollTop, Math.max(0, targetScrollTop));
    if (Math.abs(nextScrollTop - previousScrollTop) <= ELEMENT_ANCHOR_EPSILON_PX) {
      return false;
    }

    scroller.scrollTop = nextScrollTop;
    return true;
  }

  release(reason = 'unspecified'): void {
    const previousMode = this.mode;
    const hadElementAnchor = Boolean(this.elementAnchor);
    const previousPreservationPhase = this.elementAnchor?.preservationPhase ?? null;
    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'idle';
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'B',
        location: 'FlowChatViewportCoordinator.release',
        message: 'Viewport coordinator released semantic ownership',
        data: () => ({ previousMode, previousPreservationPhase, hadElementAnchor, reason }),
      });
    }
  }

  private validateElementAnchor(source: string): void {
    const anchor = this.elementAnchor;
    if (anchor && (!anchor.element.isConnected || !anchor.scroller.isConnected)) {
      this.release(`element-anchor-disconnected:${source}`);
    }
  }

  private startAnchorGuard(): void {
    if (
      this.anchorGuardFrame !== null ||
      typeof requestAnimationFrame === 'undefined' ||
      (
        this.mode === 'preserving-element' &&
        this.elementAnchor?.preservationPhase === 'retained'
      )
    ) {
      return;
    }
    this.anchorGuardFrame = requestAnimationFrame(this.runAnchorGuardFrame);
  }

  private stopAnchorGuard(): void {
    if (this.anchorGuardFrame === null || typeof cancelAnimationFrame === 'undefined') {
      this.anchorGuardFrame = null;
      return;
    }
    cancelAnimationFrame(this.anchorGuardFrame);
    this.anchorGuardFrame = null;
  }

  private runAnchorGuardFrame = (): void => {
    this.anchorGuardFrame = null;
    this.validateElementAnchor('anchor-guard');
    const anchor = this.elementAnchor;
    if (
      !anchor ||
      (this.mode !== 'pinned-item' && this.mode !== 'preserving-element') ||
      (
        this.mode === 'preserving-element' &&
        anchor.preservationPhase === 'retained'
      )
    ) {
      return;
    }

    this.restoreElementAnchor(anchor.scroller, 'anchor-guard');
    this.startAnchorGuard();
  };
}
