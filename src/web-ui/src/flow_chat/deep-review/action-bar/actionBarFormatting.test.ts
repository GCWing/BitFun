import { describe, expect, it } from 'vitest';
import {
  classifyReviewActionErrorMessage,
  formatElapsedTime,
  getReviewActionErrorMessage,
} from './actionBarFormatting';
import { createTestI18nT } from '@/test/i18nTestUtils';

describe('action bar formatting', () => {
  const translate = createTestI18nT('flow-chat');

  it('prefers structured launch copy and preserves the original backend reason', () => {
    const error = Object.assign(new Error('Failed to start dialog turn: legacy wrapper'), {
      launchErrorMessageKey: 'deepReviewActionBar.launchError.network',
      originalMessage: 'Failed to start dialog turn: provider connection closed',
    });
    expect(getReviewActionErrorMessage(error, translate, 'fallback')).toBe(
      'Network connection interrupted. Review failed to start.\nprovider connection closed',
    );
  });

  it('supports legacy stored strings, plain errors, and empty failure payloads', () => {
    expect(getReviewActionErrorMessage('Failed to start dialog turn: quota exhausted', translate, 'fallback'))
      .toBe('Unable to start this action: quota exhausted');
    expect(getReviewActionErrorMessage(new Error('provider detail'), translate, 'fallback')).toBe('provider detail');
    expect(getReviewActionErrorMessage(null, translate, 'fallback')).toBe('fallback');
  });

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
