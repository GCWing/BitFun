import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import { notificationService } from '@/shared/notification-system';
import type { DialogTurnData } from '@/shared/types/session-history';
import { flowChatStore } from '../store/FlowChatStore';
import type { DialogTurn, Session } from '../types/flow-chat';

export interface UsageReportCommandParams {
  session: Session;
  isProcessing: boolean;
  busyMessage: string;
  noWorkspaceMessage: string;
  failedTitle: string;
  unknownErrorMessage: string;
  loadingMarkdown: string;
}

export interface UsageReportCommandResult {
  inserted: boolean;
  reason?: 'busy' | 'missing_workspace';
  report?: SessionUsageReport;
}

export async function runUsageReportCommand(
  params: UsageReportCommandParams
): Promise<UsageReportCommandResult> {
  if (params.isProcessing) {
    notificationService.warning(params.busyMessage);
    return { inserted: false, reason: 'busy' };
  }

  if (!params.session.workspacePath) {
    notificationService.error(params.noWorkspaceMessage);
    return { inserted: false, reason: 'missing_workspace' };
  }

  const requestedAt = Date.now();
  const pendingReportId = `pending-${params.session.sessionId}-${requestedAt}`;
  const pendingTurn = flowChatStore.addLocalUsageReportTurn({
    sessionId: params.session.sessionId,
    markdown: params.loadingMarkdown,
    reportId: pendingReportId,
    schemaVersion: 1,
    generatedAt: requestedAt,
    status: 'loading',
  });
  let finalizedPendingTurn = false;

  try {
    const report = await sessionAPI.getSessionUsageReport({
      sessionId: params.session.sessionId,
      workspacePath: params.session.workspacePath,
      remoteConnectionId: params.session.remoteConnectionId,
      remoteSshHost: params.session.remoteSshHost,
    });
    const markdown = renderUsageReportMarkdown(report);
    const turn = pendingTurn
      ? updatePendingUsageReportTurn({
        sessionId: params.session.sessionId,
        dialogTurnId: pendingTurn.id,
        markdown,
        report,
      })
      : flowChatStore.addLocalUsageReportTurn({
        sessionId: params.session.sessionId,
        markdown,
        reportId: report.reportId,
        schemaVersion: report.schemaVersion,
        generatedAt: report.generatedAt,
        report: report as unknown as Record<string, any>,
      });
    finalizedPendingTurn = !!pendingTurn;

    if (turn) {
      await sessionAPI.saveSessionTurn(
        toPersistedLocalReportTurn(turn),
        params.session.workspacePath,
        params.session.remoteConnectionId,
        params.session.remoteSshHost,
      );
    }

    return { inserted: !!turn, report };
  } catch (error) {
    if (pendingTurn && !finalizedPendingTurn) {
      flowChatStore.deleteDialogTurn(params.session.sessionId, pendingTurn.id);
    }
    notificationService.error(
      error instanceof Error ? error.message : params.unknownErrorMessage,
      {
        title: params.failedTitle,
        duration: 5000,
      }
    );
    throw error;
  }
}

function updatePendingUsageReportTurn(params: {
  sessionId: string;
  dialogTurnId: string;
  markdown: string;
  report: SessionUsageReport;
}): DialogTurn | null {
  flowChatStore.updateDialogTurn(params.sessionId, params.dialogTurnId, turn => ({
    ...turn,
    status: 'completed',
    userMessage: {
      ...turn.userMessage,
      content: params.markdown,
      timestamp: params.report.generatedAt,
      metadata: {
        ...turn.userMessage.metadata,
        reportId: params.report.reportId,
        schemaVersion: params.report.schemaVersion,
        generatedAt: params.report.generatedAt,
        usageReportStatus: 'completed',
        usageReport: params.report as unknown as Record<string, any>,
      },
    },
    startTime: params.report.generatedAt,
    endTime: params.report.generatedAt,
  }));

  return flowChatStore.getState().sessions
    .get(params.sessionId)
    ?.dialogTurns.find(turn => turn.id === params.dialogTurnId) ?? null;
}

export function renderUsageReportMarkdown(report: SessionUsageReport): string {
  const lines: string[] = [
    '# Session Usage Report',
    '',
    `- Report: \`${escapeMarkdown(report.reportId)}\``,
    `- Session: \`${escapeMarkdown(report.sessionId)}\``,
    `- Workspace: ${escapeMarkdown(report.workspace.pathLabel || 'unavailable')}`,
    `- Scope: ${report.scope.turnCount} turns${report.scope.includesSubagents ? ', including subagents' : ''}`,
    `- Coverage: ${report.coverage.level}`,
    '',
  ];

  if (report.coverage.level !== 'complete') {
    lines.push('> Partial coverage: some metrics depend on provider or tool metadata that was not recorded for this session. Those fields are marked not reported instead of zero.', '');
  }

  lines.push(
    '## Time',
    '',
    '| Metric | Value |',
    '| --- | --- |',
    `| Accounting | ${report.time.accounting} |`,
    `| Session span | ${formatDuration(report.time.wallTimeMs)} |`,
    `| Recorded turn time | ${formatDuration(report.time.activeTurnMs)} |`,
    `| Model round time | ${formatDuration(report.time.modelMs)} |`,
    `| Tool call time | ${formatDuration(report.time.toolMs)} |`,
    '',
    '## Tokens',
    '',
    '| Metric | Value |',
    '| --- | --- |',
    `| Source | ${report.tokens.source} |`,
    `| Input | ${formatNumber(report.tokens.inputTokens)} |`,
    `| Output | ${formatNumber(report.tokens.outputTokens)} |`,
    `| Total | ${formatNumber(report.tokens.totalTokens)} |`,
    `| Cached | ${report.tokens.cacheCoverage === 'unavailable' ? 'not reported' : formatNumber(report.tokens.cachedTokens)} |`,
    '',
  );

  if (report.tools.length > 0) {
    lines.push(
      '## Tools',
      '',
      '| Tool | Calls | Success | Errors | Recorded time |',
      '| --- | ---: | ---: | ---: | --- |',
      ...report.tools.map(tool =>
        `| ${tool.redacted ? 'redacted' : escapeMarkdown(tool.toolName)} | ${tool.callCount} | ${tool.successCount} | ${tool.errorCount} | ${formatDuration(tool.durationMs)} |`
      ),
      '',
    );
  }

  lines.push(
    '## Files',
    '',
    `- Changed files: ${formatNumber(report.files.changedFiles)}`,
    `- Added lines: ${formatNumber(report.files.addedLines)}`,
    `- Deleted lines: ${formatNumber(report.files.deletedLines)}`,
    '',
  );

  if (report.slowest.length > 0) {
    lines.push(
      '## Slowest Spans',
      '',
      '| Label | Kind | Duration |',
      '| --- | --- | --- |',
      ...report.slowest.map(span =>
        `| ${span.redacted ? 'redacted' : escapeMarkdown(span.label)} | ${span.kind} | ${formatDuration(span.durationMs)} |`
      ),
      '',
    );
  }

  if (report.coverage.missing.length > 0) {
    lines.push(
      '## Coverage Gaps',
      '',
      ...report.coverage.missing.map(key => `- \`${key}\``),
      '',
    );
  }

  lines.push(
    '## Privacy',
    '',
    '- Prompt content included: no',
    '- Tool inputs included: no',
    '- Command outputs included: no',
    '- File contents included: no',
  );

  return lines.join('\n');
}

function toPersistedLocalReportTurn(turn: DialogTurn): DialogTurnData {
  return {
    turnId: turn.id,
    turnIndex: turn.backendTurnIndex ?? 0,
    sessionId: turn.sessionId,
    timestamp: turn.startTime,
    kind: 'local_command',
    userMessage: {
      id: turn.userMessage.id,
      content: turn.userMessage.content,
      timestamp: turn.userMessage.timestamp,
      metadata: turn.userMessage.metadata,
    },
    modelRounds: [],
    startTime: turn.startTime,
    endTime: turn.endTime,
    durationMs: 0,
    status: 'completed',
  };
}

function formatNumber(value: number | undefined): string {
  return value === undefined ? 'unavailable' : String(value);
}

function formatDuration(value: number | undefined): string {
  if (value === undefined) {
    return 'unavailable';
  }
  if (value < 1000) {
    return `${value}ms`;
  }
  const seconds = Math.floor(value / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) {
    return remainingSeconds === 0 ? `${minutes}m` : `${minutes}m ${remainingSeconds}s`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes === 0 ? `${hours}h` : `${hours}h ${remainingMinutes}m`;
}

function escapeMarkdown(value: string): string {
  return value.replace(/\|/g, '\\|');
}
