import { useMemo } from 'react';

export interface TokenBreakdown {
  systemPrompt: number;
  toolInjection: number;
  memories: number;
  total: number;
  contextWindowSize: number;
  percentage: string;
}

const CONTEXT_WINDOW_SIZE = 128_000;
const TOKENS_PER_TOOL = 45;
const TOKENS_PER_MEMORY = 60;
const CHARS_PER_TOKEN = 3;

export function estimateTokens(
  body: string,
  enabledToolCount: number,
  memoriesCount: number,
): TokenBreakdown {
  const systemPrompt = Math.ceil(body.length / CHARS_PER_TOKEN);
  const toolInjection = enabledToolCount * TOKENS_PER_TOOL;
  const memories = memoriesCount * TOKENS_PER_MEMORY;
  const total = systemPrompt + toolInjection + memories;
  const percentage = ((total / CONTEXT_WINDOW_SIZE) * 100).toFixed(1) + '%';

  return {
    systemPrompt,
    toolInjection,
    memories,
    total,
    contextWindowSize: CONTEXT_WINDOW_SIZE,
    percentage,
  };
}

export function useTokenEstimate(
  body: string,
  enabledToolCount: number,
  memoriesCount: number,
): TokenBreakdown {
  return useMemo(
    () => estimateTokens(body, enabledToolCount, memoriesCount),
    [body, enabledToolCount, memoriesCount],
  );
}

export function formatTokenCount(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
