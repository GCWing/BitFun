import { describe, expect, it } from 'vitest';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import {
  calculateShare,
  coerceSessionUsageReport,
  formatHitRatePercent,
  formatHitRateSuffix,
  formatTokenCount,
  getFileSummaryLabel,
  getModelHelp,
  getModelLabel,
  getSlowSpanHelp,
  getDisplayTurnIndex,
  getSlowSpanLabel,
  getTopFiles,
} from './usageReportUtils';

const t = (key: string, options?: Record<string, unknown>): string => {
  if (key === 'usage.unavailable') return 'Unavailable';
  if (key === 'usage.percent') return `${options?.value}%`;
  if (key === 'usage.duration.seconds') return `${options?.value}s`;
  if (key === 'usage.status.noFileChanges') return 'No file changes';
  if (key === 'usage.status.modelNotRecorded') return 'Model not recorded';
  if (key === 'usage.status.legacyModel') return 'Legacy model not tracked';
  if (key === 'usage.status.inferredModel') return `${options?.model} (inferred)`;
  if (key === 'usage.help.legacyModel') return 'Older sessions did not store per-round model names.';
  if (key === 'usage.help.inferredModel') return 'Inferred from the session model setting.';
  if (key === 'usage.help.slowestModelCall') return `Model call: ${options?.model}`;
  if (key === 'usage.slowestLabels.modelCall') return `Turn ${options?.turn} model call`;
  if (key === 'usage.slowestLabels.modelCallUnknown') return 'Model call';
  if (key === 'usage.slowestLabels.turn') return `Turn ${options?.turn}`;
  if (key === 'usage.slowestLabels.turnUnknown') return 'Turn';
  if (key === 'usage.redacted') return 'Redacted';
  return key;
};

function usageReport(overrides: Partial<SessionUsageReport> = {}): SessionUsageReport {
  return {
    schemaVersion: 1,
    reportId: 'usage-session-1',
    sessionId: 'session-1',
    generatedAt: 1_778_347_200_000,
    workspace: {
      kind: 'local',
      pathLabel: 'D:/workspace/openbitfun',
    },
    scope: {
      kind: 'entire_session',
      turnCount: 2,
      includesSubagents: false,
    },
    coverage: {
      level: 'partial',
      available: ['workspace_identity'],
      missing: ['token_detail_breakdown'],
      notes: [],
    },
    time: {
      accounting: 'approximate',
      denominator: 'session_wall_time',
      wallTimeMs: 10_000,
      activeTurnMs: 8_000,
      modelMs: 4_000,
      toolMs: 2_000,
    },
    tokens: {
      source: 'token_usage_records',
      inputTokens: 100,
      outputTokens: 50,
      totalTokens: 150,
      cacheCoverage: 'unavailable',
    },
    models: [],
    tools: [],
    files: {
      scope: 'snapshot_summary',
      changedFiles: 2,
      addedLines: 13,
      deletedLines: 3,
      files: [
        {
          pathLabel: 'src/small.ts',
          operationCount: 4,
          addedLines: 1,
          deletedLines: 1,
          redacted: false,
        },
        {
          pathLabel: 'src/large.ts',
          operationCount: 1,
          addedLines: 10,
          deletedLines: 2,
          redacted: false,
        },
      ],
    },
    compression: {
      compactionCount: 1,
      manualCompactionCount: 1,
      automaticCompactionCount: 0,
    },
    errors: {
      totalErrors: 0,
      toolErrors: 0,
      modelErrors: 0,
      examples: [],
    },
    slowest: [],
    privacy: {
      promptContentIncluded: false,
      toolInputsIncluded: false,
      commandOutputsIncluded: false,
      fileContentsIncluded: false,
      redactedFields: [],
    },
    ...overrides,
  };
}

describe('usageReportUtils', () => {
  it('formats token counts with compact units', () => {
    expect(formatTokenCount(999, t)).toBe('999');
    expect(formatTokenCount(1_234, t)).toBe('1.23K');
    expect(formatTokenCount(12_345, t)).toBe('12.35K');
    expect(formatTokenCount(1_000_000_000, t)).toBe('1B');
  });

  it('only accepts structured usage report metadata', () => {
    expect(coerceSessionUsageReport(usageReport())?.reportId).toBe('usage-session-1');
    expect(coerceSessionUsageReport({ reportId: 'usage-1' })).toBeUndefined();
    expect(coerceSessionUsageReport(null)).toBeUndefined();
  });

  it('does not calculate timing shares when model time is missing', () => {
    expect(calculateShare(undefined, 8_000)).toBeUndefined();
    expect(calculateShare(4_000, 8_000)).toBe(50);
    expect(calculateShare(12_000, 8_000)).toBe(150);
  });

  it('labels empty file activity as no file changes', () => {
    const label = getFileSummaryLabel(usageReport({
      files: {
        scope: 'unavailable',
        changedFiles: undefined,
        addedLines: undefined,
        deletedLines: undefined,
        files: [],
      },
    }), t);

    expect(label).toBe('No file changes');
  });

  it('orders file rows by changed lines before operation count', () => {
    const topFiles = getTopFiles(usageReport(), 2);

    expect(topFiles.map(file => file.pathLabel)).toEqual([
      'src/large.ts',
      'src/small.ts',
    ]);
  });

  it('labels legacy and inferred model identities with helpful copy', () => {
    expect(getModelLabel('unknown_model', t, 'legacy_missing')).toBe('Legacy model not tracked');
    expect(getModelLabel('model round 0', t)).toBe('Legacy model not tracked');
    expect(getModelLabel('gpt-5.4', t, 'inferred_session_model')).toBe('gpt-5.4 (inferred)');
    expect(getModelLabel('019e0c07-c7bc-73f1-b1d6-5260ed215fe0', t, 'inferred_session_model'))
      .toBe('Legacy model not tracked');
    expect(getModelLabel('model_1780555920188_0', t, 'inferred_session_model'))
      .toBe('Legacy model not tracked');
    expect(getSlowSpanLabel({
      label: 'gpt-5.4',
      kind: 'model',
      durationMs: 100,
      redacted: false,
      turnIndex: 3,
      modelIdSource: 'inferred_session_model',
    }, t)).toBe('Turn 4 model call');
  });

  it('numbers a slow span the way its own jump does', () => {
    /*
     * The report indexes Turns from zero and every reader adds one. This label
     * did not, so a row read "Turn 7" while clicking it asked for Turn 8 — the
     * Turn the rail, numbering the same way, also calls 8. Label and
     * destination have to come from the same function, so here they do.
     */
    for (const rawTurnIndex of [0, 3, 7]) {
      expect(getSlowSpanLabel({
        label: 'gpt-5.4',
        kind: 'model',
        durationMs: 100,
        redacted: false,
        turnIndex: rawTurnIndex,
      }, t)).toBe(`Turn ${getDisplayTurnIndex(rawTurnIndex)} model call`);
    }
    // And the offset itself: zero-based in, one-based out, never negative.
    expect(getDisplayTurnIndex(0)).toBe(1);
    expect(getDisplayTurnIndex(7)).toBe(8);
    expect(getDisplayTurnIndex(-4)).toBe(1);
  });

  it('builds a Turn span label rather than rendering the report\'s own', () => {
    /*
     * The report synthesises this one itself — `format!("turn {}", turn_index)`
     * — so it arrives hard-coded in English and numbered from zero. Rendered
     * verbatim it read "turn 7" next to a row this file had numbered 8.
     */
    const span = {
      label: 'turn 7',
      kind: 'turn' as const,
      durationMs: 100,
      redacted: false,
      turnIndex: 7,
    };
    expect(getSlowSpanLabel(span, t)).toBe('Turn 8');
    expect(getSlowSpanLabel({ ...span, turnIndex: undefined }, t)).toBe('Turn');
  });

  it('returns model identity tooltip copy when the source is inferred or legacy', () => {
    expect(getModelHelp('inferred_session_model', t)).toBe('Inferred from the session model setting.');
    expect(getModelHelp('inferred_session_model', t, '019e0c07-c7bc-73f1-b1d6-5260ed215fe0'))
      .toBe('Older sessions did not store per-round model names.');
    expect(getModelHelp('inferred_session_model', t, 'model_1780555920188_0'))
      .toBe('Older sessions did not store per-round model names.');
    expect(getModelHelp('legacy_missing', t)).toBe('Older sessions did not store per-round model names.');
    expect(getModelHelp(undefined, t, 'model round 0')).toBe('Older sessions did not store per-round model names.');
    expect(getSlowSpanHelp({
      label: 'unknown_model',
      kind: 'model',
      durationMs: 100,
      redacted: false,
      modelIdSource: 'legacy_missing',
    }, t)).toBe('Model call: Legacy model not tracked Older sessions did not store per-round model names.');
    expect(getSlowSpanHelp({
      label: 'model round 1',
      kind: 'model',
      durationMs: 100,
      redacted: false,
    }, t)).toBe('Model call: Legacy model not tracked Older sessions did not store per-round model names.');
    expect(getSlowSpanHelp({
      label: 'gpt-5.4',
      kind: 'model',
      durationMs: 100,
      redacted: false,
    }, t)).toBe('Model call: gpt-5.4');
    expect(getSlowSpanLabel({
      label: 'secret',
      kind: 'tool',
      durationMs: 100,
      redacted: true,
    }, t)).toBe('Redacted');
  });

  describe('formatHitRateSuffix', () => {
    it('formats a finite ratio as a parenthesised percentage with leading space', () => {
      expect(formatHitRateSuffix(0.8, t)).toBe(' (80.00%)');
      expect(formatHitRateSuffix(0, t)).toBe(' (0.00%)');
      expect(formatHitRateSuffix(1, t)).toBe(' (100.00%)');
    });

    it('returns an empty string for missing or non-finite values', () => {
      expect(formatHitRateSuffix(undefined, t)).toBe('');
      expect(formatHitRateSuffix(null, t)).toBe('');
      expect(formatHitRateSuffix(Number.NaN, t)).toBe('');
      expect(formatHitRateSuffix(Number.POSITIVE_INFINITY, t)).toBe('');
    });

    it('rounds the displayed percent down to two decimal places', () => {
      expect(formatHitRateSuffix(0.80404, t)).toBe(' (80.40%)');
      expect(formatHitRateSuffix(0.80409, t)).toBe(' (80.40%)');
    });
  });

  describe('formatHitRatePercent', () => {
    it('formats a finite ratio as a bare percentage cell', () => {
      expect(formatHitRatePercent(0.5, t)).toBe('50.00%');
      expect(formatHitRatePercent(1, t)).toBe('100.00%');
    });

    it('returns a dash for missing or non-finite values', () => {
      expect(formatHitRatePercent(undefined, t)).toBe('-');
      expect(formatHitRatePercent(null, t)).toBe('-');
      expect(formatHitRatePercent(Number.NaN, t)).toBe('-');
    });
  });
});
