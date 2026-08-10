// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FLOWCHAT_TURN_TOP_GAP_PX, tailSpacerPxForViewport } from './flowChatTailFollow';
import { VirtualMessageList, type VirtualMessageListRef } from './VirtualMessageList';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  items: [] as Array<Record<string, unknown>>,
  activeSession: null as Record<string, unknown> | null,
  scrollToIndex: vi.fn(),
  scrollTo: vi.fn(),
  virtuosoProps: null as Record<string, unknown> | null,
  setVisibleTurnInfo: vi.fn(),
  enterFollowOutput: vi.fn(),
  exitFollowOutput: vi.fn(),
}));

vi.mock('react-virtuoso', async () => {
  const ReactModule = await import('react');
  return {
    Virtuoso: ReactModule.forwardRef((props: Record<string, any>, ref) => {
      mocks.virtuosoProps = props;
      ReactModule.useImperativeHandle(ref, () => ({
        scrollToIndex: mocks.scrollToIndex,
        scrollTo: mocks.scrollTo,
      }));
      const scrollerRef = ReactModule.useRef<HTMLDivElement>(null);
      ReactModule.useLayoutEffect(() => {
        props.scrollerRef?.(scrollerRef.current);
        return () => props.scrollerRef?.(null);
      }, [props.scrollerRef]);
      const Header = props.components?.Header;
      const Footer = props.components?.Footer;
      const firstItemIndex = props.firstItemIndex ?? 0;
      return (
        <div ref={scrollerRef} data-virtuoso-scroller="true">
          {Header ? <Header context={props.context} /> : null}
          {props.data.map((item: unknown, index: number) => (
            <ReactModule.Fragment key={index}>
              {props.itemContent(firstItemIndex + index, item)}
            </ReactModule.Fragment>
          ))}
          {Footer ? <Footer context={props.context} /> : null}
        </div>
      );
    }),
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
    handleUserScrollIntent: vi.fn(),
    handleScroll: vi.fn(),
    handleScrollSettled: vi.fn(),
    handleViewportResize: vi.fn(),
    // Follow owns nothing here, which is what the real hook returns when
    // `isFollowingOutput` is false.
    getFollowTargetScrollTop: () => null,
  }),
}));

vi.mock('./VirtualItemRenderer', () => ({
  VirtualItemRenderer: ({ item, index }: { item: any; index: number }) => (
    <div
      className="virtual-item-wrapper"
      data-item-type={item.type}
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
    mocks.scrollToIndex.mockReset();
    mocks.scrollTo.mockReset();
    mocks.enterFollowOutput.mockReset();
    mocks.exitFollowOutput.mockReset();
    mocks.setVisibleTurnInfo.mockReset();
    mocks.virtuosoProps = null;
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

  it('starts bottom-aligned on the last item, with nothing to cancel yet', () => {
    act(() => root.render(<VirtualMessageList />));
    expect(mocks.virtuosoProps?.initialTopMostItemIndex).toEqual({
      index: 1,
      align: 'end',
      offset: 0,
    });
  });

  it('cancels the resident tail spacer in the initial bottom alignment', () => {
    // Virtuoso reveals the whole footer when end-aligning the last item, so
    // without this the session would open on a screen of reserved blank.
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

      // Sized to put a bare new Turn on the top edge, which is less than the
      // viewport — the end of the scroll range must not be a blank screen.
      const expectedSpacerPx = tailSpacerPxForViewport(600, 168);
      expect(expectedSpacerPx).toBeLessThan(600);
      expect(mocks.virtuosoProps?.initialTopMostItemIndex).toEqual({
        index: 1,
        align: 'end',
        offset: -expectedSpacerPx,
      });
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
    expect(mocks.scrollToIndex).toHaveBeenCalledWith({
      index: 1,
      align: 'start',
      // The same breathing gap the Virtuoso header gives the first Turn, so a
      // navigated Turn and the first one are aligned identically.
      offset: -FLOWCHAT_TURN_TOP_GAP_PX,
      behavior: 'auto',
    });
    expect(container.querySelector('.message-list-footer')?.getAttribute('style')).toContain('168px');
  });

  it('prepares history navigation without manufacturing bottom range', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    act(() => root.render(<VirtualMessageList ref={listRef} />));
    expect(listRef.current?.prepareTurnNavigation('turn-2')).toBe('pending');
    expect(container.querySelector('.message-list-footer')?.getAttribute('style')).toContain('168px');
  });
});
