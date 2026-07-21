import { describe, expect, it } from 'vitest';
import { getModelRoundItemClassName } from './modelRoundItemClassName';

describe('getModelRoundItemClassName', () => {
  it('does not attach enter animation when a streaming round becomes complete', () => {
    expect(getModelRoundItemClassName({
      isVisuallyStreaming: true,
      shouldPlayEnterAnimation: false,
    })).toBe('model-round-item model-round-item--streaming');

    expect(getModelRoundItemClassName({
      isVisuallyStreaming: false,
      shouldPlayEnterAnimation: false,
    })).toBe('model-round-item model-round-item--complete');
  });

  it('allows enter animation only for freshly mounted complete rounds', () => {
    expect(getModelRoundItemClassName({
      isVisuallyStreaming: false,
      shouldPlayEnterAnimation: true,
    })).toBe('model-round-item model-round-item--complete model-round-item--enter');
  });
});
