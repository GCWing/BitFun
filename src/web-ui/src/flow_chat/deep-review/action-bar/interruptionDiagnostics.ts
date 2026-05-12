import type {
  AiErrorDetail,
  AiErrorPresentation,
} from '@/shared/ai-errors/aiErrorPresenter';

type Translate = (key: string, options?: Record<string, unknown> & { defaultValue?: string }) => string;

function truncateDiagnosticValue(value: string): string {
  return value.length > 500 ? `${value.slice(0, 500)}... [truncated]` : value;
}

export function buildInterruptionDiagnostics(
  detail: AiErrorDetail,
  presentation: AiErrorPresentation,
  t: Translate,
): string {
  if (presentation.diagnostics && !presentation.diagnostics.trim().startsWith('category=')) {
    return presentation.diagnostics;
  }

  const lines: string[] = [];
  lines.push(t('deepReviewActionBar.diagnosticsTitle', { defaultValue: '=== Deep Review Interruption Diagnostics ===' }));
  lines.push('');

  const categoryLabel = t(presentation.titleKey, { defaultValue: presentation.category });
  const categoryMessage = t(presentation.messageKey, { defaultValue: '' });
  lines.push(`${t('deepReviewActionBar.diagnosticsErrorType', { defaultValue: 'Error type' })}: ${categoryLabel} (${presentation.category})`);
  if (categoryMessage) {
    lines.push(`${t('deepReviewActionBar.diagnosticsDescription', { defaultValue: 'Description' })}: ${categoryMessage}`);
  }
  lines.push('');

  if (presentation.actions.length > 0) {
    const actionLabels = presentation.actions.map((action) => {
      return t(action.labelKey, { defaultValue: action.code });
    });
    lines.push(`${t('deepReviewActionBar.diagnosticsSuggestedActions', { defaultValue: 'Suggested actions' })}: ${actionLabels.join(', ')}`);
    lines.push('');
  }

  lines.push(`${t('deepReviewActionBar.diagnosticsTechnicalDetails', { defaultValue: 'Technical details' })}:`);
  lines.push(`  - category: ${detail.category ?? 'unknown'}`);
  if (detail.provider) lines.push(`  - provider: ${detail.provider}`);
  if (detail.providerCode) lines.push(`  - provider code: ${detail.providerCode}`);
  if (detail.providerMessage) {
    lines.push(`  - provider message: ${truncateDiagnosticValue(detail.providerMessage)}`);
  }
  if (detail.httpStatus) lines.push(`  - HTTP status: ${detail.httpStatus}`);
  if (detail.requestId) lines.push(`  - request ID: ${detail.requestId}`);
  if (detail.rawMessage) {
    lines.push(`  - raw message: ${truncateDiagnosticValue(detail.rawMessage)}`);
  }

  return lines.join('\n');
}
