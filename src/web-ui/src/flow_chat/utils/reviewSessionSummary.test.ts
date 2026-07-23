import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  collectReviewChangedFiles,
  findLatestCodeReviewResultState,
  findLatestCodeReviewResult,
  summarizeCodeReviewResult,
} from './reviewSessionSummary';

function session(overrides: Partial<Session>): Session {
  return {
    sessionId: 'review-child',
    title: 'Deep review',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'deep_review',
    ...overrides,
  };
}

describe('reviewSessionSummary', () => {
  it('parses the latest submit_code_review result from a review session', () => {
    const reviewSession = session({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'review-child',
        userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
        modelRounds: [{
          id: 'round-1',
          index: 0,
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 1,
          items: [{
            id: 'review-result',
            type: 'tool',
            timestamp: 2,
            status: 'completed',
            toolName: 'submit_code_review',
            toolCall: { id: 'tool-1', input: {} },
            toolResult: {
              success: true,
              result: JSON.stringify({
                summary: {
                  overall_assessment: 'Needs one safe fix.',
                  risk_level: 'medium',
                  recommended_action: 'request_changes',
                },
                issues: [
                  {
                    severity: 'high',
                    certainty: 'confirmed',
                    category: 'correctness',
                    file: 'src/app.ts',
                    line: 12,
                    title: 'Wrong branch',
                    description: 'The branch is inverted.',
                    suggestion: 'Flip the branch.',
                  },
                ],
                positive_points: [],
                review_mode: 'deep',
                remediation_plan: ['Flip the branch.'],
              }),
            },
          }],
        }],
        status: 'completed',
        startTime: 1,
      }],
    });

    const result = findLatestCodeReviewResult(reviewSession);
    const summary = summarizeCodeReviewResult(result);

    expect(summary).toMatchObject({
      issueCount: 1,
      riskLevel: 'medium',
      recommendedAction: 'request_changes',
      summaryText: 'Needs one safe fix.',
    });
    expect(findLatestCodeReviewResultState(reviewSession)).toMatchObject({
      status: 'valid',
      result,
    });
  });

  it('reports missing review results when no submit_code_review tool result exists', () => {
    const reviewSession = session({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'review-child',
        userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
        modelRounds: [{
          id: 'round-1',
          index: 0,
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 1,
          items: [],
        }],
        status: 'completed',
        startTime: 1,
      }],
    });

    expect(findLatestCodeReviewResult(reviewSession)).toBeNull();
    expect(findLatestCodeReviewResultState(reviewSession)).toEqual({
      status: 'missing',
      reason: 'no_submit_code_review',
    });
  });

  it('reports invalid review results when submit_code_review returns unreadable data', () => {
    const reviewSession = session({
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'review-child',
        userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
        modelRounds: [{
          id: 'round-1',
          index: 0,
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 1,
          items: [{
            id: 'review-result',
            type: 'tool',
            timestamp: 2,
            status: 'completed',
            toolName: 'submit_code_review',
            toolCall: { id: 'tool-1', input: {} },
            toolResult: {
              success: true,
              result: 'not json',
            },
          }],
        }],
        status: 'completed',
        startTime: 1,
      }],
    });

    expect(findLatestCodeReviewResult(reviewSession)).toBeNull();
    expect(findLatestCodeReviewResultState(reviewSession)).toEqual({
      status: 'invalid',
      reason: 'unreadable_submit_code_review',
    });
  });

  it('uses snapshot changed files before falling back to review issue files or requested files', () => {
    const result = {
      summary: {
        overall_assessment: 'Done.',
        risk_level: 'low' as const,
        recommended_action: 'approve' as const,
      },
      issues: [
        { file: 'src/from-issue.ts' },
        { file: 'src/from-issue.ts' },
      ],
      positive_points: [],
    };

    expect(collectReviewChangedFiles({
      snapshotFiles: ['src/fixed.ts', 'src/fixed.ts'],
      reviewResult: result,
      requestedFiles: ['src/requested.ts'],
    })).toEqual(['src/fixed.ts']);

    expect(collectReviewChangedFiles({
      snapshotFiles: [],
      reviewResult: result,
      requestedFiles: ['src/requested.ts'],
    })).toEqual(['src/from-issue.ts']);
  });
});
