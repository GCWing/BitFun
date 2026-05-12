// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { BtwSessionPanel } from './BtwSessionPanel';
import { useReviewActionBarStore } from '../../store/deepReviewActionBarStore';
import { loadPersistedReviewState } from '../../services/ReviewActionBarPersistenceService';
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

function createCompletedDeepReviewWithoutResult(): Session {
  const childSession = createReviewSession();
  return {
    ...childSession,
    dialogTurns: childSession.dialogTurns.map((turn) => ({
      ...turn,
      modelRounds: turn.modelRounds.map((round) => ({
        ...round,
        items: [{
          id: 'reviewer-task',
          type: 'tool',
          timestamp: 2,
          status: 'completed',
          toolName: 'Task',
          toolCall: {
            id: 'task-security',
            input: { subagent_type: 'ReviewSecurity' },
          },
          toolResult: {
            success: true,
            result: {
              summary: {
                overall_assessment: 'Security reviewer found no blockers.',
              },
            },
          },
        }],
      })),
    })),
  } as Session;
}

function createInterruptedDeepReviewWithoutResult(): Session {
  const childSession = createCompletedDeepReviewWithoutResult();
  return {
    ...childSession,
    status: 'error',
    error: 'previous execution failed',
    dialogTurns: childSession.dialogTurns.map((turn) => ({
      ...turn,
      status: 'error',
      error: 'previous execution failed',
    })),
  } as Session;
}

function createRunningDeepReviewSession(): Session {
  const childSession = createCompletedDeepReviewWithoutResult();
  return {
    ...childSession,
    status: 'running',
    dialogTurns: childSession.dialogTurns.map((turn) => ({
      ...turn,
      status: 'processing',
      modelRounds: turn.modelRounds.map((round) => ({
        ...round,
        isStreaming: true,
        isComplete: false,
        status: 'streaming',
      })),
    })),
  } as Session;
}

function createPendingDeepReviewSession(): Session {
  const childSession = createRunningDeepReviewSession();
  return {
    ...childSession,
    dialogTurns: childSession.dialogTurns.map((turn) => ({
      ...turn,
      status: 'pending',
    })),
  } as Session;
}

function createParentSessionWithId(sessionId: string): Session {
  return {
    sessionId,
    title: sessionId,
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
  } as Session;
}

function cloneReviewSessionWithId(
  session: Session,
  sessionId: string,
  parentSessionId: string,
): Session {
  return {
    ...session,
    sessionId,
    parentSessionId,
    title: sessionId,
    dialogTurns: session.dialogTurns.map((turn, turnIndex) => ({
      ...turn,
      id: `${sessionId}-turn-${turnIndex + 1}`,
      sessionId,
      userMessage: turn.userMessage
        ? {
            ...turn.userMessage,
            id: `${sessionId}-user-${turnIndex + 1}`,
          }
        : undefined,
      modelRounds: turn.modelRounds.map((round, roundIndex) => ({
        ...round,
        id: `${sessionId}-round-${turnIndex + 1}-${roundIndex + 1}`,
        items: round.items.map((item, itemIndex) => ({
          ...item,
          id: `${sessionId}-item-${turnIndex + 1}-${roundIndex + 1}-${itemIndex + 1}`,
        })),
      })),
    })),
  } as Session;
}

function createCancelledResumeDeepReview(): Session {
  const childSession = createInterruptedDeepReviewWithoutResult();
  return {
    ...childSession,
    status: 'idle',
    error: null,
    dialogTurns: [
      ...childSession.dialogTurns,
      {
        id: 'turn-2',
        sessionId: 'deep-review-child',
        userMessage: {
          id: 'user-2',
          content: 'Continue interrupted Deep Review',
          timestamp: 2,
        },
        modelRounds: [],
        status: 'cancelled',
        startTime: 2,
        timestamp: 2,
      },
    ],
  } as Session;
}

function createCancelledFixDeepReview(): Session {
  const childSession = createReviewSession();
  return {
    ...childSession,
    status: 'idle',
    error: null,
    dialogTurns: [
      ...childSession.dialogTurns,
      {
        id: 'fix-turn-1',
        sessionId: 'deep-review-child',
        userMessage: {
          id: 'fix-user-1',
          content: 'Fix review findings',
          timestamp: 3,
        },
        modelRounds: [],
        status: 'cancelled',
        startTime: 3,
        timestamp: 3,
      },
    ],
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

  it('shows the running review action as minimized while Deep Review is still processing', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createRunningDeepReviewSession()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

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
      phase: 'review_running',
      minimized: true,
    });
  });

  it('shows the running review action as minimized while Deep Review is pending', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createPendingDeepReviewSession()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

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
      phase: 'review_running',
      minimized: true,
    });
  });

  it('keeps minimized running review action bars isolated across simultaneous reviews', async () => {
    const firstParent = createParentSessionWithId('parent-session-1');
    const secondParent = createParentSessionWithId('parent-session-2');
    const firstChild = cloneReviewSessionWithId(
      createRunningDeepReviewSession(),
      'deep-review-child-1',
      firstParent.sessionId,
    );
    const secondChild = cloneReviewSessionWithId(
      createRunningDeepReviewSession(),
      'deep-review-child-2',
      secondParent.sessionId,
    );

    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        [firstParent.sessionId, firstParent],
        [secondParent.sessionId, secondParent],
        [firstChild.sessionId, firstChild],
        [secondChild.sessionId, secondChild],
      ]),
      activeSessionId: firstChild.sessionId,
    } as FlowChatState;

    await act(async () => {
      root.render(
        <>
          <BtwSessionPanel
            childSessionId={firstChild.sessionId}
            parentSessionId={firstParent.sessionId}
            workspacePath="D:/workspace/project"
          />
          <BtwSessionPanel
            childSessionId={secondChild.sessionId}
            parentSessionId={secondParent.sessionId}
            workspacePath="D:/workspace/project"
          />
        </>,
      );
    });

    expect(container.querySelectorAll('.btw-session-panel__minimized-button')).toHaveLength(2);
  });

  it('keeps bottom breathing room when the review action is minimized', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createRunningDeepReviewSession()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    await act(async () => {
      root.render(
        <BtwSessionPanel
          childSessionId="deep-review-child"
          parentSessionId="parent-session"
          workspacePath="D:/workspace/project"
        />,
      );
    });

    const body = container.querySelector<HTMLElement>('.btw-session-panel__body');
    expect(body?.style.paddingBottom).toBe('96px');
  });

  it('restores the minimized running action when capacity waiting ends before the review finishes', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createRunningDeepReviewSession()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    await act(async () => {
      root.render(
        <BtwSessionPanel
          childSessionId="deep-review-child"
          parentSessionId="parent-session"
          workspacePath="D:/workspace/project"
        />,
      );
    });

    await act(async () => {
      useReviewActionBarStore.getState().showCapacityQueueBar({
        childSessionId: 'deep-review-child',
        parentSessionId: 'parent-session',
        capacityQueueState: {
          toolId: 'task-security',
          subagentType: 'ReviewSecurity',
          status: 'queued_for_capacity',
          queuedReviewerCount: 1,
          waitingReviewers: [{
            toolId: 'task-security',
            subagentType: 'ReviewSecurity',
            status: 'queued_for_capacity',
          }],
        },
      });
    });

    expect(useReviewActionBarStore.getState()).toMatchObject({
      childSessionId: 'deep-review-child',
      phase: 'review_waiting_capacity',
      minimized: false,
    });

    await act(async () => {
      useReviewActionBarStore.getState().applyCapacityQueueState({
        toolId: 'task-security',
        subagentType: 'ReviewSecurity',
        status: 'running',
        queuedReviewerCount: 0,
        waitingReviewers: [],
      });
      await Promise.resolve();
    });

    expect(useReviewActionBarStore.getState()).toMatchObject({
      childSessionId: 'deep-review-child',
      phase: 'review_running',
      minimized: true,
    });
    expect(container.querySelector('.btw-session-panel__minimized-button')).toBeTruthy();
  });

  it('lets persisted action state replace the running review placeholder', async () => {
    vi.mocked(loadPersistedReviewState).mockResolvedValueOnce({
      version: 1,
      phase: 'fix_running',
      completedRemediationIds: [],
      minimized: true,
      customInstructions: 'Keep the fix focused.',
      persistedAt: 2,
    });
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createRunningDeepReviewSession()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    await act(async () => {
      root.render(
        <BtwSessionPanel
          childSessionId="deep-review-child"
          parentSessionId="parent-session"
          workspacePath="D:/workspace/project"
        />,
      );
      await Promise.resolve();
    });

    expect(useReviewActionBarStore.getState()).toMatchObject({
      childSessionId: 'deep-review-child',
      phase: 'fix_running',
      minimized: true,
      customInstructions: 'Keep the fix focused.',
    });
  });

  it('shows a resumable Deep Review action bar when the run completed without a structured report', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createCompletedDeepReviewWithoutResult()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

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
      phase: 'review_interrupted',
      interruption: expect.objectContaining({
        canResume: true,
        resultRecoveryReason: 'missing_submit_code_review',
      }),
    });
  });

  it('does not restore a stale interruption while a resume request is starting', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createInterruptedDeepReviewWithoutResult()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    const store = useReviewActionBarStore.getState();
    store.showInterruptedActionBar({
      childSessionId: 'deep-review-child',
      parentSessionId: 'parent-session',
      interruption: {
        phase: 'review_interrupted',
        childSessionId: 'deep-review-child',
        parentSessionId: 'parent-session',
        originalTarget: '/DeepReview review latest commit',
        errorDetail: { category: 'unknown', rawMessage: 'previous execution failed' },
        canResume: true,
        recommendedActions: [],
        reviewers: [],
      },
    });
    store.setActiveAction('resume', { baselineTurnId: 'turn-1' });
    store.updatePhase('resume_running');
    store.minimize();

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
      phase: 'resume_running',
      minimized: true,
    });
  });

  it('marks a stopped fix run as interrupted and restores the action bar state', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createCancelledFixDeepReview()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    const store = useReviewActionBarStore.getState();
    store.showActionBar({
      childSessionId: 'deep-review-child',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      reviewMode: 'deep',
      phase: 'review_completed',
    });
    const itemId = useReviewActionBarStore.getState().remediationItems[0]?.id;
    expect(itemId).toBeTruthy();
    store.setSelectedRemediationIds(new Set([itemId!]));
    store.setActiveAction('fix', { baselineTurnId: 'turn-1' });
    store.updatePhase('fix_running');
    store.minimize();

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
      phase: 'fix_interrupted',
      minimized: false,
      remainingFixIds: [itemId],
    });
  });

  it('restores the interrupted action bar when a resumed Deep Review is cancelled by the user', async () => {
    flowChatState = {
      ...flowChatState,
      sessions: new Map([
        ['deep-review-child', createCancelledResumeDeepReview()],
        ['parent-session', flowChatState.sessions.get('parent-session')!],
      ]),
    } as FlowChatState;

    const store = useReviewActionBarStore.getState();
    store.showInterruptedActionBar({
      childSessionId: 'deep-review-child',
      parentSessionId: 'parent-session',
      interruption: {
        phase: 'review_interrupted',
        childSessionId: 'deep-review-child',
        parentSessionId: 'parent-session',
        originalTarget: '/DeepReview review latest commit',
        errorDetail: { category: 'unknown', rawMessage: 'previous execution failed' },
        canResume: true,
        recommendedActions: [],
        reviewers: [],
      },
    });
    store.setActiveAction('resume', { baselineTurnId: 'turn-1' });
    store.updatePhase('resume_running');
    store.minimize();

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
      phase: 'review_interrupted',
      minimized: false,
      interruption: expect.objectContaining({
        interruptionReason: 'manual_cancelled',
      }),
    });
  });
});
