import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  globalEventBus,
  PERMISSION_REQUEST_NOTIFICATION_EVENT,
  type PermissionRequestNotificationEvent,
} from '@/infrastructure/event-bus';
import { FlowChatStore } from '../../store/FlowChatStore';
import type { FlowToolItem, Session } from '../../types/flow-chat';
import {
  applyPendingAcpPermissionForTool,
  handleAcpPermissionRequestForToolCard,
} from './AcpPermissionToolCardModule';

describe('ACP permission notification bridge', () => {
  const received: PermissionRequestNotificationEvent[] = [];
  let unlisten: (() => void) | undefined;

  beforeEach(() => {
    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map(),
      activeSessionId: null,
    }));
    unlisten = globalEventBus.on<PermissionRequestNotificationEvent>(
      PERMISSION_REQUEST_NOTIFICATION_EVENT,
      (event) => received.push(event),
    );
  });

  afterEach(() => {
    received.length = 0;
    unlisten?.();
  });

  it('emits after an ACP request is applied to its matching tool card', () => {
    addToolCard('tool-1');

    expect(handleAcpPermissionRequestForToolCard({
      permissionId: 'permission-1',
      sessionId: 'acp-session-1',
      toolCall: { toolCallId: 'tool-1' },
    })).toBe(true);

    expect(received).toEqual([{
      requestId: 'permission-1',
      sessionId: 'session-1',
      roundId: 'round-1',
    }]);
  });

  it('waits to emit until a deferred ACP request has a matching tool card', () => {
    expect(handleAcpPermissionRequestForToolCard({
      permissionId: 'permission-2',
      sessionId: 'acp-session-2',
      toolCall: { toolCallId: 'tool-2' },
    })).toBe(true);
    expect(received).toEqual([]);

    addToolCard('tool-2');
    applyPendingAcpPermissionForTool(FlowChatStore.getInstance(), 'tool-2');

    expect(received).toEqual([{
      requestId: 'permission-2',
      sessionId: 'session-1',
      roundId: 'round-1',
    }]);
  });
});

function addToolCard(toolId: string): void {
  const tool: FlowToolItem = {
    id: toolId,
    type: 'tool',
    toolName: 'Tool',
    timestamp: 1000,
    status: 'streaming',
    toolCall: { id: toolId, input: {} },
  };

  FlowChatStore.getInstance().setState(() => ({
    sessions: new Map([['session-1', {
      sessionId: 'session-1',
      title: 'Session',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'session-1',
        userMessage: { id: 'user-1', content: 'Request', timestamp: 900 },
        modelRounds: [{
          id: 'round-1',
          index: 0,
          items: [tool],
          isStreaming: true,
          isComplete: false,
          status: 'streaming',
          startTime: 1000,
        }],
        status: 'processing',
        startTime: 900,
      }],
      status: 'idle',
      config: { agentType: 'agentic' },
      createdAt: 800,
      lastActiveAt: 1000,
      error: null,
    } as Session]]),
    activeSessionId: 'session-1',
  }));
}
