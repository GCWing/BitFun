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
      waitingReviewers: [{
        toolId: 'task-1',
        subagentType: 'ReviewSecurity',
        displayName: undefined,
        status: 'queued_for_capacity',
        reason: 'provider_concurrency_limit',
        optional: true,
        queueElapsedMs: 1200,
        maxQueueWaitSeconds: 60,
      }],
    });
  });

  it('uses the run manifest display name for the waiting reviewer list', () => {
    const session = createSession('deep_review');
    session.deepReviewRunManifest = {
      reviewMode: 'deep',
      policySource: 'default-review-team-config',
      target: { source: 'session_files', resolution: 'resolved', files: [], tags: [], evidence: [], warnings: [] },
      strategyLevel: 'normal',
      strategyDecision: {
        teamDefaultStrategy: 'normal',
        finalStrategy: 'normal',
        frontendRecommendation: { strategyLevel: 'normal', reasons: [] },
        backendRecommendation: { strategyLevel: 'normal', reasons: [] },
        decisionSource: 'team_default',
        rationale: [],
      },
      preReviewSummary: {
        targetSummary: 'No target files',
        riskSummary: 'No risk summary',
        recommendedStrategy: 'normal',
        reviewerSummary: 'Reviewers selected',
        warnings: [],
      },
      concurrencyPolicy: {
        maxParallelInstances: 2,
        staggerSeconds: 0,
        batchExtrasSeparately: true,
        allowProviderCapacityQueue: true,
        maxQueueWaitSeconds: 60,
      },
      executionPolicy: {
        reviewerTimeoutSeconds: 300,
        judgeTimeoutSeconds: 300,
        maxSameRoleInstances: 1,
        reviewerFileSplitThreshold: 20,
      },
      sharedContextCache: {
        source: 'work_packets',
        entries: [],
      },
      incrementalReviewCache: {
        source: 'target_manifest',
        strategy: 'reuse_completed_packets_when_fingerprint_matches',
        cacheKey: 'cache',
        fingerprint: 'fingerprint',
        filePaths: [],
        workspaceAreas: [],
        targetTags: [],
        reviewerPacketIds: [],
        lineCountSource: 'unknown',
        invalidatesOn: [],
      },
      tokenBudget: {
        mode: 'balanced',
        estimatedPromptTokens: 0,
        estimatedReviewerTokens: 0,
        estimatedJudgeTokens: 0,
        estimatedTotalTokens: 0,
        activeReviewerCalls: 1,
        eligibleExtraReviewerCount: 0,
        maxExtraReviewers: 0,
        skippedReviewerIds: [],
        warnings: [],
      },
      coreReviewers: [],
      enabledExtraReviewers: [],
      skippedReviewers: [],
      workPackets: [{
        packetId: 'reviewer:ReviewSecurity',
        phase: 'reviewer',
        launchBatch: 1,
        subagentId: 'ReviewSecurity',
        displayName: 'Security reviewer',
        roleName: 'Security reviewer',
        assignedScope: {
          kind: 'review_target',
          targetSource: 'session_files',
          targetResolution: 'resolved',
          targetTags: [],
          fileCount: 0,
          files: [],
          excludedFileCount: 0,
        },
        allowedTools: ['Read'],
        timeoutSeconds: 300,
        requiredOutputFields: ['packet_id', 'status'],
        strategyLevel: 'normal',
        strategyDirective: 'Review security risks.',
        model: 'fast',
      }],
    };

    const state = buildDeepReviewCapacityQueueStateFromEvent(createQueueEvent(), session);

    expect(state?.waitingReviewers).toEqual([
      expect.objectContaining({
        subagentType: 'ReviewSecurity',
        displayName: 'Security reviewer',
      }),
    ]);
  });

  it('ignores queue events for non-Deep Review sessions', () => {
    const state = buildDeepReviewCapacityQueueStateFromEvent(
      createQueueEvent(),
      createSession('normal'),
    );

    expect(state).toBeNull();
  });
});
