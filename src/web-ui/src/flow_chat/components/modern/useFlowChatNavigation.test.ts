// @vitest-environment jsdom

import React, { act, useRef } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { FlowToolItem, ModelRound, Session } from '../../types/flow-chat';
import { useModernFlowChatStore, type VirtualItem } from '../../store/modernFlowChatStore';
import {
  FLOWCHAT_FOCUS_ITEM_EVENT,
  FLOWCHAT_PIN_TURN_TO_TOP_EVENT,
  type FlowChatFocusItemRequest,
  type FlowChatPinTurnToTopRequest,
} from '../../events/flowchatNavigation';
import { resolveFlowChatFocusTarget } from './flowChatFocusTarget';
import { useFlowChatNavigation } from './useFlowChatNavigation';
import type { VirtualMessageListRef } from './VirtualMessageList';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function makeReadTool(id: string): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Read',
    timestamp: 1000,
    status: 'completed',
    toolCall: {
      id,
      input: { file_path: 'src/main.rs' },
    },
    toolResult: {
      result: 'file contents',
      success: true,
    },
  };
}

function makeRound(items: FlowToolItem[]): ModelRound {
  return {
    id: 'round-1',
    index: 0,
    items,
    isStreaming: false,
    isComplete: true,
    status: 'completed',
    startTime: 1000,
  };
}

function makeSession(round: ModelRound): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Inspect the file',
        timestamp: 900,
      },
      modelRounds: [round],
      status: 'completed',
      startTime: 900,
    }],
    status: 'idle',
    config: {},
    createdAt: 800,
    lastActiveAt: 1000,
    error: null,
    sessionKind: 'flow_chat',
  };
}

describe('useFlowChatNavigation focus resolution', () => {
  it('requests explore group expansion before focusing a grouped tool item', () => {
    const tool = makeReadTool('tool-1');
    const round = makeRound([tool]);
    const session = makeSession(round);
    const virtualItems: VirtualItem[] = [
      {
        type: 'user-message',
        data: session.dialogTurns[0].userMessage,
        turnId: 'turn-1',
      },
      {
        type: 'explore-group',
        turnId: 'turn-1',
        data: {
          groupId: 'round-1',
          rounds: [round],
          allItems: [tool],
          stats: {
            readCount: 1,
            searchCount: 0,
            commandCount: 0,
          },
          isGroupStreaming: false,
          isLastGroupInTurn: true,
        },
      },
    ];

    const target = resolveFlowChatFocusTarget({
      sessionId: session.sessionId,
      turnIndex: 1,
      itemId: tool.id,
      source: 'usage-report',
    }, virtualItems, session);

    expect(target).toMatchObject({
      resolvedVirtualIndex: 1,
      resolvedTurnId: 'turn-1',
      resolvedTurnIndex: 1,
      expandExploreGroupId: 'round-1',
      focusItemId: tool.id,
      preferPinnedTurnNavigation: false,
    });
  });

  it('resolves absolute Turn indexes through the catalog instead of the partial tail array', () => {
    const session: Session = {
      ...makeSession(makeRound([])),
      dialogTurns: [98, 99, 100].map(turnIndex => ({
        id: `turn-${turnIndex}`,
        sessionId: 'session-1',
        backendTurnIndex: turnIndex - 1,
        userMessage: {
          id: `user-${turnIndex}`,
          content: `Prompt ${turnIndex}`,
          timestamp: 1000,
        },
        modelRounds: [],
        status: 'completed' as const,
        startTime: 1000,
      })),
      isPartial: true,
      loadedTurnCount: 3,
      totalTurnCount: 100,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-1',
        revision: 'catalog-1',
        totalTurnCount: 100,
        complete: true,
        entries: Array.from({ length: 100 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          turnId: `turn-${ordinal + 1}`,
          preview: `Prompt ${ordinal + 1}`,
          previewTruncated: false,
        })),
      },
    };

    expect(resolveFlowChatFocusTarget({
      sessionId: session.sessionId,
      turnIndex: 2,
      source: 'usage-report',
    }, [], session)).toMatchObject({
      resolvedTurnId: 'turn-2',
      resolvedTurnIndex: 2,
    });
    expect(resolveFlowChatFocusTarget({
      sessionId: session.sessionId,
      turnIndex: 99,
      source: 'usage-report',
    }, [], session)).toMatchObject({
      resolvedTurnId: 'turn-99',
      resolvedTurnIndex: 99,
    });
  });

  it('treats an explicit stable Turn id as authoritative', () => {
    const session = makeSession(makeRound([]));
    const target = resolveFlowChatFocusTarget({
      sessionId: session.sessionId,
      turnId: 'persisted-parent-turn',
      turnIndex: 1,
      source: 'btw-back',
    }, [], session);

    expect(target).toMatchObject({
      resolvedTurnId: 'persisted-parent-turn',
      resolvedTurnIndex: 1,
      preferPinnedTurnNavigation: true,
    });
  });
});

describe('useFlowChatNavigation Turn pin handling', () => {
  it('does not reuse an absolute Turn index as a local index after delegated navigation fails', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);
    const scrollToTurn = vi.fn();
    const onNavigateToFocusTurn = vi.fn(async () => false);
    const partialSession: Session = {
      ...makeSession(makeRound([])),
      dialogTurns: [98, 99, 100].map(turnIndex => ({
        id: `turn-${turnIndex}`,
        sessionId: 'session-1',
        backendTurnIndex: turnIndex - 1,
        userMessage: {
          id: `user-${turnIndex}`,
          content: `Prompt ${turnIndex}`,
          timestamp: 1000,
        },
        modelRounds: [],
        status: 'completed' as const,
        startTime: 1000,
      })),
      isPartial: true,
      loadedTurnCount: 3,
      totalTurnCount: 100,
    };
    useModernFlowChatStore.getState().setActiveSession(partialSession);

    function Harness() {
      const virtualListRef = useRef<VirtualMessageListRef | null>({
        scrollToTurn,
      } as unknown as VirtualMessageListRef);
      useFlowChatNavigation({
        activeSessionId: partialSession.sessionId,
        virtualItems: useModernFlowChatStore.getState().virtualItems,
        virtualListRef,
        onNavigateToFocusTurn,
      });
      return null;
    }

    await act(async () => {
      root.render(React.createElement(Harness));
    });
    const request: FlowChatFocusItemRequest = {
      sessionId: partialSession.sessionId,
      turnIndex: 2,
      source: 'usage-report',
    };
    await act(async () => {
      globalEventBus.emit(FLOWCHAT_FOCUS_ITEM_EVENT, request, 'test');
      await new Promise(resolve => window.setTimeout(resolve, 0));
    });

    expect(onNavigateToFocusTurn).toHaveBeenCalledWith(request);
    expect(scrollToTurn).not.toHaveBeenCalled();

    await act(async () => {
      root.unmount();
    });
    useModernFlowChatStore.getState().clear();
    container.remove();
  });

  it('runs the pre-pin callback before pinning a newly submitted Turn', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);
    const onBeforeTurnPinRequest = vi.fn();
    const pinTurnToTop = vi.fn(() => true);
    const virtualItems: VirtualItem[] = [{
      type: 'user-message',
      turnId: 'turn-2',
      data: {
        id: 'user-2',
        content: 'New prompt',
        timestamp: 1000,
      },
    }];

    function Harness() {
      const virtualListRef = useRef<VirtualMessageListRef | null>({
        pinTurnToTop,
      } as unknown as VirtualMessageListRef);
      useFlowChatNavigation({
        activeSessionId: 'session-1',
        virtualItems,
        virtualListRef,
        onBeforeTurnPinRequest,
      });
      return null;
    }

    await act(async () => {
      root.render(React.createElement(Harness));
    });
    const request: FlowChatPinTurnToTopRequest = {
      sessionId: 'session-1',
      turnId: 'turn-2',
      behavior: 'auto',
      source: 'send-message',
      pinMode: 'sticky-latest',
    };
    await act(async () => {
      globalEventBus.emit(FLOWCHAT_PIN_TURN_TO_TOP_EVENT, request, 'test');
      await Promise.resolve();
    });

    expect(onBeforeTurnPinRequest).toHaveBeenCalledWith(request);
    expect(pinTurnToTop).toHaveBeenCalledWith('turn-2', {
      behavior: 'auto',
      pinMode: 'sticky-latest',
    });
    expect(onBeforeTurnPinRequest.mock.invocationCallOrder[0]).toBeLessThan(
      pinTurnToTop.mock.invocationCallOrder[0],
    );

    await act(async () => {
      root.unmount();
    });
    container.remove();
  });
});
