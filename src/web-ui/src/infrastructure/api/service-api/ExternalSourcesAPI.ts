import { api } from './ApiClient';

export type ExternalSourceScope =
  | 'user_global'
  | 'project'
  | 'workspace_local'
  | 'remote_user'
  | 'remote_project';

export type ExternalSourceLifecycle =
  | 'available'
  | 'restricted'
  | 'degraded'
  | 'unavailable'
  | 'removed'
  | 'suppressed'
  | 'using_last_valid_version';

export type PromptCommandAvailability =
  | { state: 'available' }
  | { state: 'restricted'; reason: string; required_capabilities: string[] }
  | { state: 'invalid'; reason: string };

export interface ExternalSourceRecord {
  key: { providerId: string; sourceId: string };
  ecosystemId: string;
  displayName: string;
  sourceKind: string;
  scope: ExternalSourceScope;
  location: string;
  executionDomainId: string;
  health: 'available' | 'partial' | 'degraded' | 'unavailable';
  contentVersion: string;
  diagnostics?: Array<{ severity: string; code: string; message: string }>;
}

export interface ExternalSourceCatalogSnapshot {
  generation: number;
  discoveryPending: boolean;
  sources: Array<{
    stableKey: string;
    record: ExternalSourceRecord;
    lifecycle: ExternalSourceLifecycle;
  }>;
  commands: Array<{
    definition: {
      id: {
        source: { providerId: string; sourceId: string };
        localId: string;
      };
      name: string;
      description: string;
      availability: PromptCommandAvailability;
      contentVersion: string;
    };
  }>;
  commandConflicts?: Array<{
    conflictKey: string;
    commandName: string;
    selectedCandidateId?: string;
    candidates: Array<{
      candidateId: string;
      source: { providerId: string; sourceId: string };
      sourceDisplayName: string;
      ecosystemId: string;
      contentVersion: string;
      commandDescription: string;
      sourceScope: ExternalSourceScope;
      sourceLocation: string;
      availability: PromptCommandAvailability;
    }>;
  }>;
  diagnostics?: Array<{ severity: string; code: string; message: string }>;
}

export const externalSourcesAPI = {
  getSnapshot(workspacePath?: string, forceRefresh = false) {
    return api.invoke<ExternalSourceCatalogSnapshot>('get_external_source_snapshot', {
      request: { workspacePath, forceRefresh },
    });
  },

  setSourceEnabled(workspacePath: string | undefined, sourceKey: string, enabled: boolean) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_source_enabled_command', {
      request: { workspacePath, sourceKey, enabled },
    });
  },

  setConflictChoice(workspacePath: string | undefined, conflictKey: string, candidateId: string) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_source_conflict_choice_command', {
      request: { workspacePath, conflictKey, candidateId },
    });
  },
};
