import { describe, expect, it } from 'vitest';
import type { Session, TokenUsage } from '../types/flow-chat';
import {
  buildContextUsageTooltip,
  buildModelRoundUsageMeta,
  formatCompactTokenCount,
  getSessionContextUsageDisplay,
} from './tokenUsageDisplay';

const t = (key: string, params?: Record<string, unknown>): string => {
  const strings: Record<string, string> = {
    'modelSelector.contextUsage.agentPrompt': 'Last request prompt: {{usage}}',
    'modelSelector.contextUsage.acpContext': 'ACP reported context: {{usage}}',
    'modelSelector.contextUsage.toolNote': 'Tool outputs may be summarized or truncated before later requests.',
    'modelRound.meta.completed': 'Completed',
    'modelRound.meta.stopped': 'Stopped',
    'modelRound.meta.duration': 'Duration',
    'modelRound.meta.tokens': 'Tokens',
    'modelRound.meta.tokensUnavailable': 'unavailable',
    'modelRound.meta.tokenBreakdown': '{{total}} total, {{input}} in, {{output}} out',
  };

  const template = strings[key] ?? key;
  return Object.entries(params ?? {}).reduce(
    (text, [paramKey, value]) => text.replace(`{{${paramKey}}}`, String(value)),
    template,
  );
};

const makeSession = (overrides: Partial<Session> = {}): Session => ({
  sessionId: 'session-1',
  title: 'Session',
  dialogTurns: [],
  status: 'idle',
  config: { agentType: 'agentic' },
  createdAt: 1,
  lastActiveAt: 1,
  error: null,
  isHistorical: false,
  todos: [],
  mode: 'agentic',
  workspacePath: 'D:/workspace/BitFun',
  isTransient: false,
  maxContextTokens: 4000,
  ...overrides,
});

describe('tokenUsageDisplay', () => {
  it('uses prompt/input tokens for non-ACP context usage instead of spent total tokens', () => {
    const session = makeSession({
      currentTokenUsage: {
        inputTokens: 1200,
        outputTokens: 300,
        totalTokens: 1500,
        timestamp: 10,
      },
      maxContextTokens: 4000,
    });

    expect(getSessionContextUsageDisplay(session)).toEqual({
      current: 1200,
      max: 4000,
      source: 'agent_prompt',
    });
  });

  it('preserves ACP-reported context usage as its own source', () => {
    const session = makeSession({
      currentAcpContextUsage: {
        used: 42000,
        size: 128000,
        timestamp: 10,
      },
      currentTokenUsage: {
        inputTokens: 1200,
        outputTokens: 300,
        totalTokens: 1500,
        timestamp: 10,
      },
    });

    expect(getSessionContextUsageDisplay(session)).toEqual({
      current: 42000,
      max: 128000,
      source: 'acp_context',
    });
  });

  it('labels the context usage source and tool-output caveat in the tooltip', () => {
    const tooltip = buildContextUsageTooltip({
      baseTooltip: 'Claude Sonnet',
      usage: {
        current: 1200,
        max: 4000,
        source: 'agent_prompt',
      },
      t,
    });

    expect(tooltip).toBe(
      'Claude Sonnet · Last request prompt: 1.2K/4K (30%) · Tool outputs may be summarized or truncated before later requests.',
    );
  });

  it('formats model-round timing and token metadata with unavailable output when missing', () => {
    const tokenUsage: TokenUsage = {
      inputTokens: 1000,
      totalTokens: 1300,
      timestamp: 10,
    };

    expect(buildModelRoundUsageMeta({
      completedAt: 1700000000000,
      durationMs: 12345,
      tokenUsage,
      formatTime: () => '12:00:00',
      formatNumber: (value) => String(value),
      t,
    })).toEqual([
      { key: 'completed', label: 'Completed', value: '12:00:00' },
      { key: 'duration', label: 'Duration', value: '12.3s' },
      { key: 'tokens', label: 'Tokens', value: '1300 total, 1000 in, unavailable out' },
    ]);
  });

  it('omits the token row for cancelled rounds when provider usage is unavailable', () => {
    expect(buildModelRoundUsageMeta({
      completedAt: 1700000000000,
      durationMs: 12345,
      tokenUsage: undefined,
      status: 'cancelled',
      formatTime: () => '12:00:00',
      formatNumber: (value) => String(value),
      t,
    })).toEqual([
      { key: 'completed', label: 'Stopped', value: '12:00:00' },
      { key: 'duration', label: 'Duration', value: '12.3s' },
    ]);
  });

  it('keeps compact token formatting stable for tooltip strings', () => {
    expect(formatCompactTokenCount(950)).toBe('950');
    expect(formatCompactTokenCount(1200)).toBe('1.2K');
    expect(formatCompactTokenCount(4000)).toBe('4K');
  });
});
