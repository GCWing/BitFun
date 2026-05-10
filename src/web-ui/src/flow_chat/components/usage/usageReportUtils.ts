import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';

type Translator = (key: string, options?: Record<string, unknown>) => string;

export function hasNoRecordedFileChanges(report: SessionUsageReport): boolean {
  return report.files.files.length === 0 &&
    (report.files.changedFiles === undefined || report.files.changedFiles === 0);
}

export function isSessionUsageReport(value: unknown): value is SessionUsageReport {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<SessionUsageReport>;
  // Keep this structural guard strict enough that legacy Markdown-only local
  // reports stay on the safe fallback renderer instead of being treated as DTOs.
  return (
    typeof candidate.reportId === 'string' &&
    typeof candidate.sessionId === 'string' &&
    typeof candidate.generatedAt === 'number' &&
    !!candidate.scope &&
    !!candidate.coverage &&
    !!candidate.time &&
    !!candidate.tokens &&
    Array.isArray(candidate.tools)
  );
}

export function coerceSessionUsageReport(value: unknown): SessionUsageReport | undefined {
  return isSessionUsageReport(value) ? value : undefined;
}

export function formatUsageNumber(value: number | undefined, t: Translator): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return t('usage.unavailable');
  }
  return new Intl.NumberFormat().format(value);
}

export function formatUsageDuration(value: number | undefined, t: Translator): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return t('usage.unavailable');
  }
  if (value < 1000) {
    return t('usage.duration.ms', { value: Math.max(0, Math.round(value)) });
  }

  const seconds = Math.round(value / 1000);
  if (seconds < 60) {
    return t('usage.duration.seconds', { value: seconds });
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) {
    return remainingSeconds === 0
      ? t('usage.duration.minutes', { value: minutes })
      : t('usage.duration.minutesSeconds', { minutes, seconds: remainingSeconds });
  }

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes === 0
    ? t('usage.duration.hours', { value: hours })
    : t('usage.duration.hoursMinutes', { hours, minutes: remainingMinutes });
}

export function formatUsageTimestamp(value: number | undefined, t: Translator): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return t('usage.unavailable');
  }
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

export function formatUsagePercent(value: number | undefined, t: Translator): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return t('usage.unavailable');
  }
  return t('usage.percent', { value: Math.round(value) });
}

export function calculateShare(part: number | undefined, denominator: number | undefined): number | undefined {
  if (
    typeof part !== 'number' ||
    typeof denominator !== 'number' ||
    !Number.isFinite(part) ||
    !Number.isFinite(denominator) ||
    denominator <= 0
  ) {
    return undefined;
  }
  return Math.min(100, Math.max(0, (part / denominator) * 100));
}

export function getCoverageLabel(level: SessionUsageReport['coverage']['level'], t: Translator): string {
  return t(`usage.coverage.${level}`);
}

export function getCoverageTone(level: SessionUsageReport['coverage']['level']): 'complete' | 'partial' | 'minimal' {
  return level;
}

export function getToolCategoryLabel(
  category: SessionUsageReport['tools'][number]['category'] | undefined,
  t: Translator
): string {
  return t(`usage.toolCategories.${category ?? 'other'}`);
}

export function getFileScopeLabel(scope: SessionUsageReport['files']['scope'], t: Translator): string {
  return t(`usage.fileScopes.${scope}`);
}

export function getFileSummaryLabel(report: SessionUsageReport, t: Translator): string {
  if (hasNoRecordedFileChanges(report)) {
    return t('usage.status.noFileChanges');
  }
  return formatUsageNumber(report.files.changedFiles, t);
}

export function getFileScopeHelp(report: SessionUsageReport, t: Translator): string | undefined {
  if (report.files.scope !== 'unavailable') {
    return undefined;
  }
  if (report.workspace.kind === 'remote_ssh') {
    return t('usage.help.filesRemoteUnavailable');
  }
  if (hasNoRecordedFileChanges(report)) {
    return t('usage.help.filesNoRecordedChanges');
  }
  return t('usage.help.filesNotTracked');
}

export function getAccountingLabel(accounting: SessionUsageReport['time']['accounting'], t: Translator): string {
  return t(`usage.accounting.${accounting}`);
}

export function getTopModels(report: SessionUsageReport, limit: number): SessionUsageReport['models'] {
  return [...report.models]
    .sort((a, b) => (b.totalTokens ?? 0) - (a.totalTokens ?? 0) || (b.durationMs ?? 0) - (a.durationMs ?? 0))
    .slice(0, limit);
}

export function getTopTools(report: SessionUsageReport, limit: number): SessionUsageReport['tools'] {
  return [...report.tools]
    .sort((a, b) => (b.durationMs ?? 0) - (a.durationMs ?? 0) || b.callCount - a.callCount)
    .slice(0, limit);
}

export function getTopFiles(report: SessionUsageReport, limit: number): SessionUsageReport['files']['files'] {
  return [...report.files.files]
    .sort((a, b) =>
      (b.addedLines ?? 0) + (b.deletedLines ?? 0) - ((a.addedLines ?? 0) + (a.deletedLines ?? 0)) ||
      b.operationCount - a.operationCount
    )
    .slice(0, limit);
}

export function getRedactedLabel(t: Translator): string {
  return t('usage.redacted');
}
