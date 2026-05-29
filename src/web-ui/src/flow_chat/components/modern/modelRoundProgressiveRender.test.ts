import { describe, expect, it } from 'vitest';

import {
  MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE,
  MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT,
  getInitialModelRoundGroupRenderCount,
  getNextModelRoundGroupRenderCount,
  getSynchronizedModelRoundGroupRenderCount,
} from './modelRoundProgressiveRender';

describe('modelRoundProgressiveRender', () => {
  it('renders completed large historical rounds in bounded initial chunks', () => {
    expect(getInitialModelRoundGroupRenderCount({
      groupCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 25,
      isStreaming: false,
    })).toBe(MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT);
  });

  it('keeps streaming rounds fully rendered', () => {
    expect(getInitialModelRoundGroupRenderCount({
      groupCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 25,
      isStreaming: true,
    })).toBe(MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 25);
  });

  it('advances chunked historical rendering without overshooting the group count', () => {
    expect(getNextModelRoundGroupRenderCount({
      currentCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT,
      groupCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE + 5,
    })).toBe(MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE);

    expect(getNextModelRoundGroupRenderCount({
      currentCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE,
      groupCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE + 5,
    })).toBe(MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE + 5);
  });

  it('does not shrink a fully rendered round after streaming completes', () => {
    expect(getSynchronizedModelRoundGroupRenderCount({
      currentCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 120,
      groupCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 120,
      initialCount: MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT,
      isStreaming: false,
    })).toBe(MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT + 120);
  });
});
