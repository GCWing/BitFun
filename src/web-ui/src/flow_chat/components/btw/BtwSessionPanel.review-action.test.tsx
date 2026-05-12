// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { BtwSessionPanel } from './BtwSessionPanel';
import { useReviewActionBarStore } from '../../store/deepReviewActionBarStore';
import type { FlowChatState, Session } from '../../types/flow-chat';

let flowChatState: FlowChatState;
const translate = (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => (
  options?.defaultValue ?? _key
);

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: translate,
  }),
}));

vi.mock('../modern/VirtualItemRenderer', () => ({
  VirtualItemRenderer: () => <div />,
}));

vi.mock('../modern/ProcessingIndicator', () => ({
  ProcessingIndicator: () => <div />,
}));

vi.mock('../modern/processingIndicatorVisibility', () => ({
  shouldReserveProcessingIndicatorSpace: () => false,
  shouldShowProcessingIndicator: () => false,
}));

vi.mock('../modern/useExploreGroupState', () => ({
  useExploreGroupState: () => ({
    exploreGroupStates: {},
    onExploreGroupToggle: vi.fn(),
    onExpandGroup: vi.fn(),
    onExpandAllInTurn: vi.fn(),
    onCollapseGroup: vi.fn(),
  }),
}));

vi.mock('@/flow_chat', () => ({
  ScrollToBottomButton: () => <div />,
}));

vi.mock('./DeepReviewActionBar', () => ({
  ReviewActionBar: () => <div data-testid="review-action-bar" />,
}));

vi.mock('@/component-library', () => ({
  IconButton: ({
    children,
    onClick,
  }: {
    children: React.ReactNode;
    onClick?: () => void;
  }) => (
    <button type="button" onClick={onClick}>
      {children}
    </button>
  ),
}));

vi.mock('@/shared/services/FileTabManager', () => ({
  fileTabManager: {
    openFile: vi.fn(),
  },
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  createTab: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    cancelSession: vi.fn(),
  },
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
  }),
}));

vi.mock('../../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => flowChatState,
      subscribe: () => () => {},
      loadSessionHistory: vi.fn(),
    }),
  },
  flowChatStore: {
    getState: () => flowChatState,
    subscribe: () => () => {},
    loadSessionHistory: vi.fn(),
  },
}));

vi.mock('../../store/modernFlowChatStore', () => ({
  sessionToVirtualItems: () => [],
}));

vi.mock('../../utils/reviewSessionStop', () => ({
  settleStoppedReviewSessionState: vi.fn(),
}));

vi.mock('../../services/ReviewActionBarPersistenceService', () => ({
  loadPersistedReviewState: vi.fn(() => Promise.resolve(null)),
}));

function createReviewSession(): Session {
  return {
    sessionId: 'deep-review-child',
    title: 'Deep review',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'deep-review-child',
      userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
      modelRounds: [{
        id: 'round-1',
        index: 0,
        isStreaming: false,
        isComplete: true,
        status: 'completed',
        startTime: 1,
        items: [{
          id: 'review-result',
          type: 'tool',
          timestamp: 2,
          status: 'completed',
          toolName: 'submit_code_review',
          toolCall: { id: 'tool-1', input: {} },
          toolResult: {
            success: true,
            result: JSON.stringify({
              summary: {
                overall_assessment: 'Looks safe.',
                risk_level: 'low',
                recommended_action: 'approve',
              },
              issues: [],
              positive_points: ['No risky changes found.'],
              review_mode: 'deep',
              remediation_plan: [],
            }),
          },
        }],
      }],
      status: 'completed',
      startTime: 1,
    }],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'deep_review',
    parentSessionId: 'parent-session',
    workspacePath: 'D:/workspace/project',
  } as Session;
}

describe('BtwSessionPanel review action bar integration', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    useReviewActionBarStore.getState().reset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const childSession = createReviewSession();
    flowChatState = {
      sessions: new Map([
        ['deep-review-child', childSession],
        ['parent-session', {
          sessionId: 'parent-session',
          title: 'Parent',
          dialogTurns: [],
          status: 'idle',
          config: {},
          createdAt: 1,
          lastActiveAt: 1,
          error: null,
        } as Session],
      ]),
      activeSessionId: 'deep-review-child',
    } as FlowChatState;

    globalThis.ResizeObserver = class {
      observe() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    useReviewActionBarStore.getState().reset();
  });

  it('shows the completed Deep Review action bar even when the report has no remediation items', async () => {
    await act(async () => {
      root.render(
        <BtwSessionPanel
          childSessionId="deep-review-child"
          parentSessionId="parent-session"
          workspacePath="D:/workspace/project"
        />,
      );
    });

    expect(useReviewActionBarStore.getState()).toMatchObject({
      childSessionId: 'deep-review-child',
      phase: 'review_completed',
    });
    expect(useReviewActionBarStore.getState().remediationItems).toEqual([]);
  });
});
