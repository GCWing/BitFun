import React, { useCallback, useMemo, useState } from 'react';
import { Check, ClipboardCopy, FileDown, FilePenLine, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
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
}

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
}) => {
  const { t } = useTranslation('flow-chat');
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);

  const markdownLabels = useMemo<Partial<CodeReviewReportMarkdownLabels>>(() => ({
    titleStandard: t('toolCards.codeReview.report.titleStandard', { defaultValue: 'Code Review Report' }),
    titleDeep: t('toolCards.codeReview.report.titleDeep', { defaultValue: 'Deep Review Report' }),
    executiveSummary: t('toolCards.codeReview.sections.summary', { defaultValue: 'Executive Summary' }),
    reviewDecision: t('toolCards.codeReview.report.reviewDecision', { defaultValue: 'Review Decision' }),
    runManifest: t('toolCards.codeReview.sections.runManifest', { defaultValue: 'Run manifest' }),
    riskLevel: t('toolCards.codeReview.riskLevel', { defaultValue: 'Risk Level' }),
    recommendedAction: t('toolCards.codeReview.recommendedAction', { defaultValue: 'Recommended Action' }),
    scope: t('toolCards.codeReview.reviewScope', { defaultValue: 'Scope' }),
    target: t('toolCards.codeReview.runManifest.target', { defaultValue: 'Target' }),
    budget: t('toolCards.codeReview.runManifest.budget', { defaultValue: 'Budget' }),
    estimatedCalls: t('toolCards.codeReview.runManifest.estimatedCalls', { defaultValue: 'Estimated calls' }),
    activeReviewers: t('toolCards.codeReview.runManifest.activeGroupTitle', { defaultValue: 'Will run' }),
    skippedReviewers: t('toolCards.codeReview.runManifest.skippedGroupTitle', { defaultValue: 'Skipped reviewers' }),
    issues: t('toolCards.codeReview.sections.issues', { defaultValue: 'Issues' }),
    noIssues: t('toolCards.codeReview.report.noIssues', { defaultValue: 'No validated issues.' }),
    remediationPlan: t('toolCards.codeReview.sections.remediation', { defaultValue: 'Remediation Plan' }),
    strengths: t('toolCards.codeReview.sections.strengths', { defaultValue: 'Strengths' }),
    reviewTeam: t('toolCards.codeReview.sections.team', { defaultValue: 'Code Review Team' }),
    reliabilitySignals: t('toolCards.codeReview.report.reliabilitySignals', { defaultValue: 'Review Reliability' }),
    coverageNotes: t('toolCards.codeReview.sections.coverage', { defaultValue: 'Coverage Notes' }),
    status: t('toolCards.codeReview.report.status', { defaultValue: 'Status' }),
    packet: t('toolCards.codeReview.report.packet', { defaultValue: 'Packet' }),
    findings: t('toolCards.codeReview.report.findings', { defaultValue: 'Findings' }),
    validation: t('toolCards.codeReview.report.validation', { defaultValue: 'Validation' }),
    suggestion: t('toolCards.codeReview.suggestion', { defaultValue: 'Suggestion' }),
    source: t('toolCards.codeReview.report.source', { defaultValue: 'Source' }),
    noItems: t('toolCards.codeReview.report.noItems', { defaultValue: 'None.' }),
    groupTitles: {
      must_fix: t('toolCards.codeReview.groups.must_fix', { defaultValue: 'Must fix' }),
      should_improve: t('toolCards.codeReview.groups.should_improve', { defaultValue: 'Should improve' }),
      needs_decision: t('toolCards.codeReview.groups.needs_decision', { defaultValue: 'Needs decision' }),
      verification: t('toolCards.codeReview.groups.verification', { defaultValue: 'Verification' }),
      architecture: t('toolCards.codeReview.groups.architecture', { defaultValue: 'Architecture' }),
      maintainability: t('toolCards.codeReview.groups.maintainability', { defaultValue: 'Maintainability' }),
      tests: t('toolCards.codeReview.groups.tests', { defaultValue: 'Tests' }),
      security: t('toolCards.codeReview.groups.security', { defaultValue: 'Security' }),
      performance: t('toolCards.codeReview.groups.performance', { defaultValue: 'Performance' }),
      user_experience: t('toolCards.codeReview.groups.user_experience', { defaultValue: 'User experience' }),
      other: t('toolCards.codeReview.groups.other', { defaultValue: 'Other' }),
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
    const prefix = t('toolCards.codeReview.export.fileNamePrefix', {
      defaultValue: 'BitFun-Code-Review',
    });
    return `${prefix}_${timestampForFileName()}.md`;
  }, [t]);

  const handleCopy = useCallback(async (event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
      notificationService.success(t('toolCards.codeReview.export.copySuccess', {
        defaultValue: 'Review report copied as Markdown.',
      }));
    } catch {
      notificationService.error(t('toolCards.codeReview.export.copyFailed', {
        defaultValue: 'Failed to copy review report.',
      }));
    }
  }, [markdown, t]);

  const handleOpenInEditor = useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      createMarkdownEditorTab(
        t('toolCards.codeReview.export.editorTitle', { defaultValue: 'Review Report' }),
        markdown,
        undefined,
        undefined,
        'agent',
      );
    } catch {
      notificationService.error(t('toolCards.codeReview.export.openFailed', {
        defaultValue: 'Failed to open review report in editor.',
      }));
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
          title: t('toolCards.codeReview.export.saveDialogTitle', {
            defaultValue: 'Export review report',
          }),
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
      notificationService.success(t('toolCards.codeReview.export.saveSuccess', {
        defaultValue: 'Review report exported.',
      }));
    } catch {
      notificationService.error(t('toolCards.codeReview.export.saveFailed', {
        defaultValue: 'Failed to export review report.',
      }));
    } finally {
      setSaving(false);
    }
  }, [fileName, markdown, t]);

  return (
    <div className="code-review-report-actions" onClick={(event) => event.stopPropagation()}>
      <Tooltip content={t('toolCards.codeReview.export.copyMarkdown', { defaultValue: 'Copy Markdown' })} placement="top">
        <button
          type="button"
          className="code-review-report-actions__button"
          onClick={handleCopy}
          aria-label={t('toolCards.codeReview.export.copyMarkdown', { defaultValue: 'Copy Markdown' })}
        >
          {copied ? <Check size={13} /> : <ClipboardCopy size={13} />}
        </button>
      </Tooltip>
      <Tooltip content={t('toolCards.codeReview.export.openMarkdown', { defaultValue: 'Open as Markdown' })} placement="top">
        <button
          type="button"
          className="code-review-report-actions__button"
          onClick={handleOpenInEditor}
          aria-label={t('toolCards.codeReview.export.openMarkdown', { defaultValue: 'Open as Markdown' })}
        >
          <FilePenLine size={13} />
        </button>
      </Tooltip>
      <Tooltip content={t('toolCards.codeReview.export.saveMarkdown', { defaultValue: 'Save Markdown' })} placement="top">
        <button
          type="button"
          className="code-review-report-actions__button"
          onClick={handleSave}
          disabled={saving}
          aria-label={t('toolCards.codeReview.export.saveMarkdown', { defaultValue: 'Save Markdown' })}
        >
          {saving ? <Loader2 className="animate-spin" size={13} /> : <FileDown size={13} />}
        </button>
      </Tooltip>
    </div>
  );
};

CodeReviewReportExportActions.displayName = 'CodeReviewReportExportActions';
