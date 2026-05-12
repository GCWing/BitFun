import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  buildErrorAttribution,
  buildRecoveryPlan,
  extractPartialReviewData,
} from './deepReviewExperience';

function sessionWithTaskResult(result: unknown): Session {
  return {
    sessionId: 'deep-review-session',
    title: 'Deep Review',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'deep-review-session',
      timestamp: 1,
      status: 'error',
      userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
      startTime: 1,
      modelRounds: [{
        id: 'round-1',
        index: 0,
        startTime: 1,
        isStreaming: false,
        isComplete: true,
        status: 'completed',
        items: [{
          id: 'tool-1',
          type: 'tool',
          toolName: 'Task',
          toolCall: {
            id: 'call-security',
            input: { subagent_type: 'ReviewSecurity' },
          },
          toolResult: {
            result,
            success: true,
          },
          startTime: 1,
          timestamp: 1,
          status: 'completed',
        }],
      }],
    }],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'deep_review',
  } as Session;
}

describe('deepReviewExperience', () => {
  it('keeps skipped reviewers out of rerun work and shows them in the recovery plan', () => {
    const plan = buildRecoveryPlan({
      phase: 'review_interrupted',
      childSessionId: 'deep-review-session',
      originalTarget: '/DeepReview review latest commit',
      errorDetail: { category: 'model_error' },
      canResume: true,
      recommendedActions: [],
      reviewers: [
        { reviewer: 'ReviewPerformance', status: 'completed' },
        { reviewer: 'ReviewSecurity', status: 'timed_out' },
        { reviewer: 'ReviewFrontend', status: 'skipped' },
      ],
    });

    expect(plan.willPreserve).toEqual(['ReviewPerformance']);
    expect(plan.willRerun).toEqual(['ReviewSecurity']);
    expect(plan.willSkip).toEqual(['ReviewFrontend']);
  });

  it('uses specific attribution copy for a missing structured review report', () => {
    const attribution = buildErrorAttribution({
      phase: 'review_interrupted',
      childSessionId: 'deep-review-session',
      originalTarget: '/DeepReview review latest commit',
      errorDetail: { category: 'model_error' },
      canResume: true,
      recommendedActions: [
        { code: 'continue', labelKey: 'errors:ai.actions.continue' },
        { code: 'copy_diagnostics', labelKey: 'errors:ai.actions.copyDiagnostics' },
      ],
      reviewers: [],
      resultRecoveryReason: 'missing_submit_code_review',
    });

    expect(attribution).toMatchObject({
      title: 'deepReviewActionBar.resultRecovery.title',
      description: 'deepReviewActionBar.resultRecovery.missingSubmitCodeReview',
      severity: 'warning',
    });
    expect(attribution.actions.map((action) => action.code)).toEqual([
      'continue',
      'copy_diagnostics',
    ]);
  });

  it('uses user-stop attribution copy for manually cancelled deep reviews', () => {
    const attribution = buildErrorAttribution({
      phase: 'review_interrupted',
      childSessionId: 'deep-review-session',
      originalTarget: '/DeepReview review current changes',
      errorDetail: { category: 'unknown', rawMessage: 'Deep Review was stopped by the user.' },
      canResume: true,
      recommendedActions: [
        { code: 'continue', labelKey: 'errors:ai.actions.continue' },
        { code: 'copy_diagnostics', labelKey: 'errors:ai.actions.copyDiagnostics' },
      ],
      reviewers: [],
      interruptionReason: 'manual_cancelled',
    });

    expect(attribution).toMatchObject({
      category: 'manual_cancelled',
      title: 'deepReviewActionBar.manualCancel.title',
      description: 'deepReviewActionBar.manualCancel.description',
      severity: 'warning',
    });
    expect(attribution.description).not.toBe('errors:ai.genericSuggestion');
    expect(attribution.actions.map((action) => action.code)).toEqual([
      'continue',
      'copy_diagnostics',
    ]);
  });

  it('extracts partial review data from object-shaped completed reviewer results', () => {
    const partial = extractPartialReviewData(sessionWithTaskResult({
      summary: {
        overall_assessment: 'Security pass found one issue.',
      },
      issues: [{
        severity: 'high',
        title: 'Token leaked',
        file: 'src/auth.ts',
      }],
      remediation_plan: ['Remove token logging.'],
    }));

    expect(partial).toMatchObject({
      hasPartialResults: true,
      completedReviewerCount: 1,
      totalReviewerCount: 1,
      completedIssues: [
        expect.objectContaining({
          title: 'Token leaked',
          file: 'src/auth.ts',
        }),
      ],
      completedRemediationItems: ['Remove token logging.'],
      completedReviewerSummaries: ['Security pass found one issue.'],
    });
  });

  it('uses reviewer partial output as visible partial result text when no structured result is available', () => {
    const partial = extractPartialReviewData(sessionWithTaskResult({
      status: 'completed',
      partial_output: 'Security reviewer found likely token logging before the dialog failed.',
    }));

    expect(partial).toMatchObject({
      hasPartialResults: true,
      completedReviewerCount: 1,
      totalReviewerCount: 1,
      completedReviewerSummaries: [
        'Security reviewer found likely token logging before the dialog failed.',
      ],
    });
  });
});
