import { describe, expect, it } from 'vitest';
import { shouldTriggerReviewReadyGlint } from './reviewReadyGlint';

describe('shouldTriggerReviewReadyGlint', () => {
  it('triggers when an observed processing turn completes with reviewable changes', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'completed',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: null,
      nextReviewableCount: 9,
      loadingStats: false,
      reviewActionAvailable: true,
    })).toBe(true);
  });

  it('does not trigger while stats are still loading', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'completed',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: null,
      nextReviewableCount: 9,
      loadingStats: true,
      reviewActionAvailable: true,
    })).toBe(false);
  });

  it('does not trigger before the current turn is completely finished', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'finishing',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: null,
      nextReviewableCount: 9,
      loadingStats: false,
      reviewActionAvailable: true,
    })).toBe(false);
  });

  it('does not trigger when review actions are unavailable', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'completed',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: null,
      nextReviewableCount: 9,
      loadingStats: false,
      reviewActionAvailable: false,
    })).toBe(false);
  });

  it('waits until the session state machine is no longer processing', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'completed',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: null,
      nextReviewableCount: 9,
      loadingStats: false,
      reviewActionAvailable: true,
      sessionProcessing: true,
    })).toBe(false);
  });

  it('does not retrigger for a turn that already prompted the user', () => {
    expect(shouldTriggerReviewReadyGlint({
      currentTurnId: 'turn-1',
      currentTurnStatus: 'completed',
      observedProcessingTurnId: 'turn-1',
      promptedTurnId: 'turn-1',
      nextReviewableCount: 9,
      loadingStats: false,
      reviewActionAvailable: true,
    })).toBe(false);
  });
});
