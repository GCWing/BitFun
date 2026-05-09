import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import { buildDeepReviewContinuationPrompt, deriveDeepReviewInterruption } from './deepReviewContinuation';
import type { AiErrorDetail } from '@/shared/ai-errors/aiErrorPresenter';

function createDeepReviewSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'deep-review-session',
    title: 'Deep Review',
    dialogTurns: [],
    status: 'idle',
    config: {
      modelName: 'auto',
      agentType: 'DeepReview',
    },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    mode: 'DeepReview',
    sessionKind: 'deep_review',
    parentSessionId: 'parent-session',
    btwOrigin: {
      requestId: 'review-request',
      parentSessionId: 'parent-session',
      parentDialogTurnId: 'parent-turn',
      parentTurnIndex: 1,
    },
    ...overrides,
  };
}

describe('deepReviewContinuation', () => {
  it('derives an interrupted state even before submit_code_review exists', () => {
    const errorDetail: AiErrorDetail = {
      category: 'provider_unavailable',
      provider: 'anthropic',
      providerCode: 'overloaded_error',
      requestId: 'req-1',
    };
    const session = createDeepReviewSession({
      error: 'Provider overloaded',
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'deep-review-session',
          timestamp: 1,
          status: 'error',
          userMessage: {
            id: 'user-1',
            content: 'Run a deep code review using the parallel Code Review Team.',
            timestamp: 1,
          },
          modelRounds: [],
          startTime: 1,
          error: 'Provider overloaded',
        },
      ],
    });

    const interruption = deriveDeepReviewInterruption(session, errorDetail);

    expect(interruption?.phase).toBe('review_interrupted');
    expect(interruption?.canResume).toBe(true);
    expect(interruption?.recommendedActions.map((action) => action.code)).toContain('wait_and_retry');
  });

  it('blocks continuation for quota errors and points to model settings', () => {
    const interruption = deriveDeepReviewInterruption(createDeepReviewSession({
      error: 'AI client error: provider quota',
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'deep-review-session',
          timestamp: 1,
          status: 'error',
          userMessage: {
            id: 'user-1',
            content: 'Run a deep code review using the parallel Code Review Team.',
            timestamp: 1,
          },
          modelRounds: [],
          startTime: 1,
          error: 'AI client error: provider quota',
        },
      ],
    }), {
      category: 'provider_quota',
      provider: 'glm',
      providerCode: '1113',
      requestId: 'req-1',
    });

    expect(interruption?.phase).toBe('resume_blocked');
    expect(interruption?.canResume).toBe(false);
    expect(interruption?.recommendedActions.map((action) => action.code)).toContain('open_model_settings');
  });

  it('builds a continuation prompt that preserves completed reviewer work', () => {
    const session = createDeepReviewSession({
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'deep-review-session',
          timestamp: 1,
          status: 'error',
          userMessage: {
            id: 'user-1',
            content: 'Original command:\n/DeepReview review latest commit',
            timestamp: 1,
          },
          startTime: 1,
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              startTime: 1,
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              items: [
                {
                  id: 'tool-1',
                  type: 'tool',
                  toolName: 'Task',
                  toolCall: {
                    id: 'call-performance',
                    input: { subagent_type: 'ReviewPerformance' },
                  },
                  toolResult: {
                    result: { text: 'Performance reviewer found no blocking issues.' },
                    success: true,
                  },
                  startTime: 1,
                  timestamp: 1,
                  status: 'completed',
                },
                {
                  id: 'tool-2',
                  type: 'tool',
                  toolName: 'Task',
                  toolCall: {
                    id: 'call-security',
                    input: { subagent_type: 'ReviewSecurity' },
                  },
                  toolResult: {
                    result: null,
                    success: false,
                    error: "Timeout: Subagent 'ReviewSecurity' timed out after 300 seconds",
                  },
                  startTime: 2,
                  timestamp: 2,
                  status: 'error',
                },
              ],
            },
          ],
          error: 'Timeout',
        },
      ],
    });

    const interruption = deriveDeepReviewInterruption(session, { category: 'timeout' });
    const prompt = buildDeepReviewContinuationPrompt(interruption!);

    expect(prompt).toContain('Continue the interrupted Deep Review');
    expect(prompt).toContain('Do not restart completed reviewer work');
    expect(prompt).toContain('ReviewPerformance: completed');
    expect(prompt).toContain('ReviewSecurity: timed_out');
  });

  it('marks policy-ineligible reviewers as skipped so continuation does not re-run them', () => {
    const session = createDeepReviewSession({
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'deep-review-session',
          timestamp: 1,
          status: 'error',
          userMessage: {
            id: 'user-1',
            content: 'Run a deep review for src/crates/core/src/lib.rs',
            timestamp: 1,
          },
          startTime: 1,
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              startTime: 1,
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              items: [
                {
                  id: 'tool-frontend',
                  type: 'tool',
                  toolName: 'Task',
                  toolCall: {
                    id: 'call-frontend',
                    input: { subagent_type: 'ReviewFrontend' },
                  },
                  toolResult: {
                    result: null,
                    success: false,
                    error:
                      'DeepReview Task policy violation: {"code":"deep_review_subagent_not_review","message":"Subagent ReviewFrontend must be marked for review."}',
                  },
                  startTime: 1,
                  timestamp: 1,
                  status: 'error',
                },
              ],
            },
          ],
          error: 'DeepReview Task policy violation',
        },
      ],
    });

    const interruption = deriveDeepReviewInterruption(session, { category: 'model_error' });
    const prompt = buildDeepReviewContinuationPrompt(interruption!);

    expect(interruption?.reviewers).toEqual([
      expect.objectContaining({
        reviewer: 'ReviewFrontend',
        status: 'skipped',
      }),
    ]);
    expect(prompt).toContain('ReviewFrontend: skipped');
    expect(prompt).toContain('Do not re-run skipped, non-applicable, or policy-ineligible reviewers');
  });

  it('derives a resumable interruption for manually cancelled deep reviews without model recovery actions', () => {
    const session = createDeepReviewSession({
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'deep-review-session',
          timestamp: 1,
          status: 'cancelled',
          userMessage: {
            id: 'user-1',
            content: 'Original command:\n/DeepReview review current changes',
            timestamp: 1,
          },
          startTime: 1,
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              startTime: 1,
              isStreaming: false,
              isComplete: true,
              status: 'cancelled',
              items: [
                {
                  id: 'tool-security',
                  type: 'tool',
                  toolName: 'Task',
                  toolCall: {
                    id: 'call-security',
                    input: { subagent_type: 'ReviewSecurity' },
                  },
                  startTime: 1,
                  timestamp: 1,
                  status: 'cancelled',
                },
              ],
            },
          ],
        },
      ],
    });

    const interruption = deriveDeepReviewInterruption(session);
    const actionCodes = interruption?.recommendedActions.map((action) => action.code);

    expect(interruption?.phase).toBe('review_interrupted');
    expect(interruption?.canResume).toBe(true);
    expect(interruption?.reviewers).toEqual([
      expect.objectContaining({
        reviewer: 'ReviewSecurity',
        status: 'cancelled',
      }),
    ]);
    expect(actionCodes).toEqual(['continue', 'copy_diagnostics']);
    expect(actionCodes).not.toContain('switch_model');
    expect(actionCodes).not.toContain('wait_and_retry');
  });
});
