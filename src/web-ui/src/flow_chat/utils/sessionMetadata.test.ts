import { describe, expect, it, vi } from 'vitest';
import type { Session } from '../types/flow-chat';
import type { SessionMetadata } from '@/shared/types/session-history';

vi.mock('@/infrastructure/i18n/core/I18nService', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

import {
  buildSessionMetadata,
  deriveLastFinishedAtFromMetadata,
  deriveSessionRelationshipFromMetadata,
  normalizeSessionRelationship,
  resolveSessionRelationship,
} from './sessionMetadata';

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Session Title',
    titleStatus: 'generated',
    dialogTurns: [],
    status: 'idle',
    config: {
      modelName: 'gpt-test',
      agentType: 'agentic',
    },
    createdAt: 1000,
    lastActiveAt: 1000,
    error: null,
    todos: [],
    maxContextTokens: 128128,
    mode: 'agentic',
    workspacePath: '/workspace',
    parentSessionId: undefined,
    sessionKind: 'normal',
    lastFinishedAt: undefined,
    btwThreads: [],
    btwOrigin: undefined,
    ...overrides,
  };
}

describe('sessionMetadata', () => {
  it('normalizes runtime sessions to an explicit normal kind', () => {
    expect(normalizeSessionRelationship({})).toEqual({
      sessionKind: 'normal',
      parentSessionId: undefined,
      btwOrigin: undefined,
    });
  });

  it('builds btw metadata without dropping existing fields', () => {
    const session = createSession({
      sessionId: 'child-1',
      title: 'BTW Child',
      sessionKind: 'btw',
      parentSessionId: 'parent-1',
      btwOrigin: {
        requestId: 'req-1',
        parentSessionId: 'parent-1',
        parentDialogTurnId: 'turn-9',
        parentTurnIndex: 9,
      },
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'child-1',
          userMessage: {
            id: 'user-1',
            content: 'question',
            timestamp: 1,
          },
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              items: [
                {
                  id: 'text-1',
                  type: 'text',
                  content: 'answer',
                  isStreaming: false,
                  timestamp: 2,
                  status: 'completed',
                },
              ],
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              startTime: 1,
              endTime: 2,
            },
          ],
          status: 'completed',
          startTime: 1,
          endTime: 2,
        },
      ],
    });

    const existingMetadata: SessionMetadata = {
      sessionId: 'child-1',
      sessionName: 'Old Name',
      agentType: 'agentic',
      modelName: 'old-model',
      createdAt: 10,
      lastActiveAt: 10,
      turnCount: 99,
      messageCount: 99,
      toolCallCount: 99,
      status: 'active',
      snapshotSessionId: 'snapshot-1',
      tags: ['keep-me'],
      customMetadata: {
        unrelated: 'preserved',
      },
      todos: [],
      workspacePath: '/workspace',
    };

    const metadata = buildSessionMetadata(session, existingMetadata);

    expect(metadata.snapshotSessionId).toBe('snapshot-1');
    expect(metadata.tags).toEqual(['keep-me', 'btw']);
    expect(metadata.turnCount).toBe(99);
    expect(metadata.messageCount).toBe(99);
    expect(metadata.toolCallCount).toBe(99);
    expect(metadata.customMetadata).toEqual({
      unrelated: 'preserved',
      kind: 'btw',
      parentSessionId: 'parent-1',
      parentRequestId: 'req-1',
      parentDialogTurnId: 'turn-9',
      parentTurnIndex: 9,
      lastFinishedAt: null,
    });
  });

  it('writes normal metadata explicitly and removes stale btw linkage', () => {
    const session = createSession({
      sessionKind: 'normal',
      parentSessionId: undefined,
      btwOrigin: undefined,
    });

    const metadata = buildSessionMetadata(session, {
      sessionId: 'session-1',
      sessionName: 'Session Title',
      agentType: 'agentic',
      modelName: 'gpt-test',
      createdAt: 1000,
      lastActiveAt: 1000,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: ['btw'],
      customMetadata: {
        unrelated: 'preserved',
        kind: 'btw',
        parentSessionId: 'stale-parent',
        parentRequestId: 'stale-request',
      },
      todos: [],
      workspacePath: '/workspace',
    });

    expect(metadata.customMetadata).toEqual({
      unrelated: 'preserved',
      kind: 'normal',
      lastFinishedAt: null,
    });
  });

  it('persists and restores lastFinishedAt without dropping unrelated metadata', () => {
    const session = createSession({
      lastFinishedAt: 4321,
    });

    const metadata = buildSessionMetadata(session, {
      sessionId: 'session-1',
      sessionName: 'Session Title',
      agentType: 'agentic',
      modelName: 'gpt-test',
      createdAt: 1000,
      lastActiveAt: 1000,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: {
        unrelated: 'preserved',
      },
      todos: [],
      workspacePath: '/workspace',
    });

    expect(metadata.customMetadata).toEqual({
      unrelated: 'preserved',
      kind: 'normal',
      lastFinishedAt: 4321,
    });
    expect(deriveLastFinishedAtFromMetadata(metadata)).toBe(4321);
  });

  it('persists locale-aware default title metadata before the first message', () => {
    const session = createSession({
      title: 'flow-chat:session.newCodeWithIndex',
      titleSource: 'i18n',
      titleI18nKey: 'flow-chat:session.newCodeWithIndex',
      titleI18nParams: { count: 2 },
      titleStatus: undefined,
    });

    const metadata = buildSessionMetadata(session);

    expect(metadata.sessionName).toBe('flow-chat:session.newCodeWithIndex');
    expect(metadata.customMetadata).toEqual({
      kind: 'normal',
      lastFinishedAt: null,
      titleSource: 'i18n',
      titleKey: 'flow-chat:session.newCodeWithIndex',
      titleParams: { count: 2 },
    });
  });

  it('treats persisted btw identity as legacy and no longer restores it', () => {
    const metadata: SessionMetadata = {
      sessionId: 'child-1',
      sessionName: 'BTW Child',
      agentType: 'agentic',
      modelName: 'gpt-test',
      createdAt: 1000,
      lastActiveAt: 1001,
      turnCount: 1,
      messageCount: 2,
      toolCallCount: 0,
      status: 'active',
      tags: ['btw'],
      customMetadata: {
        kind: 'btw',
        parentSessionId: 'parent-1',
        parentRequestId: 'req-1',
        parentDialogTurnId: 'turn-2',
        parentTurnIndex: 2,
      },
      todos: [],
      workspacePath: '/workspace',
    };

    const relationship = deriveSessionRelationshipFromMetadata(metadata);
    const resolved = resolveSessionRelationship(relationship);

    expect(relationship).toEqual({
      sessionKind: 'normal',
      parentSessionId: undefined,
      btwOrigin: undefined,
    });
    expect(resolved).toEqual({
      kind: 'normal',
      isBtw: false,
      isReview: false,
      isDeepReview: false,
      parentSessionId: undefined,
      displayAsChild: false,
      canOpenInAuxPane: false,
      origin: undefined,
    });
  });

  it('round-trips review child identity without treating it as a side question', () => {
    const session = createSession({
      sessionId: 'review-child-1',
      title: 'Code review',
      sessionKind: 'review',
      parentSessionId: 'parent-1',
      btwOrigin: {
        requestId: 'review-req-1',
        parentSessionId: 'parent-1',
        parentDialogTurnId: 'turn-3',
        parentTurnIndex: 3,
      },
    });

    const metadata = buildSessionMetadata(session, {
      sessionId: 'review-child-1',
      sessionName: 'Code review',
      agentType: 'CodeReview',
      modelName: 'gpt-test',
      createdAt: 1000,
      lastActiveAt: 1001,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: {},
      todos: [],
      workspacePath: '/workspace',
    });

    expect(metadata.tags).toEqual(['review']);
    expect(metadata.customMetadata).toMatchObject({
      kind: 'review',
      parentSessionId: 'parent-1',
      parentRequestId: 'review-req-1',
      parentDialogTurnId: 'turn-3',
      parentTurnIndex: 3,
    });

    const relationship = deriveSessionRelationshipFromMetadata(metadata);
    const resolved = resolveSessionRelationship(relationship);

    expect(resolved).toMatchObject({
      kind: 'review',
      isBtw: false,
      isReview: true,
      isDeepReview: false,
      parentSessionId: 'parent-1',
      displayAsChild: true,
      canOpenInAuxPane: true,
    });
  });

  it('round-trips deep review child identity as a review session', () => {
    const relationship = normalizeSessionRelationship({
      sessionKind: 'deep_review',
      parentSessionId: 'parent-1',
      btwOrigin: {
        requestId: 'deep-review-req-1',
        parentSessionId: 'parent-1',
      },
    });

    expect(relationship).toEqual({
      sessionKind: 'deep_review',
      parentSessionId: 'parent-1',
      btwOrigin: {
        requestId: 'deep-review-req-1',
        parentSessionId: 'parent-1',
        parentDialogTurnId: undefined,
        parentTurnIndex: undefined,
      },
    });

    expect(resolveSessionRelationship(relationship)).toMatchObject({
      kind: 'deep_review',
      isBtw: false,
      isReview: true,
      isDeepReview: true,
      displayAsChild: true,
      canOpenInAuxPane: true,
    });
  });

  it('persists the Deep Review run manifest from the runtime session', () => {
    const runManifest = {
      reviewMode: 'deep',
      skippedReviewers: [
        {
          subagentId: 'ReviewFrontend',
          displayName: 'Frontend Reviewer',
          reason: 'not_applicable',
        },
      ],
    };
    const session = createSession({
      sessionKind: 'deep_review',
      deepReviewRunManifest: runManifest,
    } as Partial<Session>);

    const metadata = buildSessionMetadata(session);

    expect(metadata.deepReviewRunManifest).toBe(runManifest);
  });

  describe('unread completion persistence', () => {
    it('persists unreadCompletion from session to metadata', () => {
      const session = createSession({
        hasUnreadCompletion: 'completed',
      });

      const metadata = buildSessionMetadata(session);

      expect(metadata.unreadCompletion).toBe('completed');
    });

    it('persists needsUserAttention from session to metadata', () => {
      const session = createSession({
        needsUserAttention: 'ask_user',
      });

      const metadata = buildSessionMetadata(session);

      expect(metadata.needsUserAttention).toBe('ask_user');
    });

    it('clears unreadCompletion when session has hasUnreadCompletion undefined', () => {
      const session = createSession({
        hasUnreadCompletion: undefined,
      });

      const existingMetadata: SessionMetadata = {
        sessionId: 'session-1',
        sessionName: 'Session Title',
        agentType: 'agentic',
        modelName: 'gpt-test',
        createdAt: 1000,
        lastActiveAt: 1000,
        turnCount: 0,
        messageCount: 0,
        toolCallCount: 0,
        status: 'active',
        tags: [],
        customMetadata: {},
        todos: [],
        workspacePath: '/workspace',
        unreadCompletion: 'completed',
      };

      const metadata = buildSessionMetadata(session, existingMetadata);

      // The cleared value (undefined) must NOT fall back to existingMetadata.unreadCompletion
      expect(metadata.unreadCompletion).toBeUndefined();
    });

    it('clears needsUserAttention when session has needsUserAttention undefined', () => {
      const session = createSession({
        needsUserAttention: undefined,
      });

      const existingMetadata: SessionMetadata = {
        sessionId: 'session-1',
        sessionName: 'Session Title',
        agentType: 'agentic',
        modelName: 'gpt-test',
        createdAt: 1000,
        lastActiveAt: 1000,
        turnCount: 0,
        messageCount: 0,
        toolCallCount: 0,
        status: 'active',
        tags: [],
        customMetadata: {},
        todos: [],
        workspacePath: '/workspace',
        needsUserAttention: 'tool_confirm',
      };

      const metadata = buildSessionMetadata(session, existingMetadata);

      expect(metadata.needsUserAttention).toBeUndefined();
    });
  });
});
