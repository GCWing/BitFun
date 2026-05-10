import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  AlertTriangle,
  Check,
  Copy,
  Clock3,
  Database,
  FileText,
  ShieldCheck,
  Wrench,
} from 'lucide-react';
import { IconButton, MarkdownRenderer, Tooltip } from '@/component-library';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
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
  getRedactedLabel,
  getToolCategoryLabel,
} from './usageReportUtils';
import './SessionUsagePanel.scss';

type UsagePanelTab = 'overview' | 'models' | 'tools' | 'files' | 'errors';

interface SessionUsagePanelProps {
  report?: SessionUsageReport;
  markdown?: string;
  sessionId?: string;
  workspacePath?: string;
}

const TABS: UsagePanelTab[] = ['overview', 'models', 'tools', 'files', 'errors'];

export const SessionUsagePanel: React.FC<SessionUsagePanelProps> = ({
  report,
  markdown = '',
  sessionId,
  workspacePath,
}) => {
  const { t } = useTranslation('flow-chat');
  const [activeTab, setActiveTab] = useState<UsagePanelTab>('overview');
  const [copied, setCopied] = useState(false);
  const [copiedMeta, setCopiedMeta] = useState<'session' | 'workspace' | null>(null);

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

  return (
    <div className="session-usage-panel">
      <header className="session-usage-panel__header">
        <div className="session-usage-panel__title-wrap">
          <span className={`session-usage-panel__badge session-usage-panel__badge--${coverageTone}`}>
            {getCoverageLabel(report.coverage.level, t)}
          </span>
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
        {activeTab === 'files' && <UsageFiles report={report} />}
        {activeTab === 'errors' && <UsageErrors report={report} />}
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

type UsageTableCell = string | {
  value: string;
  help?: string;
};

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
  return (
    <UsageTable
      empty={report.models.length === 0}
      emptyLabel={t('usage.empty.models')}
      emptyDescription={t('usage.empty.modelsDescription')}
      headers={[
        t('usage.table.model'),
        t('usage.table.calls'),
        t('usage.table.input'),
        t('usage.table.output'),
        t('usage.table.cached'),
      ]}
      rows={report.models.map(model => {
        const cached = formatUsageNumber(model.cachedTokens, t);
        return [
          model.modelId,
          formatUsageNumber(model.callCount, t),
          formatUsageNumber(model.inputTokens, t),
          formatUsageNumber(model.outputTokens, t),
          report.tokens.cacheCoverage === 'unavailable'
            ? { value: t('usage.status.cacheNotReported'), help: t('usage.help.cachedTokens') }
            : cached,
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
          tool.durationMs === undefined ? { value: t('usage.status.timingNotRecorded'), help: t('usage.help.toolDuration') } : duration,
          tool.p95DurationMs === undefined ? { value: t('usage.status.timingNotRecorded'), help: t('usage.help.toolP95') } : p95,
          tool.executionMs === undefined ? { value: t('usage.status.timingNotRecorded'), help: t('usage.help.toolExecution') } : execution,
        ];
      })}
    />
  );
}

function UsageFiles({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  const fileScopeHelp = getFileScopeHelp(report, t);
  const rows = useMemo(() => report.files.files.map(file => [
    file.redacted ? getRedactedLabel(t) : file.pathLabel,
    formatUsageNumber(file.operationCount, t),
    formatUsageNumber(file.addedLines, t),
    formatUsageNumber(file.deletedLines, t),
    (file.turnIndexes ?? []).join(', ') || { value: t('usage.status.notRecorded'), help: t('usage.help.fileTurnIndexes') },
    (file.operationIds ?? []).slice(0, 3).join(', ') || { value: t('usage.status.notRecorded'), help: t('usage.help.fileOperationIds') },
  ]), [report.files.files, t]);

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
          t('usage.table.operationIds'),
        ]}
        rows={rows}
      />
    </section>
  );
}

function UsageErrors({ report }: { report: SessionUsageReport }) {
  const { t } = useTranslation('flow-chat');
  return (
    <section className="session-usage-panel__section">
      <dl className="session-usage-panel__definition-list">
        <div>
          <dt>{t('usage.metrics.errors')}</dt>
          <dd>{formatUsageNumber(report.errors.totalErrors, t)}</dd>
        </div>
        <div>
          <dt>{t('usage.panel.toolErrors')}</dt>
          <dd>{formatUsageNumber(report.errors.toolErrors, t)}</dd>
        </div>
        <div>
          <dt>{t('usage.panel.modelErrors')}</dt>
          <dd>{formatUsageNumber(report.errors.modelErrors, t)}</dd>
        </div>
      </dl>
      <UsageTable
        empty={report.errors.examples.length === 0}
        emptyLabel={t('usage.empty.errors')}
        emptyDescription={t('usage.empty.errorsDescription')}
        headers={[t('usage.table.label'), t('usage.table.count')]}
        rows={report.errors.examples.map(example => [
          example.redacted ? getRedactedLabel(t) : example.label,
          formatUsageNumber(example.count, t),
        ])}
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
}

function UsageTable({ empty, emptyLabel, emptyDescription, emptyHelp, headers, rows }: UsageTableProps) {
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
      <table className="session-usage-panel__table">
        <thead>
          <tr>
            {headers.map(header => <th key={header}>{header}</th>)}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {row.map((cell, cellIndex) => (
                <td key={`${rowIndex}-${cellIndex}`}>
                  {typeof cell === 'string'
                    ? <span>{cell}</span>
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
