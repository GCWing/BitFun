import { describe, expect, it } from 'vitest';
import {
  buildLaunchCleanupError,
  createDeepReviewLaunchError,
  getDeepReviewLaunchErrorMessage,
  isSessionMissingError,
  normalizeErrorMessage,
  type FailedDeepReviewCleanupResult,
} from './launchErrors';

describe('Deep Review launch errors', () => {
  it('normalizes empty and string errors', () => {
    expect(normalizeErrorMessage(' network down ')).toBe('network down');
    expect(normalizeErrorMessage(new Error(' model missing '))).toBe('model missing');
    expect(normalizeErrorMessage(null)).toBe('Deep review failed to start');
  });

  it('recognizes missing-session cleanup failures as non-fatal', () => {
    expect(isSessionMissingError(new Error('Session does not exist'))).toBe(true);
    expect(isSessionMissingError(new Error('session not found'))).toBe(true);
    expect(isSessionMissingError(new Error('permission denied'))).toBe(false);
  });

  it('creates localized launch errors for user-facing surfaces', () => {
    const error = createDeepReviewLaunchError(
      'send_start_message',
      new Error('SSE stream connection timeout'),
      'child-123',
      { cleanupCompleted: true, cleanupIssues: [] },
    );

    expect(error.message).toBe('Network connection was interrupted before Deep Review could start.');
    expect(error.launchErrorMessageKey).toBe('deepReviewActionBar.launchError.network');
    expect(error.launchErrorCategory).toBe('network');
    expect(error.childSessionId).toBe('child-123');
    expect(
      getDeepReviewLaunchErrorMessage(error, (key) => `translated:${key}`),
    ).toBe('translated:deepReviewActionBar.launchError.network');
  });

  it('keeps original launch error when cleanup completed', () => {
    const original = new Error('Pane open failed');
    const cleanupResult: FailedDeepReviewCleanupResult = {
      cleanupCompleted: true,
      cleanupIssues: [],
    };

    expect(buildLaunchCleanupError(
      'open_aux_pane',
      'child-123',
      original,
      cleanupResult,
    )).toBe(original);
  });

  it('adds cleanup context when cleanup is incomplete', () => {
    const cleanupResult: FailedDeepReviewCleanupResult = {
      cleanupCompleted: false,
      cleanupIssues: ['Failed to remove local state.'],
    };

    expect(buildLaunchCleanupError(
      'open_aux_pane',
      'child-123',
      new Error('Pane open failed'),
      cleanupResult,
    ).message).toContain('The partially created deep review session (child-123) may need manual cleanup.');
  });
});
