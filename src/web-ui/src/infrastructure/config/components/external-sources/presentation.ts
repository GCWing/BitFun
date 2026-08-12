import type { TFunction } from 'i18next';
import type {
  ExternalSourceCatalogSnapshot,
  ExternalToolCatalogEntry,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';

/** Trims deep paths to the trailing segments that identify the source. */
export function abbreviatedLocation(location: string): string {
  const normalized = location.replace(/\\/g, '/');
  const segments = normalized.split('/').filter(Boolean);
  return segments.length <= 3 ? normalized : `…/${segments.slice(-3).join('/')}`;
}

export function matchesToolSource(
  source: ExternalSourceCatalogSnapshot['sources'][number],
  tool: ExternalToolCatalogEntry,
): boolean {
  return source.record.key.providerId === tool.definition.id.target.source.providerId
    && source.record.key.sourceId === tool.definition.id.target.source.sourceId;
}

export function executionLocationLabel(t: TFunction, executionDomainId?: string): string {
  if (executionDomainId?.startsWith('local')) return t('executionLocation.local');
  if (executionDomainId?.startsWith('remote')) return t('executionLocation.remote');
  return t('executionLocation.unknown');
}

export function sourceScopeLabel(scope: string, t: TFunction): string {
  return scope === 'workspace_local'
    ? t('shared:features.workspace')
    : t(`scope.${scope}`);
}

/** Capability counters shown as badges on a source group. */
export const SOURCE_COUNT_LABELS = [
  ['commands', 'sources.commandCount'],
  ['tools', 'sources.toolCount'],
  ['agents', 'sources.agentCount'],
  ['mcps', 'sources.mcpCount'],
] as const;

type SourceDiagnostic = NonNullable<
  ExternalSourceCatalogSnapshot['diagnostics']
>[number];

/**
 * Maps a raw diagnostic code to a user-facing category. Codes stay in the
 * collapsed technical details; the category is what the user reads first.
 */
export function sourceDiagnosticCategory(code: string): string {
  if (code.includes('external_mcp.runtime_unavailable')) return 'runtimeUnavailable';
  if (code.includes('preference_read_failed')) return 'confirmationStateUnavailable';
  if (code.includes('conflict_history_write_failed')) return 'conflictHistoryUnavailable';
  if (code.includes('discovery_in_progress')) return 'checkInProgress';
  if (code.includes('timeout')) return 'checkTimedOut';
  if (code.includes('trust_required')) return 'confirmationRequired';
  if (code.includes('too_large') || code.includes('file_limit') || code.includes('bytes_limit')) {
    return 'sourceTooLarge';
  }
  if (code.includes('invalid') || code.includes('parse') || code.includes('definition')
    || code.includes('export_missing') || code.includes('name_unsupported')) {
    return 'invalidSettings';
  }
  if (code.includes('unreadable') || code.includes('read_failed')
    || code.includes('metadata_failed') || code.includes('directory_')) {
    return 'unreadableSource';
  }
  if (code.includes('projection_only') || code.includes('unsupported')
    || code.includes('restricted')) {
    return 'notSupported';
  }
  if (code.includes('failed')) return 'checkFailed';
  return 'sourceIssue';
}

export function isActionableSourceDiagnostic(diagnostic: SourceDiagnostic): boolean {
  return diagnostic.severity.toLowerCase() !== 'info';
}
