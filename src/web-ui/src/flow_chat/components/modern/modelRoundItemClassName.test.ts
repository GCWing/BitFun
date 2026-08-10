import { describe, expect, it } from 'vitest';
import { getModelRoundItemClassName } from './modelRoundItemClassName';

describe('getModelRoundItemClassName', () => {
  it('only reflects the streaming state', () => {
    expect(getModelRoundItemClassName({
      isVisuallyStreaming: true,
    })).toBe('model-round-item model-round-item--streaming');

    expect(getModelRoundItemClassName({
      isVisuallyStreaming: false,
    })).toBe('model-round-item model-round-item--complete');
  });

  it('never emits a mount / enter animation modifier', () => {
    expect(getModelRoundItemClassName({ isVisuallyStreaming: true }))
      .not.toContain('model-round-item--enter');
    expect(getModelRoundItemClassName({ isVisuallyStreaming: false }))
      .not.toContain('model-round-item--enter');
  });
});
