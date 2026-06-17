// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VirtualMessageList } from './VirtualMessageList';
import { activeSessionHistoryProjectionHandoff } from './historyProjectionHandoff';
import type { Session } from '../../types/flow-chat';
import type { VirtualItem } from '../../store/modernFlowChatStore';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as VirtualItem[],
  visibleTurnInfo: null as unknown,
  setVisibleTurnInfo: vi.fn(),
}));

vi.mock('react-virtuoso', () => ({
  Virtuoso: React.forwardRef((props: any, ref) => {
    React.useImperativeHandle(ref, () => ({
      scrollTo: vi.fn(),
      scrollToIndex: vi.fn(),
    }));

    React.useEffect(() => {
      if (props.data?.[0]?.turnId === 'turn-a') {
        props.atBottomStateChange?.(false);
      }
    }, [props]);

    return (
      <div data-testid="virtuoso" data-session-id={stateMocks.activeSession?.sessionId ?? ''}>
        {props.data?.map((item: VirtualItem, index: number) => (
          <div key={item.turnId} className="virtual-item-wrapper" data-turn-id={item.turnId} data-virtual-index={index} data-item-type={item.type}>
            {item.turnId}
          </div>
        ))}
        {props.components?.Footer ? <props.components.Footer /> : null}
      </div>
    );
  }),
}));

vi.mock('../../store/modernFlowChatStore', () => {
  const useModernFlowChatStore = (selector: (state: any) => unknown) => selector({
    visibleTurnInfo: stateMocks.visibleTurnInfo,
  });
  useModernFlowChatStore.getState = () => ({
    visibleTurnInfo: stateMocks.visibleTurnInfo,
    setVisibleTurnInfo: stateMocks.setVisibleTurnInfo,
  });

  return {
    useActiveSession: () => stateMocks.activeSession,
    useVirtualItems: () => stateMocks.virtualItems,
    useModernFlowChatStore,
  };
});

vi.mock('../../hooks/useActiveSessionState', () => ({
  useActiveSessionState: () => ({
    isProcessing: false,
    processingPhase: null,
  }),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: (selector: (state: any) => unknown) => selector({
    isActive: false,
    isExpanded: false,
    inputHeight: 0,
  }),
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    hasPendingSessionHistoryCompletion: () => false,
    hasDeferredSessionHistoryProjection: () => false,
    requestSessionFullHistoryProjection: vi.fn(),
  },
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  startupTrace: { markPhase: vi.fn() },
}));

vi.mock('./VirtualItemRenderer', () => ({
  VirtualItemRenderer: ({ item, index }: { item: VirtualItem; index: number }) => (
    <div className="virtual-item-wrapper" data-turn-id={item.turnId} data-virtual-index={index} data-item-type={item.type}>
      {item.turnId}
    </div>
  ),
}));

vi.mock('../ScrollToLatestBar', () => ({
  ScrollToLatestBar: ({ visible }: { visible: boolean }) => (
    <div data-testid="scroll-to-latest" data-visible={visible ? 'true' : 'false'} />
  ),
}));

vi.mock('../ScrollToTurnHeaderButton', () => ({
  ScrollToTurnHeaderButton: () => null,
}));

vi.mock('../../hooks/useScrollToTurnHeader', () => ({
  useScrollToTurnHeader: () => ({
    shouldShowButton: false,
    handleClick: vi.fn(),
  }),
}));

vi.mock('../../hooks/useVisibleTaskInfo', () => ({
  useVisibleTaskInfo: () => ({
    visibleTaskInfo: null,
    scrollToTask: vi.fn(),
  }),
}));

vi.mock('../StickyTaskIndicator', () => ({
  StickyTaskIndicator: () => null,
}));

vi.mock('./ProcessingIndicator', () => ({
  ProcessingIndicator: () => null,
}));

vi.mock('./processingIndicatorVisibility', () => ({
  shouldReserveProcessingIndicatorSpace: () => false,
  shouldShowProcessingIndicator: () => false,
}));

vi.mock('./ScrollAnchor', () => ({
  ScrollAnchor: () => null,
}));

function createSession(sessionId: string, turnId: string): Session {
  return {
    sessionId,
    title: sessionId,
    dialogTurns: [{
      id: turnId,
      sessionId,
      userMessage: { id: `user-${turnId}`, content: turnId, timestamp: 1 },
      modelRounds: [],
      status: 'completed',
      startTime: 1,
    }],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: false,
    todos: [],
    mode: 'agentic',
    sessionKind: 'normal',
  } as Session;
}

function createItem(turnId: string): VirtualItem {
  return {
    type: 'user-message',
    turnId,
    data: {
      id: `user-${turnId}`,
      content: turnId,
      timestamp: 1,
    },
  } as VirtualItem;
}

describe('VirtualMessageList session boundary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    stateMocks.visibleTurnInfo = null;
    stateMocks.setVisibleTurnInfo.mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('resets viewport-local at-bottom state when the active session changes', () => {
    stateMocks.activeSession = createSession('session-a', 'turn-a');
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-testid="scroll-to-latest"]')?.getAttribute('data-visible')).toBe('true');

    stateMocks.activeSession = createSession('session-b', 'turn-b');
    stateMocks.virtualItems = [createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-testid="scroll-to-latest"]')?.getAttribute('data-visible')).toBe('false');
  });

  it('does not expose stale history projection handoff snapshots across sessions', () => {
    const snapshot = {
      sessionId: 'session-a',
      reason: 'session-open',
      createdAtMs: 1,
      items: [createItem('turn-a')],
      mode: 'bottom-tail',
      targetTurnId: 'turn-a',
      anchorKey: null,
      anchorOffsetTopPx: 0,
      scrollTop: 0,
      scrollHeight: 100,
      clientHeight: 100,
      footerHeightPx: 0,
    } as const;

    expect(activeSessionHistoryProjectionHandoff(snapshot, 'session-a')).toBe(snapshot);
    expect(activeSessionHistoryProjectionHandoff(snapshot, 'session-b')).toBeNull();
    expect(activeSessionHistoryProjectionHandoff(snapshot, null)).toBeNull();
    expect(activeSessionHistoryProjectionHandoff(null, 'session-a')).toBeNull();
  });
});
