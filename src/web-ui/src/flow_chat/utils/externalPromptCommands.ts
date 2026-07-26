import type {
  ExternalSourceCatalogSnapshot,
  PromptCommandAvailability,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';

export interface ExternalPromptCommandItem {
  id: string;
  command: string;
  label: string;
  candidateId: string;
  contentVersion: string;
  available: boolean;
  unavailableReason?: string;
  conflictKey?: string;
  expectedPreferenceRevision?: number;
}

export type ExternalPromptCommandInvocation =
  | { state: 'none' }
  | { state: 'conflict'; command: string }
  | { state: 'unavailable'; item: ExternalPromptCommandItem }
  | { state: 'resolved'; item: ExternalPromptCommandItem; arguments: string };

export type ExternalPromptCommandCatalogIssue = 'host_unavailable' | 'load_failed';

const HOST_UNAVAILABLE_CODES = new Set([
  'host_unavailable',
  'host_capability_unavailable',
  'incompatible_version',
]);

export function classifyExternalPromptCommandCatalogIssue(
  error: unknown,
): ExternalPromptCommandCatalogIssue {
  const code = typeof error === 'object' && error !== null && 'code' in error
    ? String((error as { code?: unknown }).code ?? '')
    : '';
  return HOST_UNAVAILABLE_CODES.has(code) ? 'host_unavailable' : 'load_failed';
}

export function isExternalPromptSubmissionTargetCurrent(
  capturedSessionId: string | null,
  currentSessionId: string | null,
  capturedWorkspacePath: string,
  currentWorkspacePath: string,
): boolean {
  return capturedSessionId === currentSessionId
    && capturedWorkspacePath === currentWorkspacePath;
}

export function externalPromptComposerIsUnchanged(
  submittedValue: string,
  currentValue: string,
): boolean {
  return submittedValue === currentValue;
}

export function routeUnmatchedExternalPromptCommand(input: {
  hasNativeCommand: boolean;
  catalogLoading: boolean;
  discoveryPending: boolean;
  catalogIssue: ExternalPromptCommandCatalogIssue | undefined;
}): 'native' | 'wait' | 'load_failed' | 'ordinary' {
  if (input.hasNativeCommand) return 'native';
  if (!input.catalogIssue && (input.catalogLoading || input.discoveryPending)) return 'wait';
  if (input.catalogIssue === 'load_failed') return 'load_failed';
  return 'ordinary';
}

function availabilityFacts(availability: PromptCommandAvailability): {
  available: boolean;
  unavailableReason?: string;
} {
  if (availability.state === 'available') {
    return { available: true };
  }
  return {
    available: false,
    unavailableReason: availability.reason,
  };
}

export function buildExternalPromptCommandItems(
  snapshot: ExternalSourceCatalogSnapshot,
): ExternalPromptCommandItem[] {
  const sourceLabels = new Map(
    snapshot.sources.map(source => [
      `${source.record.key.providerId}:${source.record.key.sourceId}`,
      source.record.displayName,
    ]),
  );
  const items = new Map<string, ExternalPromptCommandItem>();

  for (const entry of snapshot.commands) {
    const { definition } = entry;
    const candidateId = entry.candidateId?.trim();
    if (!candidateId) {
      // Legacy hosts cannot provide the guarded identity required by the new
      // expansion endpoint, so do not expose a command that cannot be invoked.
      continue;
    }
    const sourceKey = `${definition.id.source.providerId}:${definition.id.source.sourceId}`;
    const sourceLabel = sourceLabels.get(sourceKey) ?? definition.id.source.providerId;
    items.set(candidateId, {
      id: candidateId,
      command: `/${definition.name}`,
      label: `${definition.description || definition.name} · ${sourceLabel}`,
      candidateId,
      contentVersion: definition.contentVersion,
      ...availabilityFacts(definition.availability),
    });
  }

  for (const conflict of snapshot.commandConflicts ?? []) {
    const candidates = conflict.selectedCandidateId
      ? conflict.candidates.filter(candidate => candidate.candidateId === conflict.selectedCandidateId)
      : conflict.candidates;
    for (const candidate of candidates) {
      if (items.has(candidate.candidateId)) {
        continue;
      }
      items.set(candidate.candidateId, {
        id: candidate.candidateId,
        command: `/${conflict.commandName}`,
        label: `${candidate.commandDescription || conflict.commandName} · ${candidate.sourceDisplayName}`,
        candidateId: candidate.candidateId,
        contentVersion: candidate.contentVersion,
        conflictKey: conflict.conflictKey,
        expectedPreferenceRevision: snapshot.preferenceRevision ?? 0,
        ...availabilityFacts(candidate.availability),
      });
    }
  }

  return Array.from(items.values()).sort((left, right) =>
    left.command.localeCompare(right.command)
      || left.label.localeCompare(right.label)
      || left.candidateId.localeCompare(right.candidateId));
}

export function resolveExternalPromptCommandInvocation(
  input: string,
  items: readonly ExternalPromptCommandItem[],
  reservedCommands: ReadonlySet<string>,
  selectedCandidateId?: string,
): ExternalPromptCommandInvocation {
  const trimmed = input.trim();
  if (!trimmed.startsWith('/')) {
    return { state: 'none' };
  }
  const whitespaceIndex = trimmed.search(/\s/);
  const command = (whitespaceIndex === -1 ? trimmed : trimmed.slice(0, whitespaceIndex))
    .toLowerCase();
  const argumentsText = whitespaceIndex === -1 ? '' : trimmed.slice(whitespaceIndex).trimStart();
  const candidates = items.filter(item => item.command.toLowerCase() === command);
  if (candidates.length === 0) {
    return { state: 'none' };
  }

  const selected = selectedCandidateId
    ? candidates.find(item => item.candidateId === selectedCandidateId)
    : undefined;
  if (!selected && (candidates.length !== 1 || reservedCommands.has(command))) {
    return { state: 'conflict', command };
  }
  const item = selected ?? candidates[0];
  if (!item.available) {
    return { state: 'unavailable', item };
  }
  return { state: 'resolved', item, arguments: argumentsText };
}
