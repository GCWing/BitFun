 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { normalizeRemoteSessionScope } from '@/shared/utils/remoteSessionScope';

const log = createLogger('SnapshotAPI');

const requireWorkspacePath = (workspacePath?: string): string => {
  if (!workspacePath) {
    throw new Error('workspacePath is required for snapshot operations');
  }
  return workspacePath;
};

const requireSessionWorkspacePath = (sessionId: string, workspacePath?: string): string => {
  const resolved =
    workspacePath ||
    flowChatStore.getState().sessions.get(sessionId)?.workspacePath;
  if (!resolved) {
    throw new Error(`workspacePath is required for snapshot session: ${sessionId}`);
  }
  return resolved;
};

interface SnapshotSessionScope {
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

type SnapshotRemoteScope = Omit<SnapshotSessionScope, 'workspacePath'>;

const requireSessionSnapshotScope = (
  sessionId: string,
  workspacePath?: string,
): SnapshotSessionScope => {
  const session = flowChatStore.getState().sessions.get(sessionId);
  return {
    workspacePath: requireSessionWorkspacePath(sessionId, workspacePath),
    ...normalizeRemoteSessionScope(
      session?.remoteConnectionId || session?.config?.remoteConnectionId,
      session?.remoteSshHost || session?.config?.remoteSshHost,
    ),
  };
};

const snapshotScopeKey = (scope: SnapshotSessionScope): string =>
  JSON.stringify([scope.workspacePath, scope.remoteConnectionId || '', scope.remoteSshHost || '']);


export interface SandboxSessionModifications {
  hasModifications: boolean;
  totalFiles: number;
  totalAdditions: number;
  totalDeletions: number;
  modifiedFiles: Array<{
    filePath: string;
    toolName: string;
    operationType: string;
    additions: number;
    deletions: number;
  }>;
}

export interface SandboxOperationDiff {
  filePath: string;
  originalContent: string;
  modifiedContent: string;
  diff?: string;
  operationType?: string;
  toolName?: string;
  anchorLine?: number | null;
}

export interface SessionFileDiffStats {
  filePath: string;
  linesAdded: number;
  linesRemoved: number;
  approximate: boolean;
  changeKind: 'create' | 'modify' | 'delete';
}

export interface GetSessionModificationsRequest {
  sessionId: string;
}

export interface GetOperationDiffRequest {
  sessionId: string;
  filePath: string;
  operationId?: string;
}

export interface GetBaselineSnapshotDiffRequest {
  filePath: string;
}

export interface SandboxOperationSummary {
  operationId: string;
  sessionId: string;
  turnIndex?: number | null;
  seqInTurn?: number | null;
  filePath?: string | null;
  operationType?: string | null;
  toolName?: string | null;
  linesAdded?: number | null;
  linesRemoved?: number | null;
}

export interface GetOperationSummaryRequest {
  sessionId: string;
  operationId: string;
}

export interface AcceptSessionModificationsRequest {
  sessionId: string;
}

export interface RejectSessionModificationsRequest {
  sessionId: string;
}

export interface AcceptFileModificationsRequest {
  sessionId: string;
  filePath: string;
}

export interface RejectFileModificationsRequest {
  sessionId: string;
  filePath: string;
}

export interface AcceptDiffBlockRequest {
  sessionId: string;
  filePath: string;
  blockIndex: number;
}

export interface RejectDiffBlockRequest {
  sessionId: string;
  filePath: string;
  blockIndex: number;
}

export interface AcceptOperationRequest {
  sessionId: string;
  operationId: string;
}

export interface RejectOperationRequest {
  sessionId: string;
  operationId: string;
}

export interface RollbackSessionRequest {
  sessionId: string;
}

export interface CleanupSandboxDataRequest {
  maxAgeDays: number;
}

export class SnapshotAPI {
  private readonly inFlightRequests = new Map<string, Promise<unknown>>();

  private dedupeInFlight<T>(requestKey: string, load: () => Promise<T>): Promise<T> {
    const scope = getActiveSurfaceScope();
    const key = scope.key(scope.epoch, requestKey);
    const existing = this.inFlightRequests.get(key) as Promise<T> | undefined;
    if (existing) {
      return existing;
    }

    const request = load().finally(() => {
      if (this.inFlightRequests.get(key) === request) {
        this.inFlightRequests.delete(key);
      }
    });
    this.inFlightRequests.set(key, request);
    return request;
  }

   
  async getSessionStats(sessionId: string, workspacePath?: string): Promise<{
    session_id: string;
    total_files: number;
    total_turns: number;
    total_changes: number;
  }> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      const key = `get_session_stats:${snapshotScopeKey(scope)}:${sessionId}`;
      return await this.dedupeInFlight(key, () => api.invoke('get_session_stats', {
        request: { session_id: sessionId, ...scope }
      }));
    } catch (error) {
      throw createTauriCommandError('get_session_stats', error, { sessionId, workspacePath });
    }
  }

   
  async getSessionFiles(sessionId: string, workspacePath?: string): Promise<string[]> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      const key = `get_session_files:${snapshotScopeKey(scope)}:${sessionId}`;
      return await this.dedupeInFlight(key, () => api.invoke('get_session_files', {
        request: { session_id: sessionId, ...scope }
      }));
    } catch (error) {
      throw createTauriCommandError('get_session_files', error, { sessionId, workspacePath });
    }
  }

   
  async getOperationDiff(
    sessionId: string,
    filePath: string,
    operationId?: string,
    workspacePath?: string,
  ): Promise<SandboxOperationDiff> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      return await api.invoke('get_operation_diff', { 
        request: { sessionId, filePath, operationId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('get_operation_diff', error, {
        sessionId,
        filePath,
        operationId,
        workspacePath,
      });
    }
  }

  async getSessionFileDiffStats(
    sessionId: string,
    filePath: string,
    workspacePath?: string,
  ): Promise<SessionFileDiffStats> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      const key = `get_session_file_diff_stats:${snapshotScopeKey(scope)}:${sessionId}:${filePath}`;
      return await this.dedupeInFlight(key, () => api.invoke('get_session_file_diff_stats', {
        request: { sessionId, filePath, ...scope },
      }));
    } catch (error) {
      throw createTauriCommandError('get_session_file_diff_stats', error, {
        sessionId,
        filePath,
        workspacePath,
      });
    }
  }

  async getOperationSummary(
    sessionId: string,
    operationId: string,
    workspacePath?: string,
  ): Promise<SandboxOperationSummary> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      const key = `get_operation_summary:${snapshotScopeKey(scope)}:${sessionId}:${operationId}`;
      return await this.dedupeInFlight(key, () => api.invoke('get_operation_summary', {
        request: { sessionId, operationId, ...scope }
      }));
    } catch (error) {
      throw createTauriCommandError('get_operation_summary', error, {
        sessionId,
        operationId,
        workspacePath,
      });
    }
  }

   
  async getBaselineSnapshotDiff(
    filePath: string,
    workspacePath?: string,
    remoteScope: SnapshotRemoteScope = {},
  ): Promise<SandboxOperationDiff> {
    try {
      const resolvedWorkspacePath = requireWorkspacePath(workspacePath);
      return await api.invoke('get_baseline_snapshot_diff', {
        request: { filePath, workspacePath: resolvedWorkspacePath, ...remoteScope }
      });
    } catch (error) {
      throw createTauriCommandError('get_baseline_snapshot_diff', error, { filePath, workspacePath });
    }
  }



   
  async acceptSessionModifications(sessionId: string, workspacePath?: string): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('accept_session', {
        request: { sessionId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('accept_session', error, { sessionId, workspacePath });
    }
  }

   
  async rejectSessionModifications(sessionId: string, workspacePath?: string): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('rollback_session', {
        request: { sessionId, deleteSession: true, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('rollback_session', error, { sessionId, workspacePath });
    }
  }

   
  async acceptFileModifications(
    sessionId: string,
    filePath: string,
    workspacePath?: string,
  ): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('accept_file', {
        request: { sessionId, filePath, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('accept_file', error, { sessionId, filePath, workspacePath });
    }
  }

   
  async rejectFileModifications(
    sessionId: string,
    filePath: string,
    workspacePath?: string,
  ): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('reject_file', {
        request: { sessionId, filePath, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('reject_file', error, { sessionId, filePath, workspacePath });
    }
  }

   
  async acceptDiffBlock(sessionId: string, filePath: string, blockIndex: number): Promise<void> {
    try {
      await api.invoke('accept_diff_block', { 
        request: { sessionId, filePath, blockId: blockIndex.toString() } 
      });
    } catch (error) {
      throw createTauriCommandError('accept_diff_block', error, { sessionId, filePath, blockIndex });
    }
  }

   
  async rejectDiffBlock(sessionId: string, filePath: string, blockIndex: number): Promise<void> {
    try {
      await api.invoke('reject_diff_block', { 
        request: { sessionId, filePath, blockId: blockIndex.toString() } 
      });
    } catch (error) {
      throw createTauriCommandError('reject_diff_block', error, { sessionId, filePath, blockIndex });
    }
  }

   
  async acceptOperation(
    sessionId: string,
    operationId: string,
    workspacePath?: string,
  ): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('accept_operation', {
        request: { sessionId, operationId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('accept_operation', error, { sessionId, operationId, workspacePath });
    }
  }

   
  async rejectOperation(
    sessionId: string,
    operationId: string,
    workspacePath?: string,
  ): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('reject_operation', {
        request: { sessionId, operationId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('reject_operation', error, { sessionId, operationId, workspacePath });
    }
  }

   
  async rollbackSession(sessionId: string, workspacePath?: string): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('rollback_session', { 
        request: { sessionId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('rollback_session', error, { sessionId, workspacePath });
    }
  }

  async cleanupEmptySessions(): Promise<any> {
    try {
      return await api.invoke('cleanup_empty_sessions', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('cleanup_empty_sessions', error);
    }
  }

   
  async getSnapshotStats(
    workspacePath?: string,
    remoteScope: SnapshotRemoteScope = {},
  ): Promise<any> {
    try {
      const resolvedWorkspacePath = requireWorkspacePath(workspacePath);
      return await api.invoke('get_snapshot_system_stats', {
        request: { workspacePath: resolvedWorkspacePath, ...remoteScope }
      });
    } catch (error) {
      throw createTauriCommandError('get_snapshot_system_stats', error, { workspacePath });
    }
  }

   
  async getSnapshotSessions(
    workspacePath?: string,
    remoteScope: SnapshotRemoteScope = {},
  ): Promise<any> {
    try {
      const resolvedWorkspacePath = requireWorkspacePath(workspacePath);
      return await api.invoke('get_snapshot_sessions', {
        request: { workspacePath: resolvedWorkspacePath, ...remoteScope }
      });
    } catch (error) {
      throw createTauriCommandError('get_snapshot_sessions', error, { workspacePath });
    }
  }

   
  async getSessionOperations(sessionId: string, workspacePath?: string): Promise<any> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      return await api.invoke('get_session_operations', {
        request: { sessionId, ...scope }
      });
    } catch (error) {
      throw createTauriCommandError('get_session_operations', error, { sessionId, workspacePath });
    }
  }

  

   
  async recordTurnSnapshot(
    sessionId: string,
    turnIndex: number,
    modifiedFiles: string[],
    workspacePath?: string,
  ): Promise<void> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      await api.invoke('record_turn_snapshot', {
        session_id: sessionId,
        turn_index: turnIndex,
        modified_files: modifiedFiles,
        ...scope,
      });
    } catch (error) {
      throw createTauriCommandError('record_turn_snapshot', error, {
        sessionId,
        turnIndex,
        modifiedFiles,
        workspacePath,
      });
    }
  }

   
  async rollbackEntireSession(
    sessionId: string,
    deleteSession: boolean = true,
    workspacePath?: string,
  ): Promise<string[]> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      return await api.invoke('rollback_session', {
        request: {
          session_id: sessionId,
          delete_session: deleteSession,
          ...scope,
        }
      });
    } catch (error) {
      throw createTauriCommandError('rollback_session', error, { sessionId, workspacePath });
    }
  }

   
  async getSessionTurnSnapshots(
    sessionId: string,
    workspacePath?: string,
  ): Promise<TurnSnapshot[]> {
    try {
      const scope = requireSessionSnapshotScope(sessionId, workspacePath);
      
      const turnIndices: number[] = await api.invoke('get_session_turns', {
        request: {
          session_id: sessionId,
          ...scope,
        }
      });

      
      const turnSnapshots: TurnSnapshot[] = [];
      for (const turnIndex of turnIndices) {
        try {
          const files: string[] = await api.invoke('get_turn_files', {
            request: {
              session_id: sessionId,
              turn_index: turnIndex,
              ...scope,
            }
          });

          turnSnapshots.push({
            sessionId,
            turnIndex,
            modifiedFiles: files,
            timestamp: Date.now() / 1000, 
          });
        } catch (error) {
          log.warn('Failed to get turn files', { sessionId, turnIndex, error });
          // Continue processing the remaining turns.
          turnSnapshots.push({
            sessionId,
            turnIndex,
            modifiedFiles: [],
            timestamp: Date.now() / 1000,
          });
        }
      }

      return turnSnapshots;
    } catch (error) {
      throw createTauriCommandError('get_session_turns', error, { sessionId, workspacePath });
    }
  }

   
  async getFileChangeHistory(
    filePath: string,
    workspacePath?: string,
    remoteScope: SnapshotRemoteScope = {},
  ): Promise<FileChangeEntry[]> {
    try {
      const resolvedWorkspacePath = requireWorkspacePath(workspacePath);
      const result = await api.invoke('get_file_change_history', {
        request: { file_path: filePath, workspacePath: resolvedWorkspacePath, ...remoteScope }
      });
      return result as FileChangeEntry[];
    } catch (error) {
      throw createTauriCommandError('get_file_change_history', error, { filePath, workspacePath });
    }
  }

   
  async getAllModifiedFiles(
    workspacePath?: string,
    remoteScope: SnapshotRemoteScope = {},
  ): Promise<string[]> {
    try {
      const resolvedWorkspacePath = requireWorkspacePath(workspacePath);
      return await api.invoke('get_all_modified_files', {
        request: { workspacePath: resolvedWorkspacePath, ...remoteScope }
      });
    } catch (error) {
      throw createTauriCommandError('get_all_modified_files', error, { workspacePath });
    }
  }
}


export interface TurnSnapshot {
  sessionId: string;
  turnIndex: number;
  modifiedFiles: string[];
  timestamp: number;
}


export interface FileChangeEntry {
  session_id: string;
  turn_index: number;
  snapshot_id: string;
  timestamp: {
    secs_since_epoch: number;
    nanos_since_epoch: number;
  };
  operation_type: string;
  tool_name: string;
}


export const snapshotAPI = new SnapshotAPI();
