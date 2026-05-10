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
  Info,
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
  getRedactedLabel,
  getTopFiles,
  getTopModels,
  getTopTools,
  getToolCategoryLabel,
} from './usageReportUtils';
import './SessionUsageReportCard.scss';

interface SessionUsageReportCardProps {
  report?: SessionUsageReport;
  markdown?: string;
  generatedAt?: number;
  isLoading?: boolean;
  onOpenDetails?: (report: SessionUsageReport) => void;
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

  const topModels = useMemo(() => report ? getTopModels(report, 3) : [], [report]);
  const topTools = useMemo(() => report ? getTopTools(report, 3) : [], [report]);
  const topFiles = useMemo(() => report ? getTopFiles(report, 3) : [], [report]);
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
    },
  ];

  return (
    <div className="session-usage-report-card" data-report-id={report.reportId}>
      <div className="session-usage-report-card__header">
        <div className="session-usage-report-card__title-block">
          <div className="session-usage-report-card__eyebrow">{t('usage.card.eyebrow')}</div>
          <h3 className="session-usage-report-card__title">{t('usage.title')}</h3>
        </div>
        <div className="session-usage-report-card__actions">
          <span className={`session-usage-report-card__coverage session-usage-report-card__coverage--${coverageTone}`}>
            {getCoverageLabel(report.coverage.level, t)}
          </span>
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
          <Tooltip content={t('usage.actions.openDetails')}>
            <IconButton
              variant="ghost"
              size="xs"
              onClick={handleOpenDetails}
              disabled={!onOpenDetails}
              aria-label={t('usage.actions.openDetails')}
            >
              <Info size={14} />
            </IconButton>
          </Tooltip>
        </div>
      </div>

      <div className="session-usage-report-card__meta">
        <span>{formatUsageTimestamp(generatedAt ?? report.generatedAt, t)}</span>
        <span>{t('usage.card.turns', { count: report.scope.turnCount })}</span>
        <span>{report.workspace.pathLabel || t('usage.unavailable')}</span>
      </div>

      {report.coverage.level !== 'complete' && (
        <div className="session-usage-report-card__notice">
          <AlertTriangle size={14} aria-hidden />
          <span>{t('usage.coverage.partialNotice')}</span>
        </div>
      )}

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
          items={topModels.map(model => ({
            label: model.modelId,
            value: formatUsageNumber(model.totalTokens, t),
            detail: t('usage.card.calls', { count: model.callCount }),
          }))}
          emptyLabel={t('usage.empty.models')}
          emptyDescription={t('usage.empty.modelsDescription')}
        />
        <UsageMiniList
          title={t('usage.sections.tools')}
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
          items={topFiles.map(file => ({
            label: file.redacted ? getRedactedLabel(t) : file.pathLabel,
            value: t('usage.card.operations', { count: file.operationCount }),
            detail: `${formatUsageNumber(file.addedLines, t)} / ${formatUsageNumber(file.deletedLines, t)}`,
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

interface UsageMiniListProps {
  title: string;
  items: Array<{
    label: string;
    value: string;
    detail: string;
  }>;
  emptyLabel: string;
  emptyDescription?: string;
}

function UsageMiniList({ title, items, emptyLabel, emptyDescription }: UsageMiniListProps) {
  return (
    <div className="session-usage-report-card__mini-list">
      <div className="session-usage-report-card__mini-list-title">{title}</div>
      {items.length === 0 ? (
        <div className="session-usage-report-card__mini-list-empty">
          <strong>{emptyLabel}</strong>
          {emptyDescription && <span>{emptyDescription}</span>}
        </div>
      ) : (
        items.map(item => (
          <div className="session-usage-report-card__mini-list-row" key={`${item.label}-${item.value}`}>
            <span className="session-usage-report-card__mini-list-label">{item.label}</span>
            <span className="session-usage-report-card__mini-list-value">{item.value}</span>
            <span className="session-usage-report-card__mini-list-detail">{item.detail}</span>
          </div>
        ))
      )}
    </div>
  );
}

SessionUsageReportCard.displayName = 'SessionUsageReportCard';
