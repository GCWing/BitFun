import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  AlertTriangle,
  Check,
  ChevronRight,
  Copy,
  Clock3,
  Database,
  FileText,
} from 'lucide-react';
import { IconButton, MarkdownRenderer, ToolProcessingDots, Tooltip } from '@/component-library';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import {
  formatUsageDuration,
  formatUsageNumber,
  formatUsageTimestamp,
  getCoverageLabel,
  getCoverageTone,
  getFileScopeHelp,
  getFileSummaryLabel,
  getModelHelp,
  getModelLabel,
  getRedactedLabel,
  getTopFiles,
  getTopModels,
  getTopTools,
  getToolCategoryLabel,
  getUsageFileNameFromPath,
} from './usageReportUtils';
import type { SessionUsagePanelTab } from './sessionUsagePanelTypes';
import './SessionUsageReportCard.scss';

const SUMMARY_LIST_LIMIT = 3;

interface SessionUsageReportCardProps {
  report?: SessionUsageReport;
  markdown?: string;
  generatedAt?: number;
  isLoading?: boolean;
  onOpenDetails?: (report: SessionUsageReport, initialTab?: SessionUsagePanelTab) => void;
}

export const SessionUsageReportCard: React.FC<SessionUsageReportCardProps> = ({
  report,
  markdown = '',
  generatedAt,
  isLoading = false,
  onOpenDetails,
}) => {
  const { t } = useTranslation('flow-chat');
  const [copied, setCopied] = useState(false);
  const [loadingStep, setLoadingStep] = useState(0);

  const handleCopy = useCallback(async (event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  }, [markdown]);

  const handleOpenDetails = useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    if (report) {
      onOpenDetails?.(report);
    }
  }, [onOpenDetails, report]);

  const handleOpenSectionDetails = useCallback((initialTab: SessionUsagePanelTab) => (event: React.MouseEvent) => {
    event.stopPropagation();
    if (report) {
      onOpenDetails?.(report, initialTab);
    }
  }, [onOpenDetails, report]);

  const topModels = useMemo(() => report ? getTopModels(report, SUMMARY_LIST_LIMIT) : [], [report]);
  const topTools = useMemo(() => report ? getTopTools(report, SUMMARY_LIST_LIMIT) : [], [report]);
  const topFiles = useMemo(() => report ? getTopFiles(report, SUMMARY_LIST_LIMIT) : [], [report]);
  const loadingHints = useMemo(() => [
    t('usage.loading.steps.collecting'),
    t('usage.loading.steps.tokens'),
    t('usage.loading.steps.safety'),
  ], [t]);

  useEffect(() => {
    if (!isLoading || loadingHints.length <= 1) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      setLoadingStep(step => (step + 1) % loadingHints.length);
    }, 1600);

    return () => window.clearInterval(timer);
  }, [isLoading, loadingHints.length]);

  if (isLoading) {
    return (
      <div className="session-usage-report-card session-usage-report-card--loading" aria-live="polite">
        <div className="session-usage-report-card__loading-main">
          <ToolProcessingDots className="session-usage-report-card__loading-dots" size={12} />
          <div>
            <h3 className="session-usage-report-card__loading-title">{t('usage.loading.title')}</h3>
            <p className="session-usage-report-card__loading-description">{t('usage.loading.description')}</p>
          </div>
        </div>
        <div className="session-usage-report-card__loading-step">
          {loadingHints[loadingStep] ?? loadingHints[0]}
        </div>
      </div>
    );
  }

  if (!report) {
    return (
      <div className="session-usage-report-card session-usage-report-card--fallback">
        <div className="session-usage-report-card__fallback-actions">
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
  const tokenTotal = report.tokens.totalTokens;
  const cachedTokenText = report.tokens.cacheCoverage === 'unavailable'
    ? t('usage.status.cacheNotReported')
    : formatUsageNumber(report.tokens.cachedTokens, t);
  const cachedTokenHelp = report.tokens.cacheCoverage === 'unavailable'
    ? t('usage.help.cachedTokens')
    : report.tokens.cacheCoverage === 'partial'
      ? t('usage.help.cachedTokensPartial')
    : undefined;
  const fileMetricHelp = getFileScopeHelp(report, t);

  const metrics = [
    {
      key: 'wall',
      label: t('usage.metrics.wall'),
      value: formatUsageDuration(report.time.wallTimeMs, t),
      icon: Clock3,
      help: t('usage.help.wall'),
    },
    {
      key: 'active',
      label: t('usage.metrics.active'),
      value: formatUsageDuration(report.time.activeTurnMs, t),
      icon: Activity,
      help: t('usage.help.active'),
    },
    {
      key: 'tokens',
      label: t('usage.metrics.tokens'),
      value: formatUsageNumber(tokenTotal, t),
      icon: Database,
    },
    {
      key: 'cached',
      label: t('usage.metrics.cached'),
      value: cachedTokenText,
      icon: Database,
      help: cachedTokenHelp,
    },
    {
      key: 'files',
      label: t('usage.metrics.files'),
      value: getFileSummaryLabel(report, t),
      icon: FileText,
      help: fileMetricHelp,
    },
    {
      key: 'errors',
      label: t('usage.metrics.errors'),
      value: formatUsageNumber(report.errors.totalErrors, t),
      icon: AlertTriangle,
      tone: report.errors.totalErrors > 0 ? 'warning' : undefined,
      help: t('usage.help.errors'),
    },
  ];

  const coverageBadgeClassName =
    `session-usage-report-card__coverage session-usage-report-card__coverage--${coverageTone}` +
    (report.coverage.level !== 'complete' ? ' session-usage-report-card__coverage--hint' : '');

  return (
    <div className="session-usage-report-card" data-report-id={report.reportId}>
      <div className="session-usage-report-card__header">
        <div className="session-usage-report-card__title-block">
          <h3 className="session-usage-report-card__title">{t('usage.card.heading')}</h3>
          <div className="session-usage-report-card__meta">
            <span>{formatUsageTimestamp(generatedAt ?? report.generatedAt, t)}</span>
            <span>{t('usage.card.turns', { count: report.scope.turnCount })}</span>
            <span>{report.workspace.pathLabel || t('usage.unavailable')}</span>
          </div>
        </div>
        <div className="session-usage-report-card__actions">
          {report.coverage.level !== 'complete' ? (
            <Tooltip content={t('usage.coverage.partialNotice')} placement="top">
              <span className={coverageBadgeClassName}>
                {getCoverageLabel(report.coverage.level, t)}
              </span>
            </Tooltip>
          ) : (
            <span className={coverageBadgeClassName}>
              {getCoverageLabel(report.coverage.level, t)}
            </span>
          )}
          <div className="session-usage-report-card__header-actions">
            <Tooltip content={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}>
              <IconButton
                className="session-usage-report-card__copy-action"
                variant="ghost"
                size="xs"
                onClick={handleCopy}
                aria-label={copied ? t('usage.actions.copied') : t('usage.actions.copyMarkdown')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </IconButton>
            </Tooltip>
            <Tooltip content={t('usage.actions.openDetails')}>
              <button
                type="button"
                className="session-usage-report-card__details-button"
                onClick={handleOpenDetails}
                disabled={!onOpenDetails}
                aria-label={t('usage.actions.openDetails')}
              >
                <span>{t('usage.actions.viewDetails')}</span>
                <ChevronRight size={13} aria-hidden />
              </button>
            </Tooltip>
          </div>
        </div>
      </div>

      <div className="session-usage-report-card__metrics">
        {metrics.map(metric => {
          const Icon = metric.icon;
          return (
            <div
              className={`session-usage-report-card__metric${metric.tone ? ` session-usage-report-card__metric--${metric.tone}` : ''}`}
              key={metric.key}
            >
              <Icon size={14} aria-hidden />
              <span className="session-usage-report-card__metric-label">{metric.label}</span>
              <UsageMetricValue value={metric.value} help={metric.help} />
            </div>
          );
        })}
      </div>

      <div className="session-usage-report-card__lists">
        <UsageMiniList
          title={t('usage.sections.models')}
          showAll={buildShowAllAction({
            totalCount: report.models.length,
            visibleCount: topModels.length,
            sectionLabel: t('usage.sections.models'),
            t,
            onClick: onOpenDetails ? handleOpenSectionDetails('models') : undefined,
          })}
          items={topModels.map(model => {
            const source = model.modelIdSource ?? (model.modelId === 'unknown_model' ? 'legacy_missing' : undefined);
            const help = getModelHelp(source, t, model.modelId);
            const label = getModelLabel(model.modelId, t, source);
            const tokenValue = typeof model.totalTokens === 'number' && Number.isFinite(model.totalTokens)
              ? t('usage.card.tokens', { value: formatUsageNumber(model.totalTokens, t) })
              : formatUsageNumber(model.totalTokens, t);
            return {
              label: help ? { value: label, help } : label,
              value: tokenValue,
              detail: t('usage.card.calls', { count: model.callCount }),
            };
          })}
          emptyLabel={t('usage.empty.models')}
          emptyDescription={t('usage.empty.modelsDescription')}
        />
        <UsageMiniList
          title={t('usage.sections.tools')}
          showAll={buildShowAllAction({
            totalCount: report.tools.length,
            visibleCount: topTools.length,
            sectionLabel: t('usage.sections.tools'),
            t,
            onClick: onOpenDetails ? handleOpenSectionDetails('tools') : undefined,
          })}
          items={topTools.map(tool => ({
            label: tool.redacted ? getRedactedLabel(t) : tool.toolName,
            value: t('usage.card.calls', { count: tool.callCount }),
            detail: getToolCategoryLabel(tool.category, t),
          }))}
          emptyLabel={t('usage.empty.tools')}
          emptyDescription={t('usage.empty.toolsDescription')}
        />
        <UsageMiniList
          title={t('usage.sections.files')}
          showAll={buildShowAllAction({
            totalCount: report.files.files.length,
            visibleCount: topFiles.length,
            sectionLabel: t('usage.sections.files'),
            t,
            onClick: onOpenDetails ? handleOpenSectionDetails('files') : undefined,
          })}
          items={topFiles.map(file => ({
            label: file.redacted
              ? getRedactedLabel(t)
              : {
                node: <UsageMiniListFilePathLabel pathLabel={file.pathLabel} />,
                text: file.pathLabel,
                help: file.pathLabel,
              },
            value: t('usage.card.operations', { count: file.operationCount }),
            detail: (
              <UsageFileChangeDetail
                addedLines={file.addedLines}
                deletedLines={file.deletedLines}
                t={t}
              />
            ),
          }))}
          emptyLabel={getFileSummaryLabel(report, t)}
          emptyDescription={fileMetricHelp ?? t('usage.empty.filesDescription')}
        />
      </div>
    </div>
  );
};

function UsageMetricValue({ value, help }: { value: string; help?: string }) {
  const node = (
    <span className={`session-usage-report-card__metric-value${help ? ' session-usage-report-card__metric-value--help' : ''}`}>
      {value}
    </span>
  );

  return help ? <Tooltip content={help}>{node}</Tooltip> : node;
}

type UsageMiniListLabel = string | {
  value: string;
  help?: string;
} | {
  node: React.ReactElement;
  text: string;
  help?: string;
};

type UsageMiniListShowAll = {
  label: string;
  ariaLabel: string;
  onClick: (event: React.MouseEvent) => void;
};

interface UsageMiniListProps {
  title: string;
  showAll?: UsageMiniListShowAll;
  items: Array<{
    label: UsageMiniListLabel;
    value: string;
    detail: React.ReactNode;
  }>;
  emptyLabel: string;
  emptyDescription?: string;
}

function buildShowAllAction({
  totalCount,
  visibleCount,
  sectionLabel,
  t,
  onClick,
}: {
  totalCount: number;
  visibleCount: number;
  sectionLabel: string;
  t: (key: string, options?: Record<string, unknown>) => string;
  onClick?: (event: React.MouseEvent) => void;
}): UsageMiniListShowAll | undefined {
  if (!onClick || totalCount <= visibleCount) {
    return undefined;
  }
  return {
    label: t('usage.actions.viewAllSection', { count: totalCount }),
    ariaLabel: t('usage.actions.openSectionDetails', { section: sectionLabel }),
    onClick,
  };
}

function getMiniListLabelText(label: UsageMiniListLabel): string {
  if (typeof label !== 'string' && 'node' in label) {
    return label.text;
  }
  return typeof label === 'string' ? label : label.value;
}

function UsageMiniListLabelView({ label }: { label: UsageMiniListLabel }) {
  if (typeof label !== 'string' && 'node' in label) {
    return label.help
      ? <Tooltip content={label.help}>{label.node}</Tooltip>
      : label.node;
  }

  const labelText = getMiniListLabelText(label);
  const node = (
    <span className={`session-usage-report-card__mini-list-label${typeof label !== 'string' && label.help ? ' session-usage-report-card__mini-list-label--help' : ''}`}>
      {labelText}
    </span>
  );

  return typeof label !== 'string' && label.help
    ? <Tooltip content={label.help}>{node}</Tooltip>
    : node;
}

function UsageMiniListFilePathLabel({ pathLabel }: { pathLabel: string }) {
  return (
    <span className="session-usage-report-card__mini-list-file-name">
      {getUsageFileNameFromPath(pathLabel)}
    </span>
  );
}

function UsageFileChangeDetail({
  addedLines,
  deletedLines,
  t,
}: {
  addedLines?: number;
  deletedLines?: number;
  t: (key: string, options?: Record<string, unknown>) => string;
}) {
  return (
    <span
      className="session-usage-report-card__file-stat"
      aria-label={`${t('usage.table.added')}: ${formatUsageNumber(addedLines, t)}, ${t('usage.table.deleted')}: ${formatUsageNumber(deletedLines, t)}`}
    >
      <span className="session-usage-report-card__file-stat--added">
        {formatSignedFileLineCount(addedLines, '+', t)}
      </span>
      <span className="session-usage-report-card__file-stat-separator">/</span>
      <span className="session-usage-report-card__file-stat--deleted">
        {formatSignedFileLineCount(deletedLines, '-', t)}
      </span>
    </span>
  );
}

function formatSignedFileLineCount(
  value: number | undefined,
  sign: '+' | '-',
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  const formatted = formatUsageNumber(value, t);
  return typeof value === 'number' && Number.isFinite(value) ? `${sign}${formatted}` : formatted;
}

function UsageMiniList({ title, showAll, items, emptyLabel, emptyDescription }: UsageMiniListProps) {
  return (
    <div className="session-usage-report-card__mini-list">
      <div className="session-usage-report-card__mini-list-header">
        <div className="session-usage-report-card__mini-list-title">{title}</div>
        {showAll && (
          <Tooltip content={showAll.ariaLabel}>
            <button
              type="button"
              className="session-usage-report-card__mini-list-more"
              onClick={showAll.onClick}
              aria-label={showAll.ariaLabel}
            >
              <span>{showAll.label}</span>
              <ChevronRight size={12} aria-hidden />
            </button>
          </Tooltip>
        )}
      </div>
      {items.length === 0 ? (
        <div className="session-usage-report-card__mini-list-empty">
          <strong>{emptyLabel}</strong>
          {emptyDescription && <span>{emptyDescription}</span>}
        </div>
      ) : (
        items.map(item => (
          <div className="session-usage-report-card__mini-list-row" key={`${getMiniListLabelText(item.label)}-${item.value}`}>
            <UsageMiniListLabelView label={item.label} />
            <span className="session-usage-report-card__mini-list-value">{item.value}</span>
            <span className="session-usage-report-card__mini-list-detail">{item.detail}</span>
          </div>
        ))
      )}
    </div>
  );
}

SessionUsageReportCard.displayName = 'SessionUsageReportCard';
