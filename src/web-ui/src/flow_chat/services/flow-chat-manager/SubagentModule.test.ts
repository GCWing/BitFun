import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DialogTurn, FlowTextItem, FlowToolItem, ModelRound } from '../../types/flow-chat';

vi.mock('./ToolEventModule', () => ({
  processToolEvent: vi.fn(),
}));

const testStoreState = vi.hoisted(() => ({
  sessions: new Map<string, any>(),
}));

const mockStore = vi.hoisted(() => ({
  getState: () => ({
    sessions: testStoreState.sessions,
    activeSessionId: null,
  }),
  findToolItem: (sessionId: string, dialogTurnId: string, toolUseId: string) => {
    const session = testStoreState.sessions.get(sessionId);
    const turn = session?.dialogTurns.find((candidate: any) => candidate.id === dialogTurnId);
    for (const round of turn?.modelRounds ?? []) {
      const item = round.items.find((candidate: any) => candidate.id === toolUseId);
      if (item) return item;
    }
    return null;
  },
  insertModelRoundItemAfterTool: (
    sessionId: string,
    dialogTurnId: string,
    parentToolId: string,
    newItem: any,
  ) => {
    const session = testStoreState.sessions.get(sessionId);
    const turn = session?.dialogTurns.find((candidate: any) => candidate.id === dialogTurnId);
    const round = turn?.modelRounds.find((candidate: any) =>
      candidate.items.some((item: any) => item.id === parentToolId),
    );
    if (!round) return;

    const existingIndex = round.items.findIndex((item: any) => item.id === newItem.id);
    if (existingIndex !== -1) return;

    const parentIndex = round.items.findIndex((item: any) => item.id === parentToolId);
    round.items.splice(parentIndex + 1, 0, newItem);
  },
  updateModelRoundItem: (
    sessionId: string,
    dialogTurnId: string,
    itemId: string,
    updates: Record<string, unknown>,
  ) => {
    const session = testStoreState.sessions.get(sessionId);
    const turn = session?.dialogTurns.find((candidate: any) => candidate.id === dialogTurnId);
    for (const round of turn?.modelRounds ?? []) {
      const item = round.items.find((candidate: any) => candidate.id === itemId);
      if (item) {
        Object.assign(item, updates);
        return;
      }
    }
  },
}));

vi.mock('../../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => mockStore,
  },
}));

import {
  routeModelRoundStartedToToolCard,
  routeTextChunkToToolCard,
} from './SubagentModule';

const parentSessionId = 'parent-session';
const parentTurnId = 'parent-turn';
const parentToolId = 'parent-task-tool';
const parentRoundId = 'parent-round';
const subagentSessionId = 'subagent-session';
const subagentTurnId = 'subagent-turn';
const subagentRoundId = 'subagent-round';

const context = { flowChatStore: mockStore } as any;

function resetStore(): void {
  testStoreState.sessions.clear();
}

function seedParentTaskTool(): void {
  resetStore();

  const parentTool: FlowToolItem = {
    id: parentToolId,
    type: 'tool',
    toolName: 'Task',
    toolCall: {
      id: parentToolId,
      input: { prompt: 'Inspect this area' },
    },
    timestamp: 1000,
    status: 'running',
    requiresConfirmation: false,
  };

  const modelRound: ModelRound = {
    id: parentRoundId,
    index: 0,
    items: [parentTool],
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: 1000,
  };

  const dialogTurn: DialogTurn = {
    id: parentTurnId,
    sessionId: parentSessionId,
    userMessage: {
      id: 'user-message',
      content: 'Start three reviewers',
      timestamp: 900,
    },
    modelRounds: [modelRound],
    status: 'processing',
    startTime: 900,
  };

  testStoreState.sessions.set(parentSessionId, {
    sessionId: parentSessionId,
    dialogTurns: [dialogTurn],
  });
}

function getParentRoundTextItem(itemId: string): FlowTextItem | undefined {
  const parentSession = testStoreState.sessions.get(parentSessionId);
  return parentSession?.dialogTurns[0]?.modelRounds[0]?.items.find(
    (item): item is FlowTextItem => item.id === itemId && item.type === 'text',
  );
}

describe('SubagentModule', () => {
  afterEach(() => {
    resetStore();
  });

  it('creates placeholder on model round start then updates from text chunk', () => {
    seedParentTaskTool();

    const itemId = `subagent-text-${parentToolId}-${subagentSessionId}-${subagentRoundId}`;

    // ModelRoundStarted creates a placeholder immediately.
    routeModelRoundStartedToToolCard(context, parentSessionId, parentToolId, {
      sessionId: subagentSessionId,
      turnId: subagentTurnId,
      roundId: subagentRoundId,
    });

    const placeholder = getParentRoundTextItem(itemId);
    expect(placeholder).toBeDefined();
    expect(placeholder?.content).toBe('\u200B');
    expect(placeholder?.status).toBe('streaming');
    expect(placeholder?.isStreaming).toBe(true);
    expect(placeholder?.isSubagentItem).toBe(true);
    expect(placeholder?.parentTaskToolId).toBe(parentToolId);
    expect(placeholder?.subagentSessionId).toBe(subagentSessionId);

    // First text chunk updates the placeholder.
    routeTextChunkToToolCard(context, parentSessionId, parentToolId, {
      sessionId: subagentSessionId,
      turnId: subagentTurnId,
      roundId: subagentRoundId,
      text: 'Review started.',
      contentType: 'text',
    });

    const item = getParentRoundTextItem(itemId);
    expect(item?.content).toBe('Review started.');
    expect(item?.status).toBe('streaming');
    expect(item?.isStreaming).toBe(true);
    expect(item?.isSubagentItem).toBe(true);
    expect(item?.parentTaskToolId).toBe(parentToolId);
    expect(item?.subagentSessionId).toBe(subagentSessionId);
  });
});
