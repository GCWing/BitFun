import { describe, expect, it } from 'vitest';
import type { PermissionRequest } from '@/infrastructure/api/service-api/AgentAPI';
import {
  applyPermissionRequestEvent,
  pendingPermissionToolCallIdsForSession,
  reconcilePermissionRequestSnapshot,
  selectPermissionRequestsForSession,
  selectActivePermissionBatch,
  sortPermissionRequests,
} from './permissionRequestRouting';

function request(
  requestId: string,
  sessionId: string,
  toolCallId: string | undefined,
  parentSessionId?: string,
  parentToolCallId?: string,
): PermissionRequest {
  return {
    requestId,
    roundId: parentSessionId ? 'round-child' : 'round-parent',
    order: requestId === 'parent-request' ? 0 : 1,
    sessionId,
    toolCallId,
    projectId: 'project-1',
    agentId: parentSessionId ? 'Explore' : 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
    source: { kind: 'tool_call', identity: 'Write' },
    delegation: parentSessionId && parentToolCallId
      ? {
          parentSessionId,
          parentDialogTurnId: 'parent-turn',
          parentToolCallId,
          subagentType: 'Explore',
        }
      : undefined,
  };
}

describe('permission request routing', () => {
  const parentRequest = request('parent-request', 'parent-session', 'parent-tool');
  const childRequest = request(
    'child-request',
    'child-session',
    'child-tool',
    'parent-session',
    'parent-task',
  );
  const unrelatedRequest = request('other-request', 'other-session', 'other-tool');

  it('selects direct and delegated requests without leaking unrelated sessions', () => {
    const requests = [parentRequest, childRequest, unrelatedRequest];

    expect(selectPermissionRequestsForSession(requests, 'parent-session')).toEqual([
      parentRequest,
      childRequest,
    ]);
    expect(selectPermissionRequestsForSession(requests, 'child-session')).toEqual([childRequest]);
    expect(selectPermissionRequestsForSession(requests, 'other-session')).toEqual([
      unrelatedRequest,
    ]);
    expect(selectPermissionRequestsForSession(requests, undefined)).toEqual([]);
  });

  it('maps delegated requests to the parent Task card and direct requests to their tool card', () => {
    const requests = [parentRequest, childRequest, unrelatedRequest];

    expect([
      ...pendingPermissionToolCallIdsForSession(requests, 'parent-session'),
    ]).toEqual(['parent-tool', 'parent-task']);
    expect([
      ...pendingPermissionToolCallIdsForSession(requests, 'child-session'),
    ]).toEqual(['child-tool']);
    expect([
      ...pendingPermissionToolCallIdsForSession(requests, 'other-session'),
    ]).toEqual(['other-tool']);
  });

  it('sorts requests by order within a round while preserving batch arrival order', () => {
    const laterRound = { ...parentRequest, requestId: 'later-round', roundId: 'round-later', order: 0 };
    const firstRoundLater = { ...parentRequest, requestId: 'first-round-later', roundId: 'round-first', order: 2 };
    const firstRoundEarlier = { ...parentRequest, requestId: 'first-round-earlier', roundId: 'round-first', order: 1 };

    expect(sortPermissionRequests([laterRound, firstRoundLater, firstRoundEarlier]).map((item) => item.requestId)).toEqual([
      'later-round',
      'first-round-earlier',
      'first-round-later',
    ]);
  });

  it('does not guess a tool card when the relevant call ID is missing', () => {
    const delegatedWithoutParentCall = request(
      'missing-parent-call',
      'child-session',
      'child-tool',
    );
    delegatedWithoutParentCall.delegation = {
      parentSessionId: 'parent-session',
      parentDialogTurnId: 'parent-turn',
      parentToolCallId: '',
      subagentType: 'Explore',
    };

    expect([
      ...pendingPermissionToolCallIdsForSession(
        [delegatedWithoutParentCall],
        'parent-session',
      ),
    ]).toEqual([]);
  });

  it('deduplicates asked events and clears replied or cancelled requests', () => {
    const updatedChild = { ...childRequest, resources: ['src/lib.rs'] };
    let requests = applyPermissionRequestEvent([], { event: 'asked', request: childRequest });
    requests = applyPermissionRequestEvent(requests, {
      event: 'asked',
      request: updatedChild,
    });
    expect(requests).toEqual([updatedChild]);

    requests = applyPermissionRequestEvent(requests, {
      event: 'replied',
      requestId: childRequest.requestId,
      reply: { reply: 'once' },
      source: 'user',
    });
    expect(requests).toEqual([]);

    requests = applyPermissionRequestEvent([childRequest], {
      event: 'cancelled',
      requestId: childRequest.requestId,
      reason: 'parent cancelled',
    });
    expect(requests).toEqual([]);
  });

  it('keeps event-only requests while excluding requests resolved before snapshot hydration', () => {
    expect(
      reconcilePermissionRequestSnapshot(
        [childRequest],
        [parentRequest, unrelatedRequest],
        new Set([unrelatedRequest.requestId]),
      ),
    ).toEqual([parentRequest, childRequest]);
  });

  it('selects only the first concrete session and round as the active batch', () => {
    const first = { ...parentRequest, requestId: 'first', order: 0 };
    const sameRound = { ...parentRequest, requestId: 'same-round', order: 1 };
    const laterRound = { ...parentRequest, requestId: 'later-round', roundId: 'round-later', order: 0 };

    expect(selectActivePermissionBatch([laterRound, sameRound, first], 'parent-session')).toEqual({
      sessionId: 'parent-session',
      roundId: 'round-later',
      requests: [laterRound],
    });
    expect(selectActivePermissionBatch([first, sameRound, laterRound], 'parent-session')).toEqual({
      sessionId: 'parent-session',
      roundId: 'round-parent',
      requests: [first, sameRound],
    });
  });

  it('routes delegated requests to the parent without merging separate child batches', () => {
    const childA = { ...childRequest, requestId: 'child-a', sessionId: 'child-a-session' };
    const childB = {
      ...childRequest,
      requestId: 'child-b',
      sessionId: 'child-b-session',
      roundId: childA.roundId,
    };

    expect(selectActivePermissionBatch([childA, childB], 'parent-session')).toEqual({
      sessionId: 'child-a-session',
      roundId: 'round-child',
      requests: [childA],
    });
  });
});
