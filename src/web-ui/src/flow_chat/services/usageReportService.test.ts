import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import type { FlowChatState, Session } from '../types/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';

const sessionApiMocks = vi.hoisted(() => ({
  getSessionUsageReport: vi.fn(),
  saveSessionTurn: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: sessionApiMocks,
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    warning: vi.fn(),
    error: vi.fn(),
  },
}));

const createSession = (): Session => ({
  sessionId: 'session-1',
  title: 'Session 1',
  dialogTurns: [],
  status: 'idle',
  config: { agentType: 'agentic' },
  createdAt: 1,
  lastActiveAt: 1,
  error: null,
  isHistorical: false,
  todos: [],
  maxContextTokens: 128128,
  mode: 'agentic',
  workspacePath: 'D:/workspace/BitFun',
  isTransient: false,
});

const usageReport = (): SessionUsageReport => ({
  schemaVersion: 1,
  reportId: 'usage-report-1',
  sessionId: 'session-1',
  generatedAt: 100,
  workspace: {
    kind: 'local',
    pathLabel: 'D:/workspace/BitFun',
  },
  scope: {
    kind: 'entire_session',
    turnCount: 1,
    includesSubagents: false,
  },
  coverage: {
    level: 'complete',
    available: [],
    missing: [],
    notes: [],
  },
  time: {
    accounting: 'approximate',
    denominator: 'session_wall_time',
    wallTimeMs: 1000,
  },
  tokens: {
    source: 'token_usage_records',
    inputTokens: 10,
    outputTokens: 5,
    totalTokens: 15,
    cacheCoverage: 'unavailable',
  },
  models: [],
  tools: [],
  files: {
    scope: 'unavailable',
    files: [],
  },
  compression: {
    compactionCount: 0,
    manualCompactionCount: 0,
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
});

describe('runUsageReportCommand', () => {
  beforeEach(() => {
    flowChatStore.setState((): FlowChatState => ({
      sessions: new Map([['session-1', createSession()]]),
      activeSessionId: 'session-1',
    }));
    sessionApiMocks.getSessionUsageReport.mockReset();
    sessionApiMocks.saveSessionTurn.mockReset();
    sessionApiMocks.saveSessionTurn.mockResolvedValue(undefined);
  });

  afterEach(() => {
    flowChatStore.setState((): FlowChatState => ({
      sessions: new Map(),
      activeSessionId: null,
    }));
  });

  it('inserts a loading usage card immediately and replaces it with the final report', async () => {
    let resolveReport: (report: SessionUsageReport) => void = () => {};
    sessionApiMocks.getSessionUsageReport.mockReturnValue(new Promise<SessionUsageReport>(resolve => {
      resolveReport = resolve;
    }));
    const { runUsageReportCommand } = await import('./usageReportService');

    const pending = runUsageReportCommand({
      session: createSession(),
      isProcessing: false,
      busyMessage: 'busy',
      noWorkspaceMessage: 'missing workspace',
      failedTitle: 'failed',
      unknownErrorMessage: 'unknown',
      loadingMarkdown: 'Generating usage report...',
    });

    const loadingTurn = flowChatStore.getState().sessions.get('session-1')?.dialogTurns[0];
    expect(loadingTurn?.userMessage.content).toBe('Generating usage report...');
    expect(loadingTurn?.userMessage.metadata).toMatchObject({
      localCommandKind: 'usage_report',
      usageReportStatus: 'loading',
      modelVisible: false,
    });

    resolveReport(usageReport());
    const result = await pending;

    const finalTurn = flowChatStore.getState().sessions.get('session-1')?.dialogTurns[0];
    expect(result.inserted).toBe(true);
    expect(finalTurn?.id).toBe(loadingTurn?.id);
    expect(finalTurn?.status).toBe('completed');
    expect(finalTurn?.userMessage.content).toContain('# Session Usage Report');
    expect(finalTurn?.userMessage.content).toContain('Session span');
    expect(finalTurn?.userMessage.content).toContain('not reported');
    expect(finalTurn?.userMessage.content).not.toContain('Wall time');
    expect(finalTurn?.userMessage.content).not.toContain('Cached | unavailable');
    expect(finalTurn?.userMessage.metadata).toMatchObject({
      reportId: 'usage-report-1',
      usageReportStatus: 'completed',
    });
    expect(sessionApiMocks.saveSessionTurn).toHaveBeenCalledTimes(1);
  });
});
