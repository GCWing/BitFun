import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  AlertTriangle,
  Check,
  Copy,
  Clock3,
  Database,
  FileText,
  GitCompare,
  ShieldCheck,
  Wrench,
} from 'lucide-react';
import { IconButton, MarkdownRenderer, Tooltip } from '@/component-library';
import { snapshotAPI } from '@/infrastructure/api';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import { globalEventBus } from '@/infrastructure/event-bus';
import { createDiffEditorTab } from '@/shared/utils/tabUtils';
import { createLogger } from '@/shared/utils/logger';
import {
  FLOWCHAT_PIN_TURN_TO_TOP_EVENT,
  type FlowChatPinTurnToTopRequest,
} from '../../events/flowchatNavigation';
import {
  calculateShare,
  formatUsageDuration,
  formatUsageNumber,
  formatUsagePercent,
  formatUsageTimestamp,
  getAccountingLabel,
  getCoverageLabel,
  getCoverageTone,
  getFileScopeHelp,
  getFileScopeLabel,
  getFileSummaryLabel,
  getModelHelp,
  getModelLabel,
  getRedactedLabel,
  getSlowSpanHelp,
  getSlowSpanLabel,
  getToolCategoryLabel,
} from './usageReportUtils';
import type { SessionUsagePanelTab } from './sessionUsagePanelTypes';
import './SessionUsagePanel.scss';

const log = createLogger('SessionUsagePanel');
interface SessionUsagePanelProps {
  report?: SessionUsageReport;
  markdown?: string;
  sessionId?: string;
  workspacePath?: string;
  initialTab?: SessionUsagePanelTab;
}

const TABS: SessionUsagePanelTab[] = ['overview', 'models', 'tools', 'files', 'errors', 'slowest'];

export const SessionUsagePanel: React.FC<SessionUsagePanelProps> = ({
  report,
  markdown = '',
  sessionId,
  workspacePath,
  initialTab,
}) => {
  const { t } = useTranslation('flow-chat');
  const [activeTab, setActiveTab] = useState<SessionUsagePanelTab>(initialTab ?? 'overview');
  const [copied, setCopied] = useState(false);
  const [copiedMeta, setCopiedMeta] = useState<'session' | 'workspace' | null>(null);

  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  }, [markdown]);

  const handleCopyMeta = useCallback(async (
    value: string,
    field: 'session' | 'workspace'
  ) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedMeta(field);
      window.setTimeout(() => setCopiedMeta(null), 1800);
    } catch {
      setCopiedMeta(null);
    }
  }, []);

  if (!report) {
    return (
      <div className="session-usage-panel session-usage-panel--fallback">
        <div className="session-usage-panel__fallback-toolbar">
          <Tooltip content={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}>
            <IconButton
              variant="ghost"
              size="xs"
              onClick={handleCopy}
              aria-label={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </IconButton>
          </Tooltip>
        </div>
        <MarkdownRenderer content={markdown} />
      </div>
    );
  }

  const coverageTone = getCoverageTone(report.coverage.level);
  const effectiveSessionId = sessionId ?? report.sessionId;
  const effectiveWorkspacePath = workspacePath ?? report.workspace.pathLabel ?? t('usage.unavailable');
  const coverageBadgeClassName =
    `session-usage-panel__badge session-usage-panel__badge--${coverageTone}` +
    (report.coverage.level !== 'complete' ? ' session-usage-panel__badge--hint' : '');
  const coverageBadge = (
    <span className={coverageBadgeClassName}>
      {getCoverageLabel(report.coverage.level, t)}
    </span>
  );

  return (
    <div className="session-usage-panel">
      <header className="session-usage-panel__header">
        <div className="session-usage-panel__title-wrap">
          <div className="session-usage-panel__title-main">
            <h2>{t('usage.title')}</h2>
            <div className="session-usage-panel__meta-list" aria-label={t('usage.panel.metadataLabel')}>
              <UsageMetaRow
                label={t('usage.meta.generatedAt')}
                value={formatUsageTimestamp(report.generatedAt, t)}
              />
              <UsageMetaRow
                label={t('usage.meta.sessionId')}
                value={effectiveSessionId}
                copyLabel={copiedMeta === 'session' ? t('usage.actions.copied') : t('usage.actions.copySessionId')}
                copied={copiedMeta === 'session'}
                onCopy={() => handleCopyMeta(effectiveSessionId, 'session')}
              />
              <UsageMetaRow
                label={t('usage.meta.workspacePath')}
                value={effectiveWorkspacePath}
                copyLabel={copiedMeta === 'workspace' ? t('usage.actions.copied') : t('usage.actions.copyWorkspacePath')}
                copied={copiedMeta === 'workspace'}
                onCopy={() => handleCopyMeta(effectiveWorkspacePath, 'workspace')}
              />
            </div>
          </div>
        </div>
        <div className="session-usage-panel__header-actions">
          {report.coverage.level !== 'complete' ? (
            <Tooltip content={t('usage.coverage.partialNotice')} placement="top">
              {coverageBadge}
            </Tooltip>
          ) : coverageBadge}
          <Tooltip content={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}>
            <IconButton
              className="session-usage-panel__copy"
              variant="ghost"
              size="xs"
              onClick={handleCopy}
              aria-label={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </IconButton>
          </Tooltip>
        </div>
      </header>

      <nav className="session-usage-panel__tabs" aria-label={t('usage.panel.tabsLabel')}>
        {TABS.map(tab => (
          <button
            key={tab}
            type="button"
            className={`session-usage-panel__tab${activeTab === tab ? ' session-usage-panel__tab--active' : ''}`}
            onClick={() => setActiveTab(tab)}
          >
            {t(`usage.tabs.${tab}`)}
          </button>
        ))}
      </nav>

      <main className="session-usage-panel__body">
        {activeTab === 'overview' && <UsageOverview report={report} />}
        {activeTab === 'models' && <UsageModels report={report} />}
        {activeTab === 'tools' && <UsageTools report={report} />}
        {activeTab === 'files' && (
          <UsageFiles
            report={report}
            sessionId={effectiveSessionId}
            workspacePath={workspacePath ?? report.workspace.pathLabel}
          />
        )}
        {activeTab === 'errors' && <UsageErrors report={report} />}
        {activeTab === 'slowest' && <UsageSlowest report={report} sessionId={effectiveSessionId} />}
      </main>
    </div>
  );
};

function UsageMetaRow({
  label,
  value,
  copyLabel,
  copied = false,
  onCopy,
}: {
  label: string;
  value: string;
  copyLabel?: string;
  copied?: boolean;
  onCopy?: () => void;
}) {
  return (
    <div className="session-usage-panel__meta-row">
      <span className="session-usage-panel__meta-label">{label}</span>
      <span className="session-usage-panel__meta-value" title={value}>{value}</span>
      {onCopy && copyLabel && (
        <Tooltip content={copyLabel}>
          <IconButton
            className="session-usage-panel__meta-copy"
            variant="ghost"
            size="xs"
            onClick={onCopy}
            aria-label={copyLabel}
          >
            {copied ? <Check size={13} /> : <Copy size={13} />}
          </IconButton>
        </Tooltip>
      )}
    </div>
  );
}

type UsageTableValueCell = {
  value: string;
  help?: string;
  className?: string;
};

type UsageTableNodeCell = {
  node: React.ReactNode;
  className?: string;
};

type UsageTableCell = string | UsageTableValueCell | UsageTableNodeCell;

function UsageValue({
  value,
  help,
  strong = false,
}: {
  value: string;
  help?: string;
  strong?: boolean;
}) {
  const className = help ? 'session-usage-panel__help-value' : undefined;
  const node = strong
    ? <strong className={className}>{value}</strong>
    : <span className={className}>{value}</span>;

  return help ? <Tooltip content={help}>{node}</Tooltip> : node;
}

function UsageMissingValue({ value, help }: { value: string; help?: string }) {
  const node = (
    <span className={`session-usage-panel__missing-value${help ? ' session-usage-panel__missing-value--help' : ''}`}>
      {value}
    </span>
  );

  return help ? <Tooltip content={help}>{node}</Tooltip> : node;
}

function missingUsageValue(value: string, help?: string): UsageTableCell {
  return {
    className: 'session-usage-panel__missing-cell',
    node: <UsageMissingValue value={value} help={help} />,
  };
}

function UsageFilePathValue({ pathLabel }: { pathLabel: string }) {
  const node = (
    <span className="session-usage-panel__file-path-display">
      {getCompactFilePathLabel(pathLabel)}
    </span>
  );

  return <Tooltip content={pathLabel}>{node}</Tooltip>;
}

function getCompactFilePathLabel(pathLabel: string): string {
  const normalizedPath = pathLabel.replace(/\\/g, '/');
  const segments = normalizedPath.split('/').filter(Boolean);

  if (segments.length <= 2) {
    return normalizedPath;
  }

  return `.../${segments.slice(-2).join('/')}`;
}

function UsageOverview({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  const denominator = report.time.activeTurnMs ?? report.time.wallTimeMs;
  const fileScopeHelp = getFileScopeHelp(report, t);
  const cacheCoverageHelp = report.tokens.cacheCoverage === 'unavailable'
    ? t('usage.help.cachedTokens')
    : report.tokens.cacheCoverage === 'partial'
      ? t('usage.help.cachedTokensPartial')
      : undefined;
  const metrics = [
    {
      key: 'wall',
      icon: Clock3,
      label: t('usage.metrics.wall'),
      value: formatUsageDuration(report.time.wallTimeMs, t),
      help: t('usage.help.wall'),
    },
    {
      key: 'active',
      icon: Activity,
      label: t('usage.metrics.active'),
      value: formatUsageDuration(report.time.activeTurnMs, t),
      help: t('usage.help.active'),
    },
    {
      key: 'model',
      icon: Database,
      label: t('usage.metrics.modelTime'),
      value: formatUsageDuration(report.time.modelMs, t),
      detail: formatUsagePercent(calculateShare(report.time.modelMs, denominator), t),
      help: t('usage.help.modelRoundTime'),
    },
    {
      key: 'tool',
      icon: Wrench,
      label: t('usage.metrics.toolTime'),
      value: formatUsageDuration(report.time.toolMs, t),
      detail: formatUsagePercent(calculateShare(report.time.toolMs, denominator), t),
      help: t('usage.help.toolTime'),
    },
    {
      key: 'tokens',
      icon: Database,
      label: t('usage.metrics.tokens'),
      value: formatUsageNumber(report.tokens.totalTokens, t),
    },
    {
      key: 'files',
      icon: FileText,
      label: t('usage.metrics.files'),
      value: getFileSummaryLabel(report, t),
      detail: getFileScopeLabel(report.files.scope, t),
      help: fileScopeHelp,
    },
  ];

  return (
    <section className="session-usage-panel__section">
      {report.coverage.level !== 'complete' && (
        <div className="session-usage-panel__notice">
          <AlertTriangle size={14} aria-hidden />
          <span>{t('usage.coverage.partialNotice')}</span>
        </div>
      )}

      <div className="session-usage-panel__overview-grid">
        {metrics.map(metric => {
          const Icon = metric.icon;
          return (
            <div className="session-usage-panel__overview-metric" key={metric.key}>
              <Icon size={16} aria-hidden />
              <div>
                <span>{metric.label}</span>
                <UsageValue value={metric.value} help={metric.help} strong />
                {metric.detail && <em>{metric.detail}</em>}
              </div>
            </div>
          );
        })}
      </div>

      <dl className="session-usage-panel__definition-list">
        <div>
          <dt>{t('usage.panel.accounting')}</dt>
          <dd>{getAccountingLabel(report.time.accounting, t)}</dd>
        </div>
        <div>
          <dt>{t('usage.panel.turnScope')}</dt>
          <dd>{t('usage.card.turns', { count: report.scope.turnCount })}</dd>
        </div>
        <div>
          <dt>{t('usage.panel.cacheCoverage')}</dt>
          <dd>
            <UsageValue
              value={t(`usage.cacheCoverage.${report.tokens.cacheCoverage}`)}
              help={cacheCoverageHelp}
            />
          </dd>
        </div>
        <div>
          <dt>{t('usage.panel.compressions')}</dt>
          <dd>{formatUsageNumber(report.compression.compactionCount, t)}</dd>
        </div>
      </dl>

      <div className="session-usage-panel__privacy">
        <ShieldCheck size={16} aria-hidden />
        <div>
          <strong>{t('usage.privacy.title')}</strong>
          <span>{t('usage.privacy.summary')}</span>
        </div>
      </div>
    </section>
  );
}

function UsageModels({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  const hasModelDuration = report.models.some(model => model.durationMs !== undefined);
  const headers = [
    t('usage.table.model'),
    t('usage.table.calls'),
    ...(hasModelDuration ? [t('usage.table.duration')] : []),
    t('usage.table.input'),
    t('usage.table.output'),
    t('usage.table.cached'),
  ];
  return (
    <UsageTable
      empty={report.models.length === 0}
      emptyLabel={t('usage.empty.models')}
      emptyDescription={t('usage.empty.modelsDescription')}
      headers={headers}
      rows={report.models.map(model => {
        const cached = formatUsageNumber(model.cachedTokens, t);
        const source = model.modelIdSource ?? (model.modelId === 'unknown_model' ? 'legacy_missing' : undefined);
        const modelHelp = getModelHelp(source, t, model.modelId);
        const row: UsageTableCell[] = [
          modelHelp
            ? { value: getModelLabel(model.modelId, t, source), help: modelHelp }
            : getModelLabel(model.modelId, t, source),
          formatUsageNumber(model.callCount, t),
        ];
        if (hasModelDuration) {
          row.push(
            model.durationMs === undefined
              ? missingUsageValue(t('usage.status.timingNotRecorded'), t('usage.help.modelRoundTime'))
              : formatUsageDuration(model.durationMs, t)
          );
        }
        row.push(
          formatUsageNumber(model.inputTokens, t),
          formatUsageNumber(model.outputTokens, t),
          report.tokens.cacheCoverage === 'unavailable'
            ? { value: t('usage.status.cacheNotReported'), help: t('usage.help.cachedTokens') }
            : cached,
        );
        return [
          ...row,
        ];
      })}
    />
  );
}

function UsageTools({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  return (
    <UsageTable
      empty={report.tools.length === 0}
      emptyLabel={t('usage.empty.tools')}
      emptyDescription={t('usage.empty.toolsDescription')}
      headers={[
        t('usage.table.tool'),
        t('usage.table.category'),
        t('usage.table.calls'),
        t('usage.table.success'),
        t('usage.table.errors'),
        t('usage.table.duration'),
        t('usage.table.p95'),
        t('usage.table.execution'),
      ]}
      rows={report.tools.map(tool => {
        const duration = formatUsageDuration(tool.durationMs, t);
        const p95 = formatUsageDuration(tool.p95DurationMs, t);
        const execution = formatUsageDuration(tool.executionMs, t);
        return [
          tool.redacted ? getRedactedLabel(t) : tool.toolName,
          getToolCategoryLabel(tool.category, t),
          formatUsageNumber(tool.callCount, t),
          formatUsageNumber(tool.successCount, t),
          formatUsageNumber(tool.errorCount, t),
          tool.durationMs === undefined
            ? missingUsageValue(t('usage.status.timingNotRecorded'), t('usage.help.toolDuration'))
            : duration,
          tool.p95DurationMs === undefined
            ? missingUsageValue(t('usage.status.timingNotRecorded'), t('usage.help.toolP95'))
            : p95,
          tool.executionMs === undefined
            ? missingUsageValue(t('usage.status.timingNotRecorded'), t('usage.help.toolExecution'))
            : execution,
        ];
      })}
    />
  );
}

function UsageFiles({
  report,
  sessionId,
  workspacePath,
}: {
  report: SessionUsageReport;
  sessionId?: string;
  workspacePath?: string;
}) {
  const { t } = useTranslation('flow-chat');
  const fileScopeHelp = getFileScopeHelp(report, t);
  const [openingDiffKey, setOpeningDiffKey] = useState<string | null>(null);

  const handleOpenFileDiff = useCallback(async (file: SessionUsageReport['files']['files'][number]) => {
    if (!sessionId || file.redacted || report.files.scope !== 'snapshot_summary') {
      return;
    }

    const resolvedPath = resolveUsageFilePath(file.pathLabel, workspacePath);
    const operationId = file.operationIds?.[0];
    const diffKey = `${resolvedPath}:${operationId ?? ''}`;
    setOpeningDiffKey(diffKey);

    try {
      const diff = await snapshotAPI.getOperationDiff(
        sessionId,
        resolvedPath,
        operationId,
        workspacePath,
      );
      const diffPath = diff.filePath || resolvedPath;
      createDiffEditorTab(
        diffPath,
        getUsageFileName(diffPath),
        diff.originalContent || '',
        diff.modifiedContent || '',
        true,
        'agent',
        workspacePath,
        diff.anchorLine ? Number(diff.anchorLine) : undefined,
        undefined,
        {
          titleKind: 'diff',
          duplicateKeyPrefix: 'diff',
        },
      );
    } catch (error) {
      log.warn('Failed to open usage report file diff', {
        sessionId,
        filePath: resolvedPath,
        operationId,
        error,
      });
    } finally {
      setOpeningDiffKey(null);
    }
  }, [report.files.scope, sessionId, workspacePath]);

  const rows = useMemo(() => report.files.files.map(file => {
    const operationId = file.operationIds?.[0];
    const resolvedPath = resolveUsageFilePath(file.pathLabel, workspacePath);
    const diffKey = `${resolvedPath}:${operationId ?? ''}`;
    const canOpenDiff = canOpenUsageFileDiff(report.files.scope, file, sessionId);
    const actionCell: UsageTableNodeCell = {
      className: 'session-usage-panel__sticky-action-cell',
      node: canOpenDiff ? (
        <Tooltip content={t('usage.actions.openFileDiff')}>
          <IconButton
            className="session-usage-panel__table-action"
            variant="ghost"
            size="xs"
            onClick={() => void handleOpenFileDiff(file)}
            disabled={openingDiffKey === diffKey}
            aria-label={t('usage.actions.openFileDiff')}
          >
            <GitCompare size={13} />
          </IconButton>
        </Tooltip>
      ) : (
        <Tooltip content={t('usage.help.fileDiffUnavailable')}>
          <span className="session-usage-panel__table-action-placeholder">-</span>
        </Tooltip>
      ),
    };

    return [
      file.redacted
        ? getRedactedLabel(t)
        : {
          node: <UsageFilePathValue pathLabel={file.pathLabel} />,
          className: 'session-usage-panel__file-path-cell',
        },
      formatUsageNumber(file.operationCount, t),
      formatUsageNumber(file.addedLines, t),
      formatUsageNumber(file.deletedLines, t),
      (file.turnIndexes ?? []).join(', ') || { value: t('usage.status.notRecorded'), help: t('usage.help.fileTurnIndexes') },
      actionCell,
    ];
  }), [handleOpenFileDiff, openingDiffKey, report.files.files, report.files.scope, sessionId, t, workspacePath]);

  return (
    <section className="session-usage-panel__section">
      <div className="session-usage-panel__scope-line">
        <span>{t('usage.panel.fileScope')}</span>
        <UsageValue
          value={report.files.scope === 'unavailable' ? getFileSummaryLabel(report, t) : getFileScopeLabel(report.files.scope, t)}
          help={fileScopeHelp}
          strong
        />
      </div>
      <UsageTable
        empty={report.files.files.length === 0}
        emptyLabel={getFileSummaryLabel(report, t)}
        emptyDescription={fileScopeHelp ?? t('usage.empty.filesDescription')}
        headers={[
          t('usage.table.file'),
          t('usage.table.operations'),
          t('usage.table.added'),
          t('usage.table.deleted'),
          t('usage.table.turns'),
          t('usage.table.actions'),
        ]}
        rows={rows}
        tableClassName="session-usage-panel__table--files"
      />
    </section>
  );
}

function UsageErrors({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  return (
    <section className="session-usage-panel__section">
      <div className="session-usage-panel__scope-line">
        <span>{t('usage.panel.errorScope')}</span>
        <UsageValue
          value={formatUsageNumber(report.errors.totalErrors, t)}
          help={t('usage.help.errors')}
          strong
        />
      </div>
      <p className="session-usage-panel__section-help">{t('usage.help.errors')}</p>
      <dl className="session-usage-panel__definition-list">
        <div>
          <dt>{t('usage.metrics.errors')}</dt>
          <dd>
            <UsageValue
              value={formatUsageNumber(report.errors.totalErrors, t)}
              help={t('usage.help.errors')}
            />
          </dd>
        </div>
        <div>
          <dt>{t('usage.panel.toolErrors')}</dt>
          <dd>
            <UsageValue
              value={formatUsageNumber(report.errors.toolErrors, t)}
              help={t('usage.help.toolErrors')}
            />
          </dd>
        </div>
        <div>
          <dt>{t('usage.panel.modelErrors')}</dt>
          <dd>
            <UsageValue
              value={formatUsageNumber(report.errors.modelErrors, t)}
              help={t('usage.help.modelErrors')}
            />
          </dd>
        </div>
      </dl>
      <UsageTable
        empty={report.errors.examples.length === 0}
        emptyLabel={t('usage.empty.errors')}
        emptyDescription={t('usage.empty.errorsDescription')}
        emptyHelp={t('usage.help.errorExamples')}
        headers={[t('usage.table.label'), t('usage.table.count')]}
        rows={report.errors.examples.map(example => [
          {
            value: example.redacted ? getRedactedLabel(t) : example.label,
            help: t('usage.help.errorExampleRow'),
          },
          {
            value: formatUsageNumber(example.count, t),
            help: t('usage.help.errorExampleCount'),
          },
        ])}
      />
    </section>
  );
}

function canOpenUsageFileDiff(
  scope: SessionUsageReport['files']['scope'],
  file: SessionUsageReport['files']['files'][number],
  sessionId?: string,
): boolean {
  return Boolean(sessionId && scope === 'snapshot_summary' && !file.redacted && file.pathLabel);
}

function resolveUsageFilePath(pathLabel: string, workspacePath?: string): string {
  if (!workspacePath || isAbsolutePathLike(pathLabel)) {
    return pathLabel;
  }
  return `${workspacePath.replace(/[\\/]+$/, '')}/${pathLabel.replace(/^[\\/]+/, '')}`;
}

function isAbsolutePathLike(value: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(value) || value.startsWith('/') || value.startsWith('\\\\');
}

function getUsageFileName(filePath: string): string {
  return filePath.split(/[\\/]/).pop() || filePath;
}

function UsageSlowest({ report, sessionId }: { report: SessionUsageReport; sessionId?: string }) {
  const { t } = useTranslation('flow-chat');
  const handleJumpToTurn = useCallback((span: SessionUsageReport['slowest'][number]) => {
    if (!sessionId || !span.turnId) return;

    const request: FlowChatPinTurnToTopRequest = {
      sessionId,
      turnId: span.turnId,
      behavior: 'smooth',
      pinMode: 'transient',
      source: 'usage-report',
    };
    globalEventBus.emit(FLOWCHAT_PIN_TURN_TO_TOP_EVENT, request, 'SessionUsagePanel');
  }, [sessionId]);

  return (
    <section className="session-usage-panel__section">
      <div className="session-usage-panel__scope-line">
        <span>{t('usage.sections.slowest')}</span>
        <UsageValue
          value={formatUsageNumber(report.slowest.length, t)}
          help={t('usage.help.slowestSpans')}
          strong
        />
      </div>
      <p className="session-usage-panel__section-help">{t('usage.help.slowestSpans')}</p>
      <UsageTable
        empty={report.slowest.length === 0}
        emptyLabel={t('usage.empty.slowest')}
        emptyDescription={t('usage.empty.slowestDescription')}
        emptyHelp={t('usage.help.slowestSpans')}
        headers={[
          t('usage.table.label'),
          t('usage.table.kind'),
          t('usage.table.duration'),
        ]}
        rows={report.slowest.map(span => {
          const spanHelp = getSlowSpanHelp(span, t);
          const spanLabel = getSlowSpanLabel(span, t);
          const canJumpToTurn = Boolean(sessionId && span.turnId);
          const jumpHelp = t('usage.actions.jumpToTurn');
          const labelCell: UsageTableCell = canJumpToTurn
            ? {
              node: (
                <Tooltip content={spanHelp ? `${spanHelp} ${jumpHelp}` : jumpHelp}>
                  <button
                    type="button"
                    className="session-usage-panel__turn-link"
                    onClick={() => handleJumpToTurn(span)}
                    aria-label={`${jumpHelp}: ${spanLabel}`}
                  >
                    {spanLabel}
                  </button>
                </Tooltip>
              ),
            }
            : spanHelp
              ? { value: spanLabel, help: spanHelp }
              : spanLabel;
          return [
            labelCell,
            t(`usage.slowestKinds.${span.kind === 'model' ? 'modelCall' : span.kind}`),
            formatUsageDuration(span.durationMs, t),
          ];
        })}
      />
    </section>
  );
}

interface UsageTableProps {
  empty: boolean;
  emptyLabel: string;
  emptyDescription?: string;
  emptyHelp?: string;
  headers: string[];
  rows: UsageTableCell[][];
  tableClassName?: string;
}

function UsageTable({ empty, emptyLabel, emptyDescription, emptyHelp, headers, rows, tableClassName }: UsageTableProps) {
  if (empty) {
    return (
      <div className="session-usage-panel__empty">
        <UsageValue value={emptyLabel} help={emptyHelp} strong />
        {emptyDescription && <span>{emptyDescription}</span>}
      </div>
    );
  }

  return (
    <div className="session-usage-panel__table-wrap">
      <table className={['session-usage-panel__table', tableClassName].filter(Boolean).join(' ')}>
        <thead>
          <tr>
            {headers.map(header => <th key={header}>{header}</th>)}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {row.map((cell, cellIndex) => (
                <td
                  key={`${rowIndex}-${cellIndex}`}
                  className={typeof cell === 'string' ? undefined : cell.className}
                >
                  {typeof cell === 'string'
                    ? <span>{cell}</span>
                    : 'node' in cell
                      ? cell.node
                      : <UsageValue value={cell.value} help={cell.help} />}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

SessionUsagePanel.displayName = 'SessionUsagePanel';
