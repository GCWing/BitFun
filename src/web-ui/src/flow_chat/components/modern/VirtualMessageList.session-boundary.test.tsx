// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tailSpacerPxForViewport } from './flowChatTailFollow';
import { ONE_SHOT_NAVIGATION_HOLD_MS } from './flowChatViewportOwnership';
import { VirtualMessageList, type VirtualMessageListRef } from './VirtualMessageList';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  items: [] as Array<Record<string, unknown>>,
  activeSession: null as Record<string, unknown> | null,
  scrollItemIntoView: vi.fn(),
  scrollToOffset: vi.fn(),
  setVisibleTurnInfo: vi.fn(),
  enterFollowOutput: vi.fn(),
  exitFollowOutput: vi.fn(),
  handleUserScrollIntent: vi.fn(),
  /** False stands in for a Turn the virtualizer can place but the DOM cannot. */
  renderItemMetadata: true,
}));

/** Input-stack footer the chat-input mock produces: 140 + 4 + 24. */
const BOTTOM_INSET = 168;

/**
 * jsdom has no layout engine, so both halves of the navigation clamp have to be
 * supplied: the scroller's own box, and where a user message sits inside it.
 */
function fakeLayout(options: {
  clientHeight: number;
  scrollHeight: number;
  turnTopFromScrollerTop: number;
}) {
  const originals = (['clientHeight', 'scrollHeight'] as const).map(name => {
    const descriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, name);
    Object.defineProperty(HTMLElement.prototype, name, {
      configurable: true,
      get: () => (name === 'clientHeight' ? options.clientHeight : options.scrollHeight),
    });
    return [name, descriptor] as const;
  });
  const originalRect = HTMLElement.prototype.getBoundingClientRect;
  HTMLElement.prototype.getBoundingClientRect = function getRect(this: HTMLElement) {
    const top = this.classList.contains('virtual-item-wrapper')
      ? options.turnTopFromScrollerTop
      : 0;
    return { ...new DOMRect(0, top, 0, 40), top, bottom: top + 40 } as DOMRect;
  };

  return () => {
    HTMLElement.prototype.getBoundingClientRect = originalRect;
    originals.forEach(([name, descriptor]) => {
      if (descriptor) {
        Object.defineProperty(HTMLElement.prototype, name, descriptor);
      } else {
        delete (HTMLElement.prototype as unknown as Record<string, unknown>)[name];
      }
    });
  };
}

/*
 * The virtualizer is mocked at FlowChat's own seam rather than at the library.
 * These tests are about which scroll the list decides to ask for, and the seam
 * is where that decision is expressed; the library's own behaviour belongs to
 * the library. Every item is rendered, which is what a list shorter than the
 * viewport would do anyway.
 */
vi.mock('./useFlowChatVirtualizer', async () => {
  // The visible range is real geometry, and the paging rule reads it. Faking it
  // would leave the rule tested against an answer no viewport can produce.
  const actual = await vi.importActual<typeof import('./useFlowChatVirtualizer')>(
    './useFlowChatVirtualizer',
  );
  return {
    ...actual,
    useFlowChatVirtualizer: (options: {
      items: Array<Record<string, unknown>>;
      getItemKey: (item: Record<string, unknown>) => string;
      scrollerRef: { current: HTMLElement | null };
    }) => {
      const rows = options.items.map((item, index) => ({
        index,
        key: options.getItemKey(item),
        startPx: index * 40,
        endPx: index * 40 + 40,
      }));
      return {
        rows,
        paddingTopPx: 0,
        paddingBottomPx: 0,
        measureRowElement: () => {},
        getItemBounds: (index: number) => ({ startPx: index * 40, endPx: index * 40 + 40 }),
        getVisibleItemRange: () => {
          const scroller = options.scrollerRef.current;
          return scroller
            ? actual.visibleRowRange(rows, scroller.scrollTop, scroller.clientHeight)
            : null;
        },
        scrollItemIntoView: mocks.scrollItemIntoView,
        scrollToOffset: mocks.scrollToOffset,
      };
    },
  };
});

vi.mock('../../store/modernFlowChatStore', () => {
  const useModernFlowChatStore = Object.assign(
    (selector: (state: Record<string, unknown>) => unknown) => selector({ visibleTurnInfo: null }),
    { getState: () => ({ setVisibleTurnInfo: mocks.setVisibleTurnInfo }) },
  );
  return {
    useVirtualItems: () => mocks.items,
    useActiveSession: () => mocks.activeSession,
    useModernFlowChatStore,
  };
});

vi.mock('../../hooks/useActiveSessionState', () => ({
  useActiveSessionState: () => ({ isProcessing: false }),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: (selector: (state: Record<string, unknown>) => unknown) => selector({
    isActive: false,
    isExpanded: false,
    inputHeight: 140,
  }),
}));

vi.mock('./useFlowChatFollowOutput', () => ({
  useFlowChatFollowOutput: () => ({
    isFollowingOutput: false,
    enterFollowOutput: mocks.enterFollowOutput,
    exitFollowOutput: mocks.exitFollowOutput,
    scheduleFollowToLatest: vi.fn(),
    handleUserScrollIntent: mocks.handleUserScrollIntent,
    handleScroll: vi.fn(),
    handleScrollSettled: vi.fn(),
    handleViewportResize: vi.fn(),
    // Follow owns nothing here, which is what the real hook returns when
    // `isFollowingOutput` is false.
    getFollowTargetScrollTop: () => null,
  }),
}));

vi.mock('./VirtualItemRenderer', () => ({
  VirtualItemRenderer: ({ item, index, measureRef }: {
    item: any;
    index: number;
    measureRef?: (element: HTMLElement | null) => void;
  }) => (
    <div
      ref={measureRef}
      className="virtual-item-wrapper"
      data-item-type={mocks.renderItemMetadata ? item.type : undefined}
      data-turn-id={item.turnId}
      data-virtual-index={index}
    >
      {item.data?.content ?? item.turnId}
    </div>
  ),
}));

vi.mock('../../hooks/useScrollToTurnHeader', () => ({
  useScrollToTurnHeader: () => ({ shouldShowButton: false, handleClick: vi.fn() }),
}));

vi.mock('../../hooks/useVisibleTaskInfo', () => ({
  useVisibleTaskInfo: () => ({ visibleTaskInfo: null, scrollToTask: vi.fn() }),
}));

vi.mock('./RuntimeStatusSlot', () => ({ RuntimeStatusSlot: () => <div data-runtime-status /> }));
vi.mock('../ScrollToLatestBar', () => ({ ScrollToLatestBar: () => null }));
vi.mock('../ScrollToTurnHeaderButton', () => ({ ScrollToTurnHeaderButton: () => null }));
vi.mock('../StickyTaskIndicator', () => ({ StickyTaskIndicator: () => null }));

function userMessage(turnId: string, id: string, content: string) {
  return {
    type: 'user-message',
    turnId,
    data: { id, content },
  };
}

describe('VirtualMessageList natural scroll contract', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.items = [
      userMessage('turn-1', 'message-1', 'First'),
      userMessage('turn-2', 'message-2', 'Second'),
    ];
    mocks.activeSession = {
      sessionId: 'session-1',
      dialogTurns: [],
    };
    mocks.scrollItemIntoView.mockReset();
    mocks.scrollToOffset.mockReset();
    mocks.enterFollowOutput.mockReset();
    mocks.exitFollowOutput.mockReset();
    mocks.handleUserScrollIntent.mockReset();
    mocks.setVisibleTurnInfo.mockReset();
    mocks.renderItemMetadata = true;
    vi.stubGlobal('ResizeObserver', class {
      observe() {}
      disconnect() {}
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('renders only the current input layout inset in the Footer', () => {
    act(() => root.render(<VirtualMessageList />));
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    expect(footer?.style.height).toBe('168px');
    expect(footer?.style.minHeight).toBe('168px');
  });

  it('reserves a tail spacer sized from the viewport and nothing else', () => {
    // The session opens on the end of *real content*, which is above this
    // reservation. Nothing aligns to the last item any more: the end of the
    // scroll range is reserved blank, and opening there is opening on nothing.
    const originalClientHeight = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      'clientHeight',
    );
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
      configurable: true,
      get: () => 600,
    });

    try {
      act(() => root.render(<VirtualMessageList />));

      const expectedSpacerPx = tailSpacerPxForViewport(600, BOTTOM_INSET);
      expect(expectedSpacerPx).toBeLessThan(600);
      const spacer = container.querySelector<HTMLElement>('.message-list-tail-spacer');
      expect(spacer?.style.height).toBe(`${expectedSpacerPx}px`);
      // The input-stack footer stays a separate reservation. It feeds the
      // spacer's size, but the two are never folded into one number.
      expect(container.querySelector<HTMLElement>('.message-list-footer')?.style.height)
        .toBe('168px');
    } finally {
      if (originalClientHeight) {
        Object.defineProperty(HTMLElement.prototype, 'clientHeight', originalClientHeight);
      } else {
        delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientHeight;
      }
    }
  });

  it('navigates a Turn with best-effort start alignment and no range reservation', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    act(() => root.render(<VirtualMessageList ref={listRef} />));
    let accepted = false;
    act(() => {
      accepted = listRef.current?.navigateToTurn('turn-2', { behavior: 'auto' }) ?? false;
    });
    expect(accepted).toBe(true);
    expect(mocks.exitFollowOutput).toHaveBeenCalledWith('scroll-to-turn');
    // The breathing gap above a top-aligned Turn is the virtualizer's
    // `scrollPaddingStart`, so an aim re-taken while items measure keeps it.
    expect(mocks.scrollItemIntoView).toHaveBeenCalledWith(1, {
      align: 'start',
      behavior: 'auto',
      // The aim carries its owner, so the re-aims it produces are still the
      // navigation's and are refused for anything that outranks it.
      owner: 'one-shot-navigation',
      holdForMs: ONE_SHOT_NAVIGATION_HOLD_MS,
    });
    expect(container.querySelector('.message-list-footer')?.getAttribute('style')).toContain('168px');
  });

  it('top-aligns a Turn that still has a transcript below it', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    // Content end at 3000 - 392 - 600 = 2008, well below the Turn's top at 492.
    const restoreLayout = fakeLayout({
      clientHeight: 600,
      scrollHeight: 3000,
      turnTopFromScrollerTop: 500,
    });
    try {
      act(() => root.render(<VirtualMessageList ref={listRef} />));
      act(() => { listRef.current?.navigateToTurn('turn-2', { behavior: 'smooth' }); });

      expect(mocks.scrollItemIntoView).toHaveBeenCalledTimes(1);
      expect(mocks.scrollItemIntoView).toHaveBeenCalledWith(1, {
        align: 'start',
        // A resolvable Turn is clamped before anything moves, so the requested
        // animation survives.
        behavior: 'smooth',
        owner: 'one-shot-navigation',
        holdForMs: ONE_SHOT_NAVIGATION_HOLD_MS,
      });
    } finally {
      restoreLayout();
    }
  });

  it('stops a short tail Turn at the content end rather than in the blank', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    // Content end at 1000 - 392 - 600 = 8; top-aligning would mean 492, which
    // is a screen of reserved blank nothing is going to fill.
    const restoreLayout = fakeLayout({
      clientHeight: 600,
      scrollHeight: 1000,
      turnTopFromScrollerTop: 500,
    });
    try {
      act(() => root.render(<VirtualMessageList ref={listRef} />));
      act(() => { listRef.current?.navigateToTurn('turn-2', { behavior: 'smooth' }); });

      // Aimed at the end of real content, read off live geometry — not at the
      // last item, whose end is the bottom of the reserved blank.
      expect(mocks.scrollItemIntoView).not.toHaveBeenCalled();
      expect(mocks.scrollToOffset).toHaveBeenCalledTimes(1);
      expect(mocks.scrollToOffset).toHaveBeenCalledWith(
        1000 - tailSpacerPxForViewport(600, BOTTOM_INSET) - 600,
        { behavior: 'smooth', owner: 'one-shot-navigation' },
      );
    } finally {
      restoreLayout();
    }
  });

  it('reads an unrendered Turn back from the virtualizer, and corrects through it', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const restoreLayout = fakeLayout({
      clientHeight: 600,
      scrollHeight: 1000,
      turnTopFromScrollerTop: 500,
    });
    try {
      mocks.renderItemMetadata = false;
      act(() => root.render(<VirtualMessageList ref={listRef} />));
      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      Object.defineProperty(scroller, 'scrollTop', {
        configurable: true,
        writable: true,
        value: 0,
      });
      // Only the virtualizer knows where the Turn is; it lands the viewport in
      // the reserved blank.
      mocks.scrollItemIntoView.mockImplementation(() => { scroller.scrollTop = 900; });

      act(() => { listRef.current?.navigateToTurn('turn-2', { behavior: 'smooth' }); });

      // Placed instantly, because an animation would not have arrived yet and
      // there would be nothing to read back.
      expect(mocks.scrollItemIntoView).toHaveBeenCalledTimes(1);
      expect(mocks.scrollItemIntoView).toHaveBeenCalledWith(1, {
        align: 'start',
        owner: 'one-shot-navigation',
        holdForMs: ONE_SHOT_NAVIGATION_HOLD_MS,
      });
      // Corrected through the virtualizer, not the scroller: a direct write
      // would leave the first call's re-aim pending, and it aims at the top.
      expect(mocks.scrollToOffset).toHaveBeenCalledWith(
        1000 - tailSpacerPxForViewport(600, BOTTOM_INSET) - 600,
        { behavior: 'auto', owner: 'one-shot-navigation' },
      );
    } finally {
      restoreLayout();
    }
  });

  describe('scrollbar drags release the viewport', () => {
    // Content box ends at 0 + 1384; the gutter runs from there to 1394.
    const CONTENT_BOX_WIDTH = 1384;

    function pressAt(clientX: number) {
      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      Object.defineProperty(scroller, 'clientWidth', {
        configurable: true,
        value: CONTENT_BOX_WIDTH,
      });
      act(() => {
        // jsdom has no PointerEvent; only `clientX` is read.
        scroller.dispatchEvent(new MouseEvent('pointerdown', { clientX, bubbles: true }));
        scroller.dispatchEvent(new Event('scroll'));
      });
    }

    it('treats a scroll under a scrollbar press as intent', () => {
      act(() => root.render(<VirtualMessageList />));
      pressAt(CONTENT_BOX_WIDTH + 6);
      expect(mocks.handleUserScrollIntent).toHaveBeenCalled();
    });

    it('leaves a scroll under a press on the transcript alone', () => {
      // Layout growth and virtualizer remeasurement emit scroll events too, so
      // the press is what qualifies one — not the event itself.
      act(() => root.render(<VirtualMessageList />));
      pressAt(CONTENT_BOX_WIDTH - 200);
      expect(mocks.handleUserScrollIntent).not.toHaveBeenCalled();
    });

    it('disarms on release, so a later scroll is not intent', () => {
      act(() => root.render(<VirtualMessageList />));
      pressAt(CONTENT_BOX_WIDTH + 6);
      mocks.handleUserScrollIntent.mockClear();

      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      act(() => {
        window.dispatchEvent(new MouseEvent('pointerup'));
        scroller.dispatchEvent(new Event('scroll'));
      });

      expect(mocks.handleUserScrollIntent).not.toHaveBeenCalled();
    });
  });

  describe('history arriving above the viewport', () => {
    it('moves the viewport by the height that was prepended', () => {
      act(() => root.render(<VirtualMessageList />));
      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      scroller.scrollTop = 500;

      mocks.items = [
        userMessage('turn-a', 'message-a', 'Older'),
        userMessage('turn-b', 'message-b', 'Older'),
        userMessage('turn-c', 'message-c', 'Older'),
        ...mocks.items,
      ];
      act(() => root.render(<VirtualMessageList />));

      // Three 40px items arrived above, so the reader's content is 120px lower
      // and the viewport follows it. Anything less leaves them looking at
      // history they never asked to be shown.
      expect(scroller.scrollTop).toBe(620);
    });

    it('leaves the viewport alone when the transcript grows at the end', () => {
      act(() => root.render(<VirtualMessageList />));
      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      scroller.scrollTop = 500;

      mocks.items = [...mocks.items, userMessage('turn-3', 'message-3', 'Newer')];
      act(() => root.render(<VirtualMessageList />));

      expect(scroller.scrollTop).toBe(500);
    });

    it('leaves the viewport alone when the head is trimmed', () => {
      act(() => root.render(<VirtualMessageList />));
      const scroller = container.querySelector<HTMLElement>('[data-flowchat-scroller]')!;
      scroller.scrollTop = 500;

      // The item that was first is gone rather than moved, so there is no
      // arrived height to account for and nothing to compensate with.
      mocks.items = mocks.items.slice(1);
      act(() => root.render(<VirtualMessageList />));

      expect(scroller.scrollTop).toBe(500);
    });
  });

  it('prepares history navigation without manufacturing bottom range', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    act(() => root.render(<VirtualMessageList ref={listRef} />));
    expect(listRef.current?.prepareTurnNavigation('turn-2')).toBe('pending');
    expect(container.querySelector('.message-list-footer')?.getAttribute('style')).toContain('168px');
  });
});
