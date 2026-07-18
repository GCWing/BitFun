
import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SessionMetadata, DialogTurnData } from '@/shared/types/session-history';

export type UiSessionMetadataField =
  | 'sessionName'
  | 'tags'
  | 'todos'
  | 'reviewActionState'
  | 'unreadCompletion'
  | 'needsUserAttention'
  | 'titleMetadata';

export interface SessionMetadataPageRequest {
  workspacePath: string;
  limit: number;
  cursor?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface SessionMetadataPage {
  sessions: SessionMetadata[];
  totalTopLevelCount: number;
  loadedTopLevelCount: number;
  nextCursor?: string;
  hasMore: boolean;
}

export interface SessionUsageReportRequest {
  sessionId: string;
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  includeHiddenSubagents?: boolean;
}

export type UsageModelIdentitySource = 'recorded' | 'inferred_session_model' | 'legacy_missing';

export interface SessionUsageReport {
  schemaVersion: number;
  reportId: string;
  sessionId: string;
  generatedAt: number;
  generatedFromAppVersion?: string;
  workspace: {
    kind: 'local' | 'remote_ssh' | 'unknown';
    pathLabel?: string;
    workspaceId?: string;
    remoteConnectionId?: string;
    remoteSshHost?: string;
  };
  scope: {
    kind: 'entire_session' | 'turn_range';
    turnCount: number;
    fromTurnId?: string;
    toTurnId?: string;
    includesSubagents: boolean;
  };
  coverage: {
    level: 'complete' | 'partial' | 'minimal';
    available: string[];
    missing: string[];
    notes: string[];
  };
  time: {
    accounting: 'approximate' | 'exact' | 'unavailable';
    denominator: 'session_wall_time' | 'active_turn_time' | 'unavailable';
    wallTimeMs?: number;
    activeTurnMs?: number;
    modelMs?: number;
    toolMs?: number;
    idleGapMs?: number;
  };
  tokens: {
    source: 'token_usage_records' | 'unavailable';
    inputTokens?: number;
    outputTokens?: number;
    totalTokens?: number;
    cachedTokens?: number;
    cacheCoverage: 'available' | 'partial' | 'unavailable';
    /** `cached / input` over records that explicitly report cached tokens. Range 0–1. */
    cacheHitRate?: number;
  };
  models: Array<{
    modelId: string;
    modelIdSource?: UsageModelIdentitySource;
    callCount: number;
    inputTokens?: number;
    outputTokens?: number;
    totalTokens?: number;
    cachedTokens?: number;
    /** Per-model hit rate. Same semantic as `tokens.cacheHitRate`. */
    cacheHitRate?: number;
    durationMs?: number;
    sampleTurnId?: string;
    sampleTurnIndex?: number;
  }>;
  tools: Array<{
    toolName: string;
    category?: 'git' | 'shell' | 'file' | 'other';
    callCount: number;
    successCount: number;
    errorCount: number;
    durationMs?: number;
    p95DurationMs?: number;
    queueWaitMs?: number;
    preflightMs?: number;
    confirmationWaitMs?: number;
    executionMs?: number;
    sampleTurnId?: string;
    sampleTurnIndex?: number;
    sampleItemId?: string;
    redacted: boolean;
  }>;
  files: {
    scope: 'snapshot_summary' | 'tool_inputs_only' | 'unavailable';
    changedFiles?: number;
    addedLines?: number;
    deletedLines?: number;
    files: Array<{
      pathLabel: string;
      operationCount: number;
      addedLines?: number;
      deletedLines?: number;
      sessionId?: string;
      turnIndexes?: number[];
      operationIds?: string[];
      redacted: boolean;
    }>;
  };
  compression: {
    compactionCount: number;
    manualCompactionCount: number;
    automaticCompactionCount: number;
    savedTokens?: number;
  };
  errors: {
    totalErrors: number;
    toolErrors: number;
    modelErrors: number;
    examples: Array<{
      label: string;
      count: number;
      sampleTurnId?: string;
      sampleTurnIndex?: number;
      sampleItemId?: string;
      redacted: boolean;
    }>;
  };
  slowest: Array<{
    label: string;
    kind: 'model' | 'tool' | 'turn';
    durationMs: number;
    redacted: boolean;
    turnId?: string;
    turnIndex?: number;
    itemId?: string;
    inputSummary?: string;
    status?: string;
    timeoutSeconds?: number;
    exitCode?: number;
    timedOut?: boolean;
    errorSummary?: string;
    queueWaitMs?: number;
    preflightMs?: number;
    confirmationWaitMs?: number;
    executionMs?: number;
    modelIdSource?: UsageModelIdentitySource;
  }>;
  privacy: {
    promptContentIncluded: boolean;
    toolInputsIncluded: boolean;
    commandOutputsIncluded: boolean;
    fileContentsIncluded: boolean;
    redactedFields: string[];
  };
}

function remoteSessionFields(
  remoteConnectionId?: string,
  remoteSshHost?: string
): Record<string, string> {
  const o: Record<string, string> = {};
  if (remoteConnectionId) {
    o.remote_connection_id = remoteConnectionId;
  }
  if (remoteSshHost) {
    o.remote_ssh_host = remoteSshHost;
  }
  return o;
}

export class SessionAPI {
  async forkSession(
    sourceSessionId: string,
    sourceTurnId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<{ sessionId: string; sessionName: string; agentType: string }> {
    try {
      return await api.invoke('fork_session', {
        request: {
          source_session_id: sourceSessionId,
          source_turn_id: sourceTurnId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('fork_session', error, {
        sourceSessionId,
        sourceTurnId,
        workspacePath,
      });
    }
  }

  async listSessions(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('list_persisted_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('list_persisted_sessions', error, { workspacePath });
    }
  }

  async listSessionsPage(
    request: SessionMetadataPageRequest
  ): Promise<SessionMetadataPage> {
    try {
      return await api.invoke('list_persisted_sessions_page', {
        request: {
          workspace_path: request.workspacePath,
          limit: request.limit,
          ...(request.cursor ? { cursor: request.cursor } : {}),
          ...remoteSessionFields(request.remoteConnectionId, request.remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('list_persisted_sessions_page', error, {
        workspacePath: request.workspacePath,
        limit: request.limit,
        cursor: request.cursor,
      });
    }
  }

  async loadSessionTurns(
    sessionId: string,
    workspacePath: string,
    limit?: number,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<DialogTurnData[]> {
    try {
      const request: Record<string, unknown> = {
        session_id: sessionId,
        workspace_path: workspacePath,
        ...remoteSessionFields(remoteConnectionId, remoteSshHost),
      };

      if (limit !== undefined) {
        request.limit = limit;
      }

      return await api.invoke('load_session_turns', {
        request
      });
    } catch (error) {
      throw createTauriCommandError('load_session_turns', error, { sessionId, workspacePath, limit });
    }
  }

  async saveSessionTurn(
    turnData: DialogTurnData,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('save_session_turn', {
        request: {
          turn_data: turnData,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_turn', error, { turnData, workspacePath });
    }
  }

  async saveSessionMetadata(
    metadata: SessionMetadata,
    workspacePath: string,
    fields: UiSessionMetadataField[],
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('save_session_metadata', {
        request: {
          metadata,
          fields,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_metadata', error, { metadata, workspacePath });
    }
  }

  async deleteSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('delete_persisted_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('delete_persisted_session', error, { sessionId, workspacePath });
    }
  }

  async touchSessionActivity(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('touch_session_activity', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('touch_session_activity', error, { sessionId, workspacePath });
    }
  }

  async loadSessionMetadata(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<SessionMetadata | null> {
    try {
      return await api.invoke('load_persisted_session_metadata', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('load_persisted_session_metadata', error, { sessionId, workspacePath });
    }
  }

  async getSessionUsageReport(
    request: SessionUsageReportRequest
  ): Promise<SessionUsageReport> {
    try {
      return await api.invoke('get_session_usage_report', {
        request: {
          session_id: request.sessionId,
          workspace_path: request.workspacePath,
          include_hidden_subagents: request.includeHiddenSubagents ?? true,
          ...remoteSessionFields(request.remoteConnectionId, request.remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('get_session_usage_report', error, {
        sessionId: request.sessionId,
        workspacePath: request.workspacePath,
      });
    }
  }

  async archiveSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('archive_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('archive_session', error, { sessionId, workspacePath });
    }
  }

  async unarchiveSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke('unarchive_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('unarchive_session', error, { sessionId, workspacePath });
    }
  }

  async archiveAllSessions(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<number> {
    try {
      return await api.invoke('archive_all_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('archive_all_sessions', error, { workspacePath });
    }
  }

  async listArchivedSessions(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('list_archived_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('list_archived_sessions', error, { workspacePath });
    }
  }

  async deleteAllArchivedSessions(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<number> {
    try {
      return await api.invoke('delete_all_archived_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost),
        }
      });
    } catch (error) {
      throw createTauriCommandError('delete_all_archived_sessions', error, { workspacePath });
    }
  }
}

export const sessionAPI = new SessionAPI();
