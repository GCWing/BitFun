import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';

type Translator = (key: string, options?: Record<string, unknown>) => string;
type ModelIdentitySource = SessionUsageReport['models'][number]['modelIdSource'];

const UNKNOWN_MODEL_ID = 'unknown_model';
const LEGACY_MODEL_ROUND_LABEL_PATTERN = /^model\s+round\s+\d+$/i;
const FILE_PATH_MIDDLE_ELLIPSIS_THRESHOLD = 48;
export const USAGE_EXPORT_REDACT_PATHS_STORAGE_KEY = 'bitfun.sessionUsage.export.redactPaths';
type UsageRedactPathsPreferenceListener = (redactPaths: boolean) => void;
const usageRedactPathsPreferenceListeners = new Set<UsageRedactPathsPreferenceListener>();

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

export function getModelLabel(
  modelId: string | undefined,
  t: Translator,
  source?: ModelIdentitySource
): string {
  if (
    source === 'inferred_session_model' &&
    modelId &&
    modelId !== UNKNOWN_MODEL_ID &&
    !isLegacyModelRoundLabel(modelId) &&
    !isOpaqueModelIdentifier(modelId)
  ) {
    return t('usage.status.inferredModel', { model: modelId });
  }
  if (
    !modelId ||
    modelId === UNKNOWN_MODEL_ID ||
    isLegacyModelRoundLabel(modelId) ||
    source === 'legacy_missing' ||
    source === 'inferred_session_model'
  ) {
    return t('usage.status.legacyModel');
  }
  return modelId;
}

export function getModelHelp(
  source: ModelIdentitySource | undefined,
  t: Translator,
  modelId?: string
): string | undefined {
  if (source === 'inferred_session_model') {
    if (modelId && (isOpaqueModelIdentifier(modelId) || isLegacyModelRoundLabel(modelId))) {
      return t('usage.help.legacyModel');
    }
    return t('usage.help.inferredModel');
  }
  if (source === 'legacy_missing') {
    return t('usage.help.legacyModel');
  }
  if (isLegacyModelRoundLabel(modelId)) {
    return t('usage.help.legacyModel');
  }
  return undefined;
}

export function getSlowSpanLabel(
  span: SessionUsageReport['slowest'][number],
  t: Translator
): string {
  if (span.redacted) {
    return getRedactedLabel(t);
  }
  if (span.kind === 'model') {
    return typeof span.turnIndex === 'number' && Number.isFinite(span.turnIndex)
      ? t('usage.slowestLabels.modelCall', { turn: span.turnIndex })
      : t('usage.slowestLabels.modelCallUnknown');
  }
  return span.label;
}

export function getSlowSpanHelp(
  span: SessionUsageReport['slowest'][number],
  t: Translator
): string | undefined {
  if (span.redacted) {
    return undefined;
  }
  if (span.kind === 'model') {
    const modelLabel = getModelLabel(span.label, t, span.modelIdSource);
    const modelHelp = getModelHelp(span.modelIdSource, t, span.label);
    const callHelp = t('usage.help.slowestModelCall', { model: modelLabel });
    return modelHelp ? `${callHelp} ${modelHelp}` : callHelp;
  }
  if (span.modelIdSource) {
    return getModelHelp(span.modelIdSource, t, span.label);
  }
  return span.label === UNKNOWN_MODEL_ID || isLegacyModelRoundLabel(span.label)
    ? t('usage.help.legacyModel')
    : undefined;
}

function isOpaqueModelIdentifier(modelId: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(modelId) ||
    /^[0-9a-f]{32}$/i.test(modelId);
}

function isLegacyModelRoundLabel(modelId: string | undefined): boolean {
  return Boolean(modelId && LEGACY_MODEL_ROUND_LABEL_PATTERN.test(modelId.trim()));
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

export function getUsageFilePathDisplayParts(pathLabel: string): { prefix: string; fileName: string } | null {
  if (pathLabel.length <= FILE_PATH_MIDDLE_ELLIPSIS_THRESHOLD) {
    return null;
  }

  const segments = pathLabel.split(/[\\/]+/).filter(Boolean);
  if (segments.length <= 1) {
    return null;
  }

  const fileName = segments.at(-1) ?? pathLabel;
  const prefix = segments.slice(0, -1).join('/');
  return prefix ? { prefix, fileName } : null;
}

export function getUsageFileNameFromPath(pathLabel: string): string {
  const segments = pathLabel.split(/[\\/]+/).filter(Boolean);
  return segments.at(-1) ?? pathLabel;
}

export function getUsageDisplayPathLabel(
  pathLabel: string | undefined,
  t: Translator,
  options: {
    redactPaths: boolean;
    keepFileName?: boolean;
  }
): string {
  const normalizedPath = pathLabel?.trim();
  if (!normalizedPath) {
    return t('usage.unavailable');
  }
  if (!options.redactPaths) {
    return normalizedPath;
  }

  const redactedPath = t('usage.export.redactedPath');
  if (!options.keepFileName) {
    return redactedPath;
  }

  const fileName = getUsageFileNameFromPath(normalizedPath);
  return fileName && fileName !== normalizedPath ? `${redactedPath}/${fileName}` : redactedPath;
}

export function getRedactedLabel(t: Translator): string {
  return t('usage.redacted');
}

export function getUsageExportRedactPathsPreference(): boolean {
  try {
    const stored = globalThis.window?.localStorage?.getItem(USAGE_EXPORT_REDACT_PATHS_STORAGE_KEY);
    return stored === null ? true : stored !== 'false';
  } catch {
    return true;
  }
}

export function setUsageExportRedactPathsPreference(redactPaths: boolean): void {
  try {
    globalThis.window?.localStorage?.setItem(
      USAGE_EXPORT_REDACT_PATHS_STORAGE_KEY,
      redactPaths ? 'true' : 'false',
    );
  } catch {
    // Ignore storage failures; the export action should still work.
  }
  usageRedactPathsPreferenceListeners.forEach(listener => listener(redactPaths));
}

export function subscribeUsageExportRedactPathsPreference(
  listener: UsageRedactPathsPreferenceListener
): () => void {
  usageRedactPathsPreferenceListeners.add(listener);
  return () => {
    usageRedactPathsPreferenceListeners.delete(listener);
  };
}

export function buildSessionUsageExportMarkdown(
  markdown: string,
  report: SessionUsageReport | undefined,
  options: {
    redactPaths: boolean;
    t: Translator;
  }
): string {
  if (!options.redactPaths || !report) {
    return markdown;
  }

  const redactedPath = options.t('usage.export.redactedPath');
  const replacements = new Map<string, string>();

  addPathReplacement(replacements, report.workspace.pathLabel, redactedPath);
  for (const file of report.files.files) {
    if (!file.redacted) {
      const fileName = getUsageFileNameFromPath(file.pathLabel);
      addPathReplacement(replacements, file.pathLabel, `${redactedPath}/${fileName}`);
    }
  }

  return [...replacements.entries()]
    .sort((a, b) => b[0].length - a[0].length)
    .reduce((value, [pathLabel, replacement]) => (
      value.replace(new RegExp(escapeRegExp(pathLabel), 'g'), replacement)
    ), markdown);
}

function addPathReplacement(
  replacements: Map<string, string>,
  value: string | undefined,
  replacement: string
): void {
  const normalizedValue = value?.trim();
  if (!normalizedValue) {
    return;
  }
  replacements.set(normalizedValue, replacement);
  const alternateSeparatorValue = normalizedValue.includes('\\')
    ? normalizedValue.replace(/\\/g, '/')
    : normalizedValue.replace(/\//g, '\\');
  if (alternateSeparatorValue !== normalizedValue) {
    replacements.set(alternateSeparatorValue, replacement);
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
