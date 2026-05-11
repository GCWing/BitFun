import { describe, expect, it } from 'vitest';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import {
  calculateShare,
  coerceSessionUsageReport,
  getFileSummaryLabel,
  getModelHelp,
  getModelLabel,
  getSlowSpanHelp,
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
      pathLabel: 'D:/workspace/bitfun',
    },
    scope: {
      kind: 'entire_session',
      turnCount: 2,
      includesSubagents: false,
    },
    coverage: {
      level: 'partial',
      available: ['workspace_identity'],
      missing: ['cost_estimates'],
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
  it('only accepts structured usage report metadata', () => {
    expect(coerceSessionUsageReport(usageReport())?.reportId).toBe('usage-session-1');
    expect(coerceSessionUsageReport({ reportId: 'usage-1' })).toBeUndefined();
    expect(coerceSessionUsageReport(null)).toBeUndefined();
  });

  it('does not calculate timing shares when model time is missing', () => {
    expect(calculateShare(undefined, 8_000)).toBeUndefined();
    expect(calculateShare(4_000, 8_000)).toBe(50);
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
    expect(getSlowSpanLabel({
      label: 'gpt-5.4',
      kind: 'model',
      durationMs: 100,
      redacted: false,
      turnIndex: 3,
      modelIdSource: 'inferred_session_model',
    }, t)).toBe('Turn 3 model call');
  });

  it('returns model identity tooltip copy when the source is inferred or legacy', () => {
    expect(getModelHelp('inferred_session_model', t)).toBe('Inferred from the session model setting.');
    expect(getModelHelp('inferred_session_model', t, '019e0c07-c7bc-73f1-b1d6-5260ed215fe0'))
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
});
