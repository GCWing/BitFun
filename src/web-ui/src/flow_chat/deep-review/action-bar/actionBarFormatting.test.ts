import { describe, expect, it } from 'vitest';
import {
  classifyReviewActionErrorMessage,
  formatElapsedTime,
} from './actionBarFormatting';

describe('action bar formatting', () => {
  it('formats elapsed milliseconds without changing existing labels', () => {
    expect(formatElapsedTime(999)).toBe('0s');
    expect(formatElapsedTime(12_000)).toBe('12s');
    expect(formatElapsedTime(60_000)).toBe('1m 0s');
    expect(formatElapsedTime(125_000)).toBe('2m 5s');
  });

  it('classifies only the stable product-owned dialog-start prefix', () => {
    expect(classifyReviewActionErrorMessage(
      'Failed to start dialog turn: provider quota exhausted',
    )).toEqual({
      kind: 'start_dialog_turn_failed',
      reason: 'provider quota exhausted',
    });

    expect(classifyReviewActionErrorMessage('Failed to start dialog turn:')).toEqual({
      kind: 'start_dialog_turn_failed',
      reason: null,
    });
  });

  it('keeps arbitrary backend and provider details as raw display data', () => {
    expect(classifyReviewActionErrorMessage('Provider request failed in region us-east')).toEqual({
      kind: 'raw',
      message: 'Provider request failed in region us-east',
    });
  });
});
