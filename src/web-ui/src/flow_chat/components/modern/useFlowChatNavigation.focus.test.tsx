// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/*
 * The two properties this file exists for, both measured from a usage-report
 * click before they held:
 *
 * - the aim goes through the register, not `element.scrollIntoView`;
 * - it is attempted in the same task as the Turn navigation, so the reader is
 *   not shown the Turn placement and then moved off it. Measured: the Turn
 *   navigation settled 178px and 334.7px from where it put itself, because
 *   this aim arrived three frames later and the intermediate frame was painted.
 */

const mocks = vi.hoisted(() => ({
  resolveFlowChatFocusTarget: vi.fn(),
  switchChatSession: vi.fn(),
  activeSessionId: 'session-1' as string | undefined,
}));

vi.mock('../../services/FlowChatManager', () => ({
  flowChatManager: { switchChatSession: mocks.switchChatSession },
}));
vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: { getState: () => ({ sessions: new Map() }) },
}));
vi.mock('../../store/modernFlowChatStore', () => ({
  useModernFlowChatStore: {
    getState: () => ({ activeSession: { sessionId: mocks.activeSessionId } }),
  },
}));
vi.mock('./flowChatFocusTarget', () => ({
  resolveFlowChatFocusTarget: mocks.resolveFlowChatFocusTarget,
}));

const { globalEventBus } = await import('@/infrastructure/event-bus');
const { FLOWCHAT_FOCUS_ITEM_EVENT } = await import('../../events/flowchatNavigation');
const { useFlowChatNavigation } = await import('./useFlowChatNavigation');

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

// jsdom ships no CSS.escape. The ids here need no escaping; this only has to
// exist so the selector can be built.
if (typeof globalThis.CSS === 'undefined') {
  (globalThis as { CSS?: unknown }).CSS = {};
}
if (typeof CSS.escape !== 'function') {
  CSS.escape = (value: string) => value.replace(/["\\]/g, '\\$&');
}

const FOCUS_ITEM_ID = 'call_00_zknuBLUKP7Y6JTioDI5Z8386';

function Harness({ listRef }: { listRef: React.RefObject<any> }) {
  useFlowChatNavigation({
    activeSessionId: 'session-1',
    virtualItems: [],
    virtualListRef: listRef,
  });
  return null;
}

describe('useFlowChatNavigation focus placement', () => {
  let container: HTMLDivElement;
  let root: Root;
  let frames: FrameRequestCallback[];
  let listRef: React.RefObject<any>;
  let focusFlowItem: ReturnType<typeof vi.fn>;
  let scrollIntoView: ReturnType<typeof vi.fn>;

  function renderItem() {
    const element = document.createElement('div');
    element.dataset.flowItemId = FOCUS_ITEM_ID;
    element.scrollIntoView = scrollIntoView as unknown as HTMLElement['scrollIntoView'];
    document.body.append(element);
    return element;
  }

  async function dispatchFocusRequest() {
    await act(async () => {
      globalEventBus.emit(FLOWCHAT_FOCUS_ITEM_EVENT, {
        sessionId: 'session-1',
        itemId: FOCUS_ITEM_ID,
      });
      // Let the handler's own awaits resolve, without granting it a frame.
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    mocks.activeSessionId = 'session-1';
    mocks.resolveFlowChatFocusTarget.mockReturnValue({ preferTurnNavigation: false });
    scrollIntoView = vi.fn();
    focusFlowItem = vi.fn(() => true);
    listRef = { current: { focusFlowItem, navigateToTurn: vi.fn(), scrollToIndex: vi.fn(), scrollToTurn: vi.fn() } };
    frames = [];
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
    act(() => root.render(<Harness listRef={listRef} />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.replaceChildren();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('aims through the register rather than the element', async () => {
    renderItem();
    await dispatchFocusRequest();

    expect(focusFlowItem).toHaveBeenCalledWith(FOCUS_ITEM_ID);
    expect(scrollIntoView).not.toHaveBeenCalled();
  });

  it('aims without first yielding a frame to the Turn placement', async () => {
    renderItem();
    await dispatchFocusRequest();

    // Nothing has been given a frame yet: every queued callback is still queued.
    expect(frames).toHaveLength(0);
    expect(focusFlowItem).toHaveBeenCalledTimes(1);
  });

  it('asks again on the next frame while the item is still unrendered', async () => {
    await dispatchFocusRequest();
    expect(focusFlowItem).not.toHaveBeenCalled();
    expect(frames).toHaveLength(1);

    renderItem();
    await act(async () => { frames.shift()?.(16); });

    expect(focusFlowItem).toHaveBeenCalledWith(FOCUS_ITEM_ID);
  });
});
