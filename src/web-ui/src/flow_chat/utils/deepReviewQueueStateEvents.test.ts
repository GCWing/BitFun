import { describe, expect, it } from 'vitest';
import type { DeepReviewQueueStateChangedEvent } from '@/infrastructure/api/service-api/AgentAPI';
import type { Session } from '../types/flow-chat';
import { buildDeepReviewCapacityQueueStateFromEvent } from './deepReviewQueueStateEvents';

function createQueueEvent(
  overrides: Partial<DeepReviewQueueStateChangedEvent> = {},
): DeepReviewQueueStateChangedEvent {
  return {
    sessionId: 'review-child',
    turnId: 'turn-1',
    queueState: {
      toolId: 'task-1',
      subagentType: 'ReviewSecurity',
      status: 'queued_for_capacity',
      reason: 'provider_concurrency_limit',
      queuedReviewerCount: 2,
      activeReviewerCount: 1,
      effectiveParallelInstances: 2,
      optionalReviewerCount: 1,
      queueElapsedMs: 1200,
      maxQueueWaitSeconds: 60,
      sessionConcurrencyHigh: true,
    },
    ...overrides,
  };
}

function createSession(sessionKind: Session['sessionKind']): Session {
  return {
    sessionId: 'review-child',
    sessionKind,
    status: 'active',
    createdAt: 1000,
    updatedAt: 1000,
    lastActiveAt: 1000,
    dialogTurns: [],
  } as Session;
}

describe('buildDeepReviewCapacityQueueStateFromEvent', () => {
  it('maps backend queue events into the action bar queue state for Deep Review sessions', () => {
    const state = buildDeepReviewCapacityQueueStateFromEvent(
      createQueueEvent(),
      createSession('deep_review'),
    );

    expect(state).toEqual({
      toolId: 'task-1',
      subagentType: 'ReviewSecurity',
      dialogTurnId: 'turn-1',
      status: 'queued_for_capacity',
      reason: 'provider_concurrency_limit',
      queuedReviewerCount: 2,
      activeReviewerCount: 1,
      effectiveParallelInstances: 2,
      optionalReviewerCount: 1,
      queueElapsedMs: 1200,
      runElapsedMs: undefined,
      maxQueueWaitSeconds: 60,
      sessionConcurrencyHigh: true,
      controlMode: 'backend',
    });
  });

  it('ignores queue events for non-Deep Review sessions', () => {
    const state = buildDeepReviewCapacityQueueStateFromEvent(
      createQueueEvent(),
      createSession('normal'),
    );

    expect(state).toBeNull();
  });
});
