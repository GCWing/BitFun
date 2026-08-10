// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { USER_DRIVEN_SCROLL_WINDOW_MS } from './flowChatViewportAnchor';
import {
  useFlowChatViewportAnchor,
  type FlowChatViewportAnchorApi,
} from './useFlowChatViewportAnchor';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const VIEWPORT_HEIGHT = 500;
const TURN_HEIGHT = 40;

function rect(top: number, height: number): DOMRect {
  return {
    top,
    bottom: top + height,
    height,
    left: 0,
    right: 800,
    width: 800,
    x: 0,
    y: top,
    toJSON: () => ({}),
  } as DOMRect;
}

interface HarnessProps {
  scroller: HTMLElement;
  isViewportOwnedElsewhere: () => boolean;
  onApi: (api: FlowChatViewportAnchorApi) => void;
}

function Harness({ scroller, isViewportOwnedElsewhere, onApi }: HarnessProps) {
  const scrollerRef = React.useRef<HTMLElement | null>(scroller);
  onApi(useFlowChatViewportAnchor({ scrollerRef, isViewportOwnedElsewhere }));
  return null;
}

describe('useFlowChatViewportAnchor', () => {
  let container: HTMLDivElement;
  let root: Root;
  let isMounted: boolean;
  let scroller: HTMLDivElement;
  let api: FlowChatViewportAnchorApi;
  let frames: FrameRequestCallback[];
  let now: number;
  let isOwnedElsewhere: boolean;

  /**
   * Place Turns in the transcript's own coordinate space.
   *
   * Rects are read back relative to the viewport, so writing `scrollTop` moves
   * them exactly as a real scroller would — which is what makes a correction
   * observable, and a second correction a no-op.
   */
  function layoutTurns(turnTops: Record<string, number>) {
    scroller.replaceChildren();
    for (const [turnId, contentTop] of Object.entries(turnTops)) {
      const element = document.createElement('div');
      element.className = 'virtual-item-wrapper';
      element.dataset.itemType = 'user-message';
      element.dataset.turnId = turnId;
      element.getBoundingClientRect = () => rect(contentTop - scroller.scrollTop, TURN_HEIGHT);
      scroller.append(element);
    }
  }

  function runFrame() {
    const frame = frames.shift();
    expect(frame).toBeDefined();
    act(() => frame?.(16));
  }

  function unmount() {
    if (!isMounted) return;
    isMounted = false;
    act(() => root.unmount());
  }

  beforeEach(() => {
    container = document.createElement('div');
    scroller = document.createElement('div');
    document.body.append(container, scroller);
    scroller.getBoundingClientRect = () => rect(0, VIEWPORT_HEIGHT);
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, writable: true, value: 0 });
    frames = [];
    now = 10_000;
    isOwnedElsewhere = false;
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    vi.spyOn(performance, 'now').mockImplementation(() => now);

    root = createRoot(container);
    isMounted = true;
    act(() => root.render(
      <Harness
        scroller={scroller}
        isViewportOwnedElsewhere={() => isOwnedElsewhere}
        onApi={next => { api = next; }}
      />,
    ));
  });

  afterEach(() => {
    unmount();
    container.remove();
    scroller.remove();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('puts the anchored Turn back where the reader left it', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980, 'turn-4': 1440 });
    api.captureAnchor();

    // A late measurement above the viewport pushes everything down 880px.
    layoutTurns({ 'turn-3': 1860, 'turn-4': 2320 });

    expect(api.restoreAnchor()).toBe(true);
    expect(scroller.scrollTop).toBe(1880);
  });

  it('is idempotent, because it restores a relationship and not a delta', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980 });
    api.captureAnchor();
    layoutTurns({ 'turn-3': 1860 });

    api.restoreAnchor();
    const settled = scroller.scrollTop;
    api.restoreAnchor();
    api.restoreAnchor();

    expect(scroller.scrollTop).toBe(settled);
  });

  it('reports that it could not answer when there is no anchor', () => {
    layoutTurns({ 'turn-3': 100 });
    // Nothing captured yet: the caller has to fall back on something coarser.
    expect(api.restoreAnchor()).toBe(false);
    expect(scroller.scrollTop).toBe(0);
  });

  it('stays out of the way while another writer owns the viewport', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980 });
    api.captureAnchor();
    layoutTurns({ 'turn-3': 1860 });

    isOwnedElsewhere = true;

    expect(api.restoreAnchor()).toBe(false);
    expect(scroller.scrollTop).toBe(1000);
  });

  it('drops an anchor whose Turn has left the rendered window', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980 });
    api.captureAnchor();

    layoutTurns({ 'turn-9': 980 });
    expect(api.restoreAnchor()).toBe(false);
    expect(scroller.scrollTop).toBe(1000);

    // Dropped rather than kept: the next scroll anchors to a Turn that is
    // actually on screen, without waiting for an intent event to allow it.
    layoutTurns({ 'turn-9': 1200 });
    api.captureAnchorForScroll();
    layoutTurns({ 'turn-9': 1700 });
    expect(api.restoreAnchor()).toBe(true);
    expect(scroller.scrollTop).toBe(1500);
  });

  it('re-anchors on a scroll the user drove, and not on one they did not', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980, 'turn-4': 2500 });
    api.captureAnchor();

    // The transcript moved under the reader, with no gesture behind it.
    layoutTurns({ 'turn-3': 1860, 'turn-4': 3380 });
    now += 5_000;
    api.captureAnchorForScroll();
    expect(api.restoreAnchor()).toBe(true);
    expect(scroller.scrollTop).toBe(1880);

    // The reader scrolls on: the position they arrived at is the new anchor,
    // and turn-4 is what they are now looking at.
    scroller.scrollTop = 3400;
    api.markUserScrollIntent();
    now += USER_DRIVEN_SCROLL_WINDOW_MS - 1;
    api.captureAnchorForScroll();

    layoutTurns({ 'turn-3': 2360, 'turn-4': 3880 });
    expect(api.restoreAnchor()).toBe(true);
    expect(scroller.scrollTop).toBe(3900);
  });

  it('re-asserts the anchor across the frames a settle takes', () => {
    scroller.scrollTop = 1000;
    layoutTurns({ 'turn-3': 980 });
    api.captureAnchor();

    api.openSettleWindow();
    // Nothing has moved yet on the first frame — the heights land later.
    runFrame();
    expect(scroller.scrollTop).toBe(1000);

    layoutTurns({ 'turn-3': 1860 });
    runFrame();
    expect(scroller.scrollTop).toBe(1880);

    // And again, for a second batch of measurements in the same settle.
    layoutTurns({ 'turn-3': 2500 });
    runFrame();
    expect(scroller.scrollTop).toBe(2520);
  });

  it('opens one settle window at a time', () => {
    layoutTurns({ 'turn-3': 100 });
    api.captureAnchor();

    api.openSettleWindow();
    api.openSettleWindow();
    api.openSettleWindow();

    expect(frames).toHaveLength(1);
  });

  it('winds the settle window down once there is no anchor to keep', () => {
    // No anchor was ever captured, so every frame is a wasted one and the
    // window has to end on its own rather than run forever.
    api.openSettleWindow();
    let ranFrames = 0;
    while (frames.length > 0 && ranFrames < 100) {
      runFrame();
      ranFrames += 1;
    }
    expect(ranFrames).toBeGreaterThan(1);
    expect(frames).toHaveLength(0);
  });

  it('keeps a frame in flight for as long as an anchor stands', () => {
    /*
     * A frame that answered refreshes the window, and a frame that found the
     * anchor already in place counts as answered — so an open transcript that
     * nobody else is writing to holds the loop open indefinitely. That is the
     * shipped behaviour, and its cost is one `querySelectorAll` and two rect
     * reads per frame for as long as a session is open.
     */
    layoutTurns({ 'turn-3': 100 });
    api.captureAnchor();
    api.openSettleWindow();

    for (let index = 0; index < 60; index += 1) runFrame();

    expect(frames).toHaveLength(1);
    expect(scroller.scrollTop).toBe(0);
  });

  it('stops re-asserting after the component goes away', () => {
    layoutTurns({ 'turn-3': 100 });
    api.captureAnchor();
    api.openSettleWindow();

    unmount();

    expect(cancelAnimationFrame).toHaveBeenCalled();
  });
});
