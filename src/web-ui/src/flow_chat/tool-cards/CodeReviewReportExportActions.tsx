import React, { useCallback, useMemo, useState } from 'react';
import { Check, Copy, Download, FilePenLine, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Tooltip } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { createMarkdownEditorTab } from '@/shared/utils/tabUtils';
import {
  formatCodeReviewReportMarkdown,
  type CodeReviewReportData,
  type CodeReviewReportMarkdownLabels,
} from '../utils/codeReviewReport';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

interface CodeReviewReportExportActionsProps {
  reviewData: CodeReviewReportData;
  runManifest?: ReviewTeamRunManifest;
  actions?: CodeReviewReportExportAction[];
  variant?: 'icon' | 'footer';
}

type CodeReviewReportExportAction = 'copy' | 'open' | 'save';

const DEFAULT_EXPORT_ACTIONS: CodeReviewReportExportAction[] = ['copy', 'open', 'save'];

function timestampForFileName(): string {
  return new Date()
    .toISOString()
    .replace(/[:.]/g, '-')
    .replace('T', '_')
    .slice(0, 19);
}

function isTauriDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

function downloadMarkdownInBrowser(fileName: string, markdown: string): void {
  const blob = new Blob([markdown], { type: 'text/markdown;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

export const CodeReviewReportExportActions: React.FC<CodeReviewReportExportActionsProps> = ({
  reviewData,
  runManifest,
  actions = DEFAULT_EXPORT_ACTIONS,
  variant = 'icon',
}) => {
  const { t } = useTranslation('flow-chat');
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);
  const visibleActions = useMemo(() => new Set(actions), [actions]);

  const markdownLabels = useMemo<Partial<CodeReviewReportMarkdownLabels>>(() => ({
    titleStandard: t('toolCards.codeReview.report.titleStandard'),
    titleDeep: t('toolCards.codeReview.report.titleDeep'),
    executiveSummary: t('toolCards.codeReview.sections.summary'),
    reviewDecision: t('toolCards.codeReview.report.reviewDecision'),
    runManifest: t('toolCards.codeReview.sections.runManifest'),
    riskLevel: t('toolCards.codeReview.riskLevel'),
    recommendedAction: t('toolCards.codeReview.recommendedAction'),
    scope: t('toolCards.codeReview.reviewScope'),
    target: t('toolCards.codeReview.runManifest.target'),
    budget: t('toolCards.codeReview.runManifest.budget'),
    estimatedCalls: t('toolCards.codeReview.runManifest.estimatedCalls'),
    activeReviewers: t('toolCards.codeReview.runManifest.activeGroupTitle'),
    skippedReviewers: t('toolCards.codeReview.runManifest.skippedGroupTitle'),
    issues: t('toolCards.codeReview.sections.issues'),
    noIssues: t('toolCards.codeReview.report.noIssues'),
    remediationPlan: t('toolCards.codeReview.sections.remediation'),
    strengths: t('toolCards.codeReview.sections.strengths'),
    reviewTeam: t('toolCards.codeReview.sections.team'),
    reliabilitySignals: t('toolCards.codeReview.report.reliabilitySignals'),
    coverageNotes: t('toolCards.codeReview.sections.coverage'),
    status: t('toolCards.codeReview.report.status'),
    packet: t('toolCards.codeReview.report.packet'),
    partialOutput: t('toolCards.codeReview.report.partialOutput'),
    findings: t('toolCards.codeReview.report.findings'),
    validation: t('toolCards.codeReview.report.validation'),
    suggestion: t('toolCards.codeReview.suggestion'),
    source: t('toolCards.codeReview.report.source'),
    noItems: t('toolCards.codeReview.report.noItems'),
    groupTitles: {
      must_fix: t('toolCards.codeReview.groups.must_fix'),
      should_improve: t('toolCards.codeReview.groups.should_improve'),
      needs_decision: t('toolCards.codeReview.groups.needs_decision'),
      verification: t('toolCards.codeReview.groups.verification'),
      architecture: t('toolCards.codeReview.groups.architecture'),
      maintainability: t('toolCards.codeReview.groups.maintainability'),
      tests: t('toolCards.codeReview.groups.tests'),
      security: t('toolCards.codeReview.groups.security'),
      performance: t('toolCards.codeReview.groups.performance'),
      user_experience: t('toolCards.codeReview.groups.user_experience'),
      other: t('toolCards.codeReview.groups.other'),
    },
    reliabilityNoticeLabels: {
      context_pressure: t('toolCards.codeReview.reliabilityStatus.context_pressure.label'),
      compression_preserved: t('toolCards.codeReview.reliabilityStatus.compression_preserved.label'),
      cache_hit: t('toolCards.codeReview.reliabilityStatus.cache_hit.label'),
      cache_miss: t('toolCards.codeReview.reliabilityStatus.cache_miss.label'),
      concurrency_limited: t('toolCards.codeReview.reliabilityStatus.concurrency_limited.label'),
      partial_reviewer: t('toolCards.codeReview.reliabilityStatus.partial_reviewer.label'),
      reduced_scope: t('toolCards.codeReview.reliabilityStatus.reduced_scope.label'),
      retry_guidance: t('toolCards.codeReview.reliabilityStatus.retry_guidance.label'),
      skipped_reviewers: t('toolCards.codeReview.reliabilityStatus.skipped_reviewers.label'),
      token_budget_limited: t('toolCards.codeReview.reliabilityStatus.token_budget_limited.label'),
      user_decision: t('toolCards.codeReview.reliabilityStatus.user_decision.label'),
    },
  }), [t]);

  const markdown = useMemo(
    () => formatCodeReviewReportMarkdown(
      reviewData,
      markdownLabels,
      { runManifest },
    ),
    [markdownLabels, reviewData, runManifest],
  );

  const fileName = useMemo(() => {
    const prefix = t('toolCards.codeReview.export.fileNamePrefix');
    return `${prefix}_${timestampForFileName()}.md`;
  }, [t]);

  const handleCopy = useCallback(async (event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
      notificationService.success(t('toolCards.codeReview.export.copySuccess'));
    } catch {
      notificationService.error(t('toolCards.codeReview.export.copyFailed'));
    }
  }, [markdown, t]);

  const handleOpenInEditor = useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      createMarkdownEditorTab(
        t('toolCards.codeReview.export.editorTitle'),
        markdown,
        undefined,
        undefined,
        'agent',
      );
    } catch {
      notificationService.error(t('toolCards.codeReview.export.openFailed'));
    }
  }, [markdown, t]);

  const handleSave = useCallback(async (event: React.MouseEvent) => {
    event.stopPropagation();
    setSaving(true);
    try {
      if (isTauriDesktop()) {
        const [{ save }, { writeFile }] = await Promise.all([
          import('@tauri-apps/plugin-dialog'),
          import('@tauri-apps/plugin-fs'),
        ]);
        const filePath = await save({
          title: t('toolCards.codeReview.export.saveDialogTitle'),
          defaultPath: fileName,
          filters: [{
            name: 'Markdown',
            extensions: ['md'],
          }],
        });
        if (!filePath) {
          return;
        }
        await writeFile(filePath, new TextEncoder().encode(markdown));
      } else {
        downloadMarkdownInBrowser(fileName, markdown);
      }
      notificationService.success(t('toolCards.codeReview.export.saveSuccess'));
    } catch {
      notificationService.error(t('toolCards.codeReview.export.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, [fileName, markdown, t]);

  if (variant === 'footer') {
    return (
      <div
        className="code-review-report-actions code-review-report-actions--footer"
        onClick={(event) => event.stopPropagation()}
      >
        {visibleActions.has('open') && (
          <Button
            type="button"
            variant="secondary"
            size="small"
            className="code-review-report-actions__footer-button"
            onClick={handleOpenInEditor}
          >
            <FilePenLine size={14} />
            {t('toolCards.codeReview.export.openMarkdown')}
          </Button>
        )}
      </div>
    );
  }

  return (
    <div className="code-review-report-actions" onClick={(event) => event.stopPropagation()}>
      {visibleActions.has('copy') && (
        <Tooltip content={t('toolCards.codeReview.export.copyMarkdown')} placement="top">
          <button
            type="button"
            className="code-review-report-actions__button"
            onClick={handleCopy}
            aria-label={t('toolCards.codeReview.export.copyMarkdown')}
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>
        </Tooltip>
      )}
      {visibleActions.has('open') && (
        <Tooltip content={t('toolCards.codeReview.export.openMarkdown')} placement="top">
          <button
            type="button"
            className="code-review-report-actions__button"
            onClick={handleOpenInEditor}
            aria-label={t('toolCards.codeReview.export.openMarkdown')}
          >
            <FilePenLine size={14} />
          </button>
        </Tooltip>
      )}
      {visibleActions.has('save') && (
        <Tooltip content={t('toolCards.codeReview.export.saveMarkdown')} placement="top">
          <button
            type="button"
            className="code-review-report-actions__button"
            onClick={handleSave}
            disabled={saving}
            aria-label={t('toolCards.codeReview.export.saveMarkdown')}
          >
            {saving ? <Loader2 className="animate-spin" size={14} /> : <Download size={14} />}
          </button>
        </Tooltip>
      )}
    </div>
  );
};

CodeReviewReportExportActions.displayName = 'CodeReviewReportExportActions';
