// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  canHandoffPinnedItemToTail,
  FlowChatViewportCoordinator,
} from './FlowChatViewportCoordinator';

function setRect(element: HTMLElement, top: number): void {
  Object.defineProperty(element, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({
      bottom: top + 40,
      height: 40,
      left: 0,
      right: 300,
      top,
      width: 300,
      x: 0,
      y: top,
      toJSON: () => ({}),
    }),
  });
}

function setScrollerGeometry(scroller: HTMLElement, scrollTop: number): void {
  Object.defineProperties(scroller, {
    clientHeight: { configurable: true, value: 500 },
    scrollHeight: { configurable: true, value: 2000 },
    scrollTop: { configurable: true, writable: true, value: scrollTop },
  });
}

afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe('FlowChatViewportCoordinator', () => {
  it('hands off after the pin drains or natural content reaches the viewport tail', () => {
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 1029,
      collapseReservationPx: 0,
      pendingStickyPinGrowthPx: 0,
      hasPendingCollapseIntent: false,
      viewport: {
        scrollTop: 900,
        clientHeight: 1000,
        naturalContentHeight: 1899,
      },
    })).toBe(false);
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 1029,
      collapseReservationPx: 0,
      pendingStickyPinGrowthPx: 0,
      hasPendingCollapseIntent: false,
      viewport: {
        scrollTop: 900,
        clientHeight: 1000,
        naturalContentHeight: 1900,
      },
    })).toBe(true);
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 1029,
      collapseReservationPx: 200,
      pendingStickyPinGrowthPx: 0,
      hasPendingCollapseIntent: false,
      viewport: {
        scrollTop: 900,
        clientHeight: 1000,
        naturalContentHeight: 2000,
      },
    })).toBe(false);
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 1029,
      collapseReservationPx: 0,
      pendingStickyPinGrowthPx: 20,
      hasPendingCollapseIntent: false,
      viewport: {
        scrollTop: 900,
        clientHeight: 1000,
        naturalContentHeight: 2000,
      },
    })).toBe(false);
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 1029,
      collapseReservationPx: 0,
      pendingStickyPinGrowthPx: 0,
      hasPendingCollapseIntent: true,
      viewport: {
        scrollTop: 900,
        clientHeight: 1000,
        naturalContentHeight: 2000,
      },
    })).toBe(false);
    expect(canHandoffPinnedItemToTail({
      pinReservationPx: 0,
      collapseReservationPx: 0,
      pendingStickyPinGrowthPx: 0,
      hasPendingCollapseIntent: false,
      viewport: null,
    })).toBe(true);
  });

  it('restores a collapsing card header to its captured viewport offset', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.preserveElement(card)).toBe(true);

    setRect(card, 80);
    expect(coordinator.restoreElementAnchor(scroller)).toBe(true);
    expect(scroller.scrollTop).toBe(860);
  });

  it('does not let automatic tail follow replace a preserved card anchor', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.preserveElement(card);

    expect(coordinator.followTail()).toBe(false);
    expect(coordinator.getMode()).toBe('preserving-element');
    expect(coordinator.followTail({ force: true })).toBe(true);
    expect(coordinator.getMode()).toBe('following-tail');
  });

  it('retains a settled card anchor without a wall-clock expiry', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const now = vi.spyOn(performance, 'now').mockReturnValue(1_000);
    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.preserveElement(card)).toBe(true);
    expect(coordinator.settleElementPreservation('test-settled')).toBe(true);

    now.mockReturnValue(60_000);
    expect(coordinator.ownsElementAnchor()).toBe(true);
    expect(coordinator.getMode()).toBe('preserving-element');

    setRect(card, 80);
    expect(coordinator.restoreElementAnchor(scroller, 'test-delayed-layout')).toBe(true);
    expect(scroller.scrollTop).toBe(860);
  });

  it('allows automatic tail follow to take ownership from a retained anchor', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.preserveElement(card);
    coordinator.settleElementPreservation('test-settled');

    expect(coordinator.followTail()).toBe(true);
    expect(coordinator.getMode()).toBe('following-tail');
    expect(coordinator.ownsElementAnchor()).toBe(false);
  });

  it('releases a retained anchor when its DOM element disconnects', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.preserveElement(card);
    coordinator.settleElementPreservation('test-settled');
    card.remove();

    expect(coordinator.ownsElementAnchor()).toBe(false);
    expect(coordinator.getMode()).toBe('idle');
  });

  it('keeps a pinned item anchored until follow mode takes ownership', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const item = document.createElement('div');
    scroller.append(item);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(item, 57);

    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.pinElement(item)).toBe(true);

    setRect(item, 87);
    expect(coordinator.restoreElementAnchor(scroller)).toBe(true);
    expect(scroller.scrollTop).toBe(730);
    expect(coordinator.getMode()).toBe('pinned-item');

    coordinator.followTail({ force: true });
    setRect(item, 117);
    expect(coordinator.restoreElementAnchor(scroller)).toBe(false);
  });

  it('does not let a tool-card collapse replace an active pinned-item anchor', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    const toolCard = document.createElement('div');
    scroller.append(pinnedItem, toolCard);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);
    setRect(toolCard, 300);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.pinElement(pinnedItem);

    expect(coordinator.preserveElement(toolCard)).toBe(false);
    expect(coordinator.getMode()).toBe('pinned-item');
  });

  it('owns virtualizer scroll compensation while an element anchor is active', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);

    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.ownsElementAnchor()).toBe(false);
    coordinator.pinElement(pinnedItem);
    expect(coordinator.ownsElementAnchor()).toBe(true);
    coordinator.followTail({ force: true });
    expect(coordinator.ownsElementAnchor()).toBe(false);
  });

  it('restores an idle viewport position once without creating a persistent lock', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);

    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.restoreScrollPositionOnce(scroller, 1200, 'test-idle')).toBe(true);
    expect(scroller.scrollTop).toBe(1200);

    scroller.scrollTop = 900;
    expect(coordinator.getMode()).toBe('idle');
    expect(scroller.scrollTop).toBe(900);
  });

  it('clamps the idle fallback restore to the current physical scroll range', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);

    const coordinator = new FlowChatViewportCoordinator();
    expect(coordinator.restoreScrollPositionOnce(scroller, 5000, 'test-clamp')).toBe(true);
    expect(scroller.scrollTop).toBe(1500);
  });

  it('delegates one-shot restoration to the semantic anchor when it owns the viewport', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.pinElement(pinnedItem);
    setRect(pinnedItem, 87);

    expect(coordinator.restoreScrollPositionOnce(scroller, 0, 'test-semantic')).toBe(true);
    expect(scroller.scrollTop).toBe(730);
  });

  it('extends the physical bottom range before retrying a stalled semantic restore', () => {
    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);

    let scrollHeight = 1200;
    const clientHeight = 500;
    let scrollTop = 700;
    let itemTop = 57;
    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, get: () => clientHeight },
      scrollHeight: { configurable: true, get: () => scrollHeight },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (requested: number) => {
          const maxScrollTop = Math.max(0, scrollHeight - clientHeight);
          const applied = Math.min(maxScrollTop, Math.max(0, requested));
          itemTop -= applied - scrollTop;
          scrollTop = applied;
        },
      },
    });
    setRect(scroller, 0);
    vi.spyOn(pinnedItem, 'getBoundingClientRect').mockImplementation(() => ({
      bottom: itemTop + 40,
      height: 40,
      left: 0,
      right: 300,
      top: itemTop,
      width: 300,
      x: 0,
      y: itemTop,
      toJSON: () => ({}),
    }));

    const ensureBottomRange = vi.fn(({ additionalPx }: { additionalPx: number }) => {
      scrollHeight += additionalPx;
      return true;
    });
    const coordinator = new FlowChatViewportCoordinator();
    coordinator.setRangeHost({ ensureBottomRange });
    coordinator.pinElement(pinnedItem);

    scrollHeight = 1150;
    scroller.scrollTop = 700;
    expect(scrollTop).toBe(650);
    expect(itemTop).toBe(107);

    expect(coordinator.restoreElementAnchor(scroller, 'test-range-recovery')).toBe(true);
    expect(ensureBottomRange).toHaveBeenCalledWith(expect.objectContaining({
      additionalPx: 51,
      mode: 'pinned-item',
      source: 'test-range-recovery',
    }));
    expect(scrollTop).toBe(700);
    expect(itemTop).toBe(57);
    coordinator.release('test-cleanup');
  });

  it('reconciles a pinned anchor from the internal animation-frame guard', () => {
    let scheduledFrame: FrameRequestCallback | null = null;
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      scheduledFrame = callback;
      return 1;
    });
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});

    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.pinElement(pinnedItem);
    setRect(pinnedItem, 157);

    expect(scheduledFrame).not.toBeNull();
    (scheduledFrame as FrameRequestCallback)(0);
    expect(scroller.scrollTop).toBe(800);
    coordinator.release('test-cleanup');
  });

  it('coalesces scheduled anchor restores into the existing frame using the latest request', () => {
    let scheduledFrame: FrameRequestCallback | null = null;
    const requestFrame = vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      scheduledFrame = callback;
      return 1;
    });
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});

    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.pinElement(pinnedItem);
    const restoreAnchor = vi.spyOn(coordinator, 'restoreElementAnchor');
    setRect(pinnedItem, 87);

    expect(coordinator.scheduleElementAnchorRestore(scroller, 'scroll-handler:first')).toBe(true);
    expect(coordinator.scheduleElementAnchorRestore(scroller, 'scroll-handler:latest')).toBe(true);
    expect(requestFrame).toHaveBeenCalledTimes(1);

    expect(scheduledFrame).not.toBeNull();
    (scheduledFrame as FrameRequestCallback)(0);
    expect(restoreAnchor).toHaveBeenCalledTimes(1);
    expect(restoreAnchor).toHaveBeenCalledWith(scroller, 'scroll-handler:latest');
    expect(scroller.scrollTop).toBe(730);
    coordinator.release('test-cleanup');
  });

  it('cancels a scheduled anchor restore when semantic ownership is released', () => {
    let scheduledFrame: FrameRequestCallback | null = null;
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      scheduledFrame = callback;
      return 17;
    });
    const cancelFrame = vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});

    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const pinnedItem = document.createElement('div');
    scroller.append(pinnedItem);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 700);
    setRect(scroller, 0);
    setRect(pinnedItem, 57);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.pinElement(pinnedItem);
    const restoreAnchor = vi.spyOn(coordinator, 'restoreElementAnchor');
    coordinator.scheduleElementAnchorRestore(scroller, 'scroll-handler');
    coordinator.release('stream-end-pinned-item');

    expect(cancelFrame).toHaveBeenCalledWith(17);
    expect(scheduledFrame).not.toBeNull();
    (scheduledFrame as FrameRequestCallback)(0);
    expect(restoreAnchor).not.toHaveBeenCalled();
    expect(coordinator.getMode()).toBe('idle');
  });

  it('stops the animation-frame guard after element preservation settles', () => {
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 17);
    const cancelFrame = vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});

    const scroller = document.createElement('div');
    scroller.dataset.virtuosoScroller = 'true';
    const card = document.createElement('div');
    scroller.append(card);
    document.body.append(scroller);
    setScrollerGeometry(scroller, 900);
    setRect(scroller, 0);
    setRect(card, 120);

    const coordinator = new FlowChatViewportCoordinator();
    coordinator.preserveElement(card);
    coordinator.settleElementPreservation('test-settled');

    expect(cancelFrame).toHaveBeenCalledWith(17);
    expect(coordinator.ownsElementAnchor()).toBe(true);
  });
});
