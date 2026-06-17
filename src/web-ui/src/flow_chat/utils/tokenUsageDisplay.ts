import type { Session, TokenUsage } from '../types/flow-chat';

export const DEFAULT_MAX_CONTEXT_TOKENS = 128128;

export type ContextUsageSource = 'agent_prompt' | 'acp_context';

export interface ContextUsageDisplay {
  current: number;
  max: number;
  source: ContextUsageSource;
}

export interface TranslationFn {
  (key: string, params?: Record<string, unknown>): string;
}

export interface ModelRoundUsageMetaItem {
  key: 'completed' | 'duration' | 'tokens';
  label: string;
  value: string;
}

export function formatCompactTokenCount(value: number): string {
  const safeValue = Math.max(0, Math.round(value));
  if (safeValue >= 1_000_000) {
    return `${formatCompactNumber(safeValue / 1_000_000)}M`;
  }
  if (safeValue >= 1_000) {
    return `${formatCompactNumber(safeValue / 1_000)}K`;
  }
  return String(safeValue);
}

function formatCompactNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1).replace(/\.0$/, '');
}

export function getSessionContextUsageDisplay(session?: Session): ContextUsageDisplay {
  if (!session) {
    return {
      current: 0,
      max: DEFAULT_MAX_CONTEXT_TOKENS,
      source: 'agent_prompt',
    };
  }

  if (session.currentAcpContextUsage) {
    return {
      current: session.currentAcpContextUsage.used,
      max: session.currentAcpContextUsage.size,
      source: 'acp_context',
    };
  }

  return {
    current: session.currentTokenUsage?.inputTokens || 0,
    max: session.maxContextTokens || DEFAULT_MAX_CONTEXT_TOKENS,
    source: 'agent_prompt',
  };
}

export function buildContextUsageTooltip(params: {
  baseTooltip: string;
  usage: ContextUsageDisplay;
  t: TranslationFn;
}): string {
  const { baseTooltip, usage, t } = params;
  if (usage.current <= 0 || usage.max <= 0) {
    return baseTooltip;
  }

  const percentage = Math.min(100, Math.round((usage.current / usage.max) * 100));
  const usageText = `${formatCompactTokenCount(usage.current)}/${formatCompactTokenCount(usage.max)} (${percentage}%)`;
  const usageLabel = usage.source === 'acp_context'
    ? t('modelSelector.contextUsage.acpContext', { usage: usageText })
    : t('modelSelector.contextUsage.agentPrompt', { usage: usageText });

  return [
    baseTooltip,
    usageLabel,
    t('modelSelector.contextUsage.toolNote'),
  ].filter(Boolean).join(' · ');
}

export function formatElapsedDuration(durationMs: number): string {
  const safeMs = Math.max(0, Math.round(durationMs));
  if (safeMs < 1000) {
    return `${safeMs}ms`;
  }

  const seconds = safeMs / 1000;
  if (seconds < 60) {
    return `${formatCompactNumber(Math.round(seconds * 10) / 10)}s`;
  }

  const wholeSeconds = Math.round(seconds);
  const minutes = Math.floor(wholeSeconds / 60);
  const remainingSeconds = wholeSeconds % 60;
  if (minutes < 60) {
    return remainingSeconds === 0 ? `${minutes}m` : `${minutes}m ${remainingSeconds}s`;
  }

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes === 0 ? `${hours}h` : `${hours}h ${remainingMinutes}m`;
}

export function buildModelRoundUsageMeta(params: {
  completedAt?: number;
  durationMs?: number;
  tokenUsage?: TokenUsage;
  status?: string;
  formatTime: (timestamp: number) => string;
  formatNumber: (value: number) => string;
  t: TranslationFn;
}): ModelRoundUsageMetaItem[] {
  const { completedAt, durationMs, tokenUsage, status, formatTime, formatNumber, t } = params;
  const items: ModelRoundUsageMetaItem[] = [];

  if (typeof completedAt === 'number') {
    items.push({
      key: 'completed',
      label: status === 'cancelled'
        ? t('modelRound.meta.stopped')
        : t('modelRound.meta.completed'),
      value: formatTime(completedAt),
    });
  }

  if (typeof durationMs === 'number') {
    items.push({
      key: 'duration',
      label: t('modelRound.meta.duration'),
      value: formatElapsedDuration(durationMs),
    });
  }

  if (tokenUsage) {
    const unavailable = t('modelRound.meta.tokensUnavailable');
    items.push({
      key: 'tokens',
      label: t('modelRound.meta.tokens'),
      value: t('modelRound.meta.tokenBreakdown', {
        total: formatNumber(tokenUsage.totalTokens),
        input: formatNumber(tokenUsage.inputTokens),
        output: typeof tokenUsage.outputTokens === 'number'
          ? formatNumber(tokenUsage.outputTokens)
          : unavailable,
      }),
    });
  } else if (status !== 'cancelled') {
    items.push({
      key: 'tokens',
      label: t('modelRound.meta.tokens'),
      value: t('modelRound.meta.tokensUnavailable'),
    });
  }

  return items;
}
