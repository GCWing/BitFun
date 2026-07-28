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
  expiresAtMs: number | null;
};

const ELEMENT_ANCHOR_TTL_MS = 1000;
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

function nowMs(): number {
  return typeof performance === 'undefined' ? Date.now() : performance.now();
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
    this.expireElementAnchor();
    return this.mode;
  }

  ownsElementAnchor(): boolean {
    this.expireElementAnchor();
    return Boolean(
      this.elementAnchor &&
      (this.mode === 'pinned-item' || this.mode === 'preserving-element'),
    );
  }

  pinItem(_reason = 'unspecified'): void {
    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'pinned-item';
  }

  pinElement(element: HTMLElement | null | undefined): boolean {
    return this.captureElement(element, 'pinned-item', null);
  }

  followTail(options?: { force?: boolean }): boolean {
    this.expireElementAnchor();
    if (this.mode === 'preserving-element' && !options?.force) {
      return false;
    }

    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'following-tail';
    return true;
  }

  preserveElement(element: HTMLElement | null | undefined): boolean {
    this.expireElementAnchor();
    if (!element || this.mode === 'following-tail' || this.mode === 'pinned-item') {
      return false;
    }

    return this.captureElement(
      element,
      'preserving-element',
      nowMs() + ELEMENT_ANCHOR_TTL_MS,
    );
  }

  private captureElement(
    element: HTMLElement | null | undefined,
    mode: 'pinned-item' | 'preserving-element',
    expiresAtMs: number | null,
  ): boolean {
    if (!element) {
      return false;
    }

    const scroller = element.closest<HTMLElement>('[data-virtuoso-scroller="true"]');
    if (!scroller) {
      return false;
    }

    const elementRect = element.getBoundingClientRect();
    const scrollerRect = scroller.getBoundingClientRect();
    this.elementAnchor = {
      element,
      scroller,
      offsetFromScrollerTop: elementRect.top - scrollerRect.top,
      expiresAtMs,
    };
    this.mode = mode;
    this.startAnchorGuard();
    return true;
  }

  restoreElementAnchor(scroller: HTMLElement, source = 'external'): boolean {
    this.expireElementAnchor();
    const anchor = this.elementAnchor;
    if (!anchor || (this.mode !== 'preserving-element' && this.mode !== 'pinned-item')) {
      return false;
    }
    if (!anchor.element.isConnected) {
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

    applyCorrection(initialCorrection);
    let remainingCorrection = readCorrection();

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

  release(_reason = 'unspecified'): void {
    this.stopAnchorGuard();
    this.elementAnchor = null;
    this.mode = 'idle';
  }

  private expireElementAnchor(): void {
    if (
      this.elementAnchor?.expiresAtMs !== null &&
      this.elementAnchor?.expiresAtMs !== undefined &&
      this.elementAnchor.expiresAtMs < nowMs()
    ) {
      this.elementAnchor = null;
      this.stopAnchorGuard();
      if (this.mode === 'preserving-element') {
        this.mode = 'idle';
      }
    }
  }

  private startAnchorGuard(): void {
    if (this.anchorGuardFrame !== null || typeof requestAnimationFrame === 'undefined') {
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
    this.expireElementAnchor();
    const anchor = this.elementAnchor;
    if (
      !anchor ||
      (this.mode !== 'pinned-item' && this.mode !== 'preserving-element') ||
      !anchor.scroller.isConnected
    ) {
      return;
    }

    this.restoreElementAnchor(anchor.scroller, 'anchor-guard');
    this.startAnchorGuard();
  };
}
