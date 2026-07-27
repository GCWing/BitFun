import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FlowTextItem, FlowToolItem, FlowUserSteeringItem, ModelRound, Session } from '../types/flow-chat';

vi.mock('./FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      activeSessionId: null,
      sessions: new Map(),
    }),
  },
}));

vi.mock('../tool-cards/toolCardMetadata', () => ({
  isCollapsibleTool: (toolName: string) => ['Read', 'LS', 'Grep', 'Glob', 'WebSearch', 'Bash', 'Git'].includes(toolName),
  READ_TOOL_NAMES: new Set(['Read']),
  SEARCH_TOOL_NAMES: new Set(['Grep', 'Glob', 'WebSearch']),
  COMMAND_TOOL_NAMES: new Set(['Bash', 'Git']),
}));

import { sessionToVirtualItems, type VirtualItem } from './modernFlowChatStore';

type ModelRoundVirtualItem = Extract<VirtualItem, { type: 'model-round' }>;

function makeTextItem(id: string, content: string): FlowTextItem {
  return {
    id,
    type: 'text',
    content,
    isStreaming: false,
    isMarkdown: true,
    timestamp: 1000,
    status: 'completed',
  };
}

function makeTextItems(count: number, prefix = 'text'): FlowTextItem[] {
  return Array.from({ length: count }, (_, index) =>
    makeTextItem(`${prefix}-${index + 1}`, `Assistant response block ${index + 1}`)
  );
}

function makeReadTool(id: string): FlowToolItem {
  return makeTool(id, 'Read');
}

function makeTool(
  id: string,
  toolName: string,
  status: FlowToolItem['status'] = 'completed',
  endTime?: number,
): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName,
    timestamp: 1001,
    status,
    toolCall: {
      id,
      input: { file_path: 'src/main.rs' },
    },
    ...(status === 'completed'
      ? {
          toolResult: {
            result: 'file contents',
            success: true,
          },
        }
      : {}),
    ...(endTime !== undefined ? { endTime } : {}),
  };
}

function makeSteeringItem(id: string, content = 'Steer now'): FlowUserSteeringItem {
  return {
    id: `steering_${id}`,
    type: 'user-steering',
    steeringId: id,
    content,
    roundIndex: 0,
    timestamp: 1100,
    status: 'pending',
  };
}

function makeRound(overrides: Partial<ModelRound> = {}): ModelRound {
  return {
    id: overrides.id ?? 'round-1',
    index: 0,
    items: overrides.items ?? [
      makeTextItem('text-1', 'I will inspect the file.'),
      makeReadTool('tool-1'),
    ],
    isStreaming: false,
    isComplete: true,
    status: 'completed',
    startTime: 1000,
    ...overrides,
  };
}

function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: overrides.sessionId ?? 'session-1',
    dialogTurns: overrides.dialogTurns ?? [{
      id: 'turn-1',
      sessionId: overrides.sessionId ?? 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Help',
        timestamp: 900,
      },
      modelRounds: [makeRound()],
      status: 'completed',
      startTime: 900,
    }],
    status: 'idle',
    config: overrides.config ?? {},
    createdAt: 800,
    lastActiveAt: 1000,
    error: null,
    ...overrides,
  };
}

describe('sessionToVirtualItems explore grouping', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('groups normal rounds containing only collapsible tools and narrative', () => {
    const session = makeSession({ sessionId: 'normal-session' });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
  });

  it('keeps trailing assistant text visible after collapsible tool history', () => {
    const round = makeRound({
      id: 'round-with-trailing-answer',
      items: [
        makeReadTool('tool-1'),
        makeTextItem('text-final', 'Here is the answer after inspecting the files.'),
      ],
    });
    const session = makeSession({
      sessionId: 'trailing-answer-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'trailing-answer-session',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [round],
        status: 'completed',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'model-round']);
  });

  it('carries turn timing and token metadata into the model round virtual item', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [makeRound({
          id: 'round-with-answer',
          items: [makeTextItem('text-final', 'Here is the answer.')],
        })],
        status: 'completed',
        startTime: 900,
        endTime: 2400,
        tokenUsage: {
          inputTokens: 1200,
          outputTokens: 300,
          totalTokens: 1500,
          timestamp: 2400,
        },
      }],
    });

    const modelItem = sessionToVirtualItems(session)
      .find((item): item is ModelRoundVirtualItem => item.type === 'model-round');

    expect(modelItem).toMatchObject({
      turnStartedAt: 900,
      turnEndedAt: 2400,
      turnDurationMs: 1500,
      turnTokenUsage: {
        inputTokens: 1200,
        outputTokens: 300,
        totalTokens: 1500,
      },
    });
  });

  it('folds consecutive rounds that share the same round group id into retry history', () => {
    const firstRound = makeRound({
      id: 'finalize-round-1',
      roundGroupId: 'finalize-group-1',
      items: [makeTextItem('text-1', 'First finalize answer.')],
    });
    const secondRound = makeRound({
      id: 'finalize-round-2',
      roundGroupId: 'finalize-group-1',
      items: [makeTextItem('text-2', 'Retried finalize answer.')],
    });
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [firstRound, secondRound],
        status: 'completed',
        startTime: 900,
      }],
    });

    const modelRounds = sessionToVirtualItems(session)
      .filter((item): item is ModelRoundVirtualItem => item.type === 'model-round');

    expect(modelRounds).toHaveLength(1);
    expect(modelRounds[0].data.id).toBe('finalize-round-2');
    expect(modelRounds[0].data.historyRounds?.map(round => round.id)).toEqual(['finalize-round-1']);
  });

  it('does not special-case ACP rounds without explicit render hints', () => {
    const session = makeSession({
      sessionId: 'acp-session',
      config: { agentType: 'acp:opencode' },
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
  });

  it('honors explicit round render hints for non-ACP sessions', () => {
    const round = makeRound({
      id: 'round-with-hint',
      renderHints: { disableExploreGrouping: true },
    });
    const session = makeSession({
      sessionId: 'hint-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'hint-session',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [round],
        status: 'completed',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'model-round']);
  });

  it('appends a completion notice for abnormal completed turns', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [makeRound()],
        status: 'completed',
        startTime: 900,
        finishReason: 'interrupted',
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual([
      'user-message',
      'explore-group',
      'turn-completion-notice',
    ]);
    expect(items[2]).toMatchObject({
      type: 'turn-completion-notice',
      data: {
        reasonCode: 'interrupted',
      },
    });
  });

  it('does not append a completion notice for normal completed turns', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [makeRound()],
        status: 'completed',
        startTime: 900,
        finishReason: 'complete',
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
  });

  it('appends a terminal failure notice even when no model round was created', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [],
        status: 'error',
        error: 'OpenAI Streaming API failed after 10 attempts: connection refused',
        errorDetail: {
          category: 'network',
          provider: 'openai',
        },
        startTime: 900,
        endTime: 1200,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'turn-failure-notice']);
    expect(items[1]).toMatchObject({
      type: 'turn-failure-notice',
      data: {
        error: 'OpenAI Streaming API failed after 10 attempts: connection refused',
        errorDetail: {
          category: 'network',
          provider: 'openai',
        },
      },
    });
  });

  it('keeps trailing explore groups expanded while the turn is still processing', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [makeRound({ id: 'round-1', isStreaming: false, isComplete: true })],
        status: 'processing',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
    expect(items[1]).toMatchObject({
      type: 'explore-group',
      data: {
        wasCutByCritical: false,
      },
    });
  });

  it('keeps an active collapsible tool in the same trailing explore group', () => {
    const session = makeSession({
      sessionId: 'active-tool-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'active-tool-session',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [
          makeRound({ id: 'round-1', isStreaming: false, isComplete: true }),
          makeRound({
            id: 'round-2',
            items: [makeTool('tool-2', 'Read', 'running')],
            // Item status is the durable source of truth even if the round's
            // streaming bit arrives one store update late.
            isStreaming: false,
            isComplete: false,
            status: 'streaming',
          }),
        ],
        status: 'processing',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
    expect(items[1]).toMatchObject({
      type: 'explore-group',
      data: {
        groupId: 'round-1',
        isGroupStreaming: true,
        isLastGroupInTurn: true,
        wasCutByCritical: false,
        allItems: [
          expect.objectContaining({ id: 'text-1' }),
          expect.objectContaining({ id: 'tool-1' }),
          expect.objectContaining({ id: 'tool-2', status: 'running' }),
        ],
      },
    });
  });

  it('merges a just-completed collapsible tool immediately, with no wall-clock window', () => {
    // The projection must not depend on Date.now(): a time-dependent
    // classification needs a timer to re-run it, and that timer restructures
    // and remounts the round long after the data settled.
    const makeJustCompletedSession = (streaming: boolean) => makeSession({
      sessionId: `just-completed-tool-session-${streaming ? 'streaming' : 'settled'}`,
      dialogTurns: [{
        id: 'turn-1',
        sessionId: `just-completed-tool-session-${streaming ? 'streaming' : 'settled'}`,
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [
          makeRound({ id: 'round-1', isStreaming: false, isComplete: true }),
          makeRound({
            id: 'round-2',
            items: [makeTool('tool-2', 'Read', 'completed', 10_000)],
            isStreaming: streaming,
            isComplete: !streaming,
            status: streaming ? 'streaming' : 'completed',
          }),
        ],
        status: 'processing',
        startTime: 900,
      }],
    });

    vi.useFakeTimers();

    // 200ms after the tool finished — inside the old 1s transient window.
    vi.setSystemTime(10_200);
    const justFinished = sessionToVirtualItems(makeJustCompletedSession(true));

    expect(justFinished.map(item => item.type)).toEqual(['user-message', 'explore-group']);
    expect(justFinished[1]).toMatchObject({
      type: 'explore-group',
      data: {
        groupId: 'round-1',
        allItems: [
          expect.objectContaining({ id: 'text-1' }),
          expect.objectContaining({ id: 'tool-1' }),
          expect.objectContaining({ id: 'tool-2' }),
        ],
      },
    });

    // Well past the old window: same projection, so no timer can change it.
    vi.setSystemTime(999_999);
    const longSettled = sessionToVirtualItems(makeJustCompletedSession(true));
    expect(longSettled.map(item => item.type)).toEqual(justFinished.map(item => item.type));

    // Streaming vs settled must not change the projection either.
    vi.setSystemTime(10_200);
    const notStreaming = sessionToVirtualItems(makeJustCompletedSession(false));
    expect(notStreaming.map(item => item.type)).toEqual(justFinished.map(item => item.type));
  });

  it('keeps the same explore group id when a completed trailing tool is merged in', () => {
    const baseTurn = {
      id: 'turn-1',
      sessionId: 'stable-group-session',
      userMessage: {
        id: 'user-1',
        content: 'Help',
        timestamp: 900,
      },
      modelRounds: [
        makeRound({ id: 'round-1', isStreaming: false, isComplete: true }),
        makeRound({
          id: 'round-2',
          items: [makeTool('tool-2', 'Read', 'running')],
          isStreaming: true,
          isComplete: false,
          status: 'streaming',
        }),
      ],
      status: 'processing' as const,
      startTime: 900,
    };
    const activeSession = makeSession({
      sessionId: 'stable-group-session',
      dialogTurns: [baseTurn],
    });
    const completedSession = makeSession({
      sessionId: 'stable-group-session-completed',
      dialogTurns: [{
        ...baseTurn,
        sessionId: 'stable-group-session-completed',
        modelRounds: [
          baseTurn.modelRounds[0],
          makeRound({
            id: 'round-2',
            items: [makeTool('tool-2', 'Read', 'completed')],
            isStreaming: false,
            isComplete: true,
            status: 'completed',
          }),
        ],
      }],
    });

    const activeItems = sessionToVirtualItems(activeSession);
    const completedItems = sessionToVirtualItems(completedSession);

    expect(activeItems.map(item => item.type)).toEqual(['user-message', 'explore-group']);
    expect(completedItems.map(item => item.type)).toEqual(activeItems.map(item => item.type));
    expect(activeItems[1]).toMatchObject({
      type: 'explore-group',
      data: {
        groupId: 'round-1',
        allItems: [
          expect.objectContaining({ id: 'text-1' }),
          expect.objectContaining({ id: 'tool-1' }),
          expect.objectContaining({ id: 'tool-2', status: 'running' }),
        ],
      },
    });
    expect(completedItems[1]).toMatchObject({
      type: 'explore-group',
      data: {
        groupId: 'round-1',
        allItems: [
          expect.objectContaining({ id: 'text-1' }),
          expect.objectContaining({ id: 'tool-1' }),
          expect.objectContaining({ id: 'tool-2' }),
        ],
      },
    });
  });

  it('keeps the latest completed trailing explore group expanded', () => {
    const session = makeSession();

    const items = sessionToVirtualItems(session);

    expect(items[1]).toMatchObject({
      type: 'explore-group',
      data: {
        isGroupStreaming: false,
        isLastGroupInTurn: true,
        wasCutByCritical: false,
      },
    });
  });

  it('collapses a completed trailing explore group once a newer turn exists', () => {
    const firstTurn: Session['dialogTurns'][number] = {
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Inspect the file',
        timestamp: 900,
      },
      modelRounds: [makeRound({ id: 'round-1' })],
      status: 'completed',
      startTime: 900,
    };
    const secondTurn: Session['dialogTurns'][number] = {
      id: 'turn-2',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-2',
        content: 'Continue',
        timestamp: 1100,
      },
      modelRounds: [makeRound({ id: 'round-2' })],
      status: 'processing',
      startTime: 1100,
    };

    // Populate the stable-turn projection cache while this is still the tail.
    const initialItems = sessionToVirtualItems(makeSession({
      dialogTurns: [firstTurn],
    }));
    expect(initialItems[1]).toMatchObject({
      type: 'explore-group',
      data: {
        wasCutByCritical: false,
      },
    });

    // Reuse the same immutable turn object, matching the real append path. The
    // cache must account for its new non-tail position.
    const session = makeSession({
      dialogTurns: [
        firstTurn,
        secondTurn,
      ],
    });

    const items = sessionToVirtualItems(session);
    const exploreGroups = items.filter(item => item.type === 'explore-group');

    expect(exploreGroups).toHaveLength(2);
    expect(exploreGroups[0]).toMatchObject({
      turnId: 'turn-1',
      data: {
        isLastGroupInTurn: false,
        wasCutByCritical: true,
      },
    });
    expect(exploreGroups[1]).toMatchObject({
      turnId: 'turn-2',
      data: {
        isLastGroupInTurn: true,
        wasCutByCritical: false,
      },
    });
  });

  it('auto-collapses non-trailing explore groups during an active turn', () => {
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Help',
          timestamp: 900,
        },
        modelRounds: [
          makeRound({ id: 'round-1' }),
          makeRound({
            id: 'round-2',
            items: [makeTool('tool-2', 'TodoWrite')],
          }),
          makeRound({
            id: 'round-3',
            isStreaming: true,
            isComplete: false,
            status: 'streaming',
          }),
        ],
        status: 'processing',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);
    const exploreGroups = items.filter(item => item.type === 'explore-group');

    expect(exploreGroups).toHaveLength(2);
    expect(exploreGroups[0]).toMatchObject({
      data: {
        isLastGroupInTurn: false,
        wasCutByCritical: true,
      },
    });
    expect(exploreGroups[1]).toMatchObject({
      data: {
        isLastGroupInTurn: true,
        wasCutByCritical: false,
      },
    });
  });

  it('renders user steering as a top-level user message item', () => {
    const steeringItem = makeSteeringItem('steer-1', 'Handle this queued request now');
    const session = makeSession({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: {
          id: 'user-1',
          content: 'Initial request',
          timestamp: 900,
        },
        modelRounds: [
          makeRound({ id: 'round-1' }),
          makeRound({
            id: 'round-2',
            items: [steeringItem],
            isStreaming: true,
            isComplete: false,
            status: 'streaming',
          }),
        ],
        status: 'processing',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual([
      'user-message',
      'explore-group',
      'user-steering-message',
    ]);
    expect(items[2]).toMatchObject({
      type: 'user-steering-message',
      data: {
        id: 'user_steering_steer-1',
        content: 'Handle this queued request now',
        timestamp: 1100,
      },
      turnId: 'turn-1',
      steeringId: 'steer-1',
      steeringStatus: 'pending',
    });
  });

  it('never splits a model round into segments (one round = one stable virtual item)', () => {
    const largeRound = makeRound({
      id: 'large-round',
      items: makeTextItems(25, 'large-text'),
      isStreaming: false,
      isComplete: true,
      status: 'completed',
    });
    const trailingRound = makeRound({
      id: 'tail-round',
      items: makeTextItems(2, 'tail-text'),
      isStreaming: false,
      isComplete: true,
      status: 'completed',
    });
    const session = makeSession({
      sessionId: 'large-round-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'large-round-session',
        userMessage: {
          id: 'user-1',
          content: 'Summarize a large trace',
          timestamp: 900,
        },
        modelRounds: [largeRound, trailingRound],
        status: 'completed',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);
    const modelItems = items.filter((item): item is ModelRoundVirtualItem => item.type === 'model-round');

    expect(modelItems.map(item => item.data.id)).toEqual(['large-round', 'tail-round']);
    expect(modelItems[0].data.items).toHaveLength(25);
    expect(modelItems[0].isLastRound).toBe(false);
    expect(modelItems[1].isLastRound).toBe(true);
  });

  it('does not split the turn-tail large round (avoids completion remount flash)', () => {
    const largeRound = makeRound({
      id: 'large-tail-round',
      items: makeTextItems(25, 'large-tail-text'),
      isStreaming: false,
      isComplete: true,
      status: 'completed',
    });
    const session = makeSession({
      sessionId: 'large-tail-round-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'large-tail-round-session',
        userMessage: {
          id: 'user-1',
          content: 'Summarize a large trace',
          timestamp: 900,
        },
        modelRounds: [largeRound],
        status: 'completed',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);
    const modelItems = items.filter((item): item is ModelRoundVirtualItem => item.type === 'model-round');

    expect(modelItems).toHaveLength(1);
    expect(modelItems[0]).toMatchObject({
      data: { id: 'large-tail-round' },
      isLastRound: true,
    });
    expect(modelItems[0].data.items).toHaveLength(25);
  });

  it('does not split active or streaming large model rounds', () => {
    const streamingRound = makeRound({
      id: 'streaming-large-round',
      items: makeTextItems(25, 'streaming-text'),
      isStreaming: true,
      isComplete: false,
      status: 'streaming',
    });
    const session = makeSession({
      sessionId: 'streaming-large-round-session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'streaming-large-round-session',
        userMessage: {
          id: 'user-1',
          content: 'Continue writing',
          timestamp: 900,
        },
        modelRounds: [streamingRound],
        status: 'processing',
        startTime: 900,
      }],
    });

    const items = sessionToVirtualItems(session);
    const modelItems = items.filter(item => item.type === 'model-round');

    expect(items.map(item => item.type)).toEqual(['user-message', 'model-round']);
    expect(modelItems).toHaveLength(1);
    expect(modelItems[0]).toMatchObject({
      type: 'model-round',
      data: {
        id: 'streaming-large-round',
        items: expect.arrayContaining([
          expect.objectContaining({ id: 'streaming-text-1' }),
          expect.objectContaining({ id: 'streaming-text-25' }),
        ]),
      },
      isLastRound: true,
      isTurnComplete: false,
    });
  });

  it('reuses the projection for completed turns when a later active turn changes', () => {
    const completedTurn = {
      id: 'completed-turn',
      sessionId: 'stable-turn-session',
      userMessage: {
        id: 'user-completed',
        content: 'Loaded prompt',
        timestamp: 900,
      },
      modelRounds: [makeRound({ id: 'completed-round' })],
      status: 'completed' as const,
      startTime: 900,
    };
    const activeTurnBase = {
      id: 'active-turn',
      sessionId: 'stable-turn-session',
      userMessage: {
        id: 'user-active',
        content: 'Continue',
        timestamp: 1000,
      },
      status: 'processing' as const,
      startTime: 1000,
    };
    const firstSession = makeSession({
      sessionId: 'stable-turn-session',
      dialogTurns: [
        completedTurn,
        {
          ...activeTurnBase,
          modelRounds: [
            makeRound({
              id: 'active-round-1',
              isStreaming: true,
              isComplete: false,
              status: 'streaming',
              items: [makeTool('active-tool-1', 'TodoWrite', 'running')],
            }),
          ],
        },
      ],
    });
    const secondSession = makeSession({
      sessionId: 'stable-turn-session',
      dialogTurns: [
        completedTurn,
        {
          ...activeTurnBase,
          modelRounds: [
            makeRound({
              id: 'active-round-2',
              isStreaming: true,
              isComplete: false,
              status: 'streaming',
              items: [makeTool('active-tool-2', 'TodoWrite', 'running')],
            }),
          ],
        },
      ],
    });

    const firstItems = sessionToVirtualItems(firstSession);
    const secondItems = sessionToVirtualItems(secondSession);

    expect(secondItems[0]).toBe(firstItems[0]);
    expect(secondItems[1]).toBe(firstItems[1]);
    expect(secondItems[2]).not.toBe(firstItems[2]);
  });
});
