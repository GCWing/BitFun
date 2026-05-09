import { describe, expect, it, vi } from 'vitest';
import type { FlowTextItem, FlowToolItem, FlowUserSteeringItem, ModelRound, Session } from '../types/flow-chat';

vi.mock('./FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      activeSessionId: null,
      sessions: new Map(),
    }),
  },
}));

vi.mock('../tool-cards', () => ({
  isCollapsibleTool: (toolName: string) => ['Read', 'LS', 'Grep', 'Glob', 'WebSearch', 'Bash'].includes(toolName),
  READ_TOOL_NAMES: new Set(['Read']),
  SEARCH_TOOL_NAMES: new Set(['Grep', 'Glob', 'WebSearch']),
  COMMAND_TOOL_NAMES: new Set(['Bash']),
}));

import { sessionToVirtualItems } from './modernFlowChatStore';

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

function makeReadTool(id: string): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Read',
    timestamp: 1001,
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
  it('groups normal rounds containing only collapsible tools and narrative', () => {
    const session = makeSession({ sessionId: 'normal-session' });

    const items = sessionToVirtualItems(session);

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
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

  it('does not render a stopped indicator for non-complete finish reasons', () => {
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

    expect(items.map(item => item.type)).toEqual(['user-message', 'explore-group']);
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
});
