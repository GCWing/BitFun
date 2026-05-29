export const MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT = 80;
export const MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE = 80;
export const MODEL_ROUND_GROUP_RENDER_CHUNK_DELAY_MS = 16;

export function getInitialModelRoundGroupRenderCount(params: {
  groupCount: number;
  isStreaming: boolean;
}): number {
  const { groupCount, isStreaming } = params;
  if (isStreaming || groupCount <= MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT) {
    return groupCount;
  }

  return MODEL_ROUND_INITIAL_GROUP_RENDER_LIMIT;
}

export function getNextModelRoundGroupRenderCount(params: {
  currentCount: number;
  groupCount: number;
}): number {
  const { currentCount, groupCount } = params;
  return Math.min(groupCount, currentCount + MODEL_ROUND_GROUP_RENDER_CHUNK_SIZE);
}

export function getSynchronizedModelRoundGroupRenderCount(params: {
  currentCount: number;
  groupCount: number;
  initialCount: number;
  isStreaming: boolean;
}): number {
  const { currentCount, groupCount, initialCount, isStreaming } = params;
  if (isStreaming) {
    return groupCount;
  }

  return Math.min(groupCount, Math.max(currentCount, initialCount));
}
