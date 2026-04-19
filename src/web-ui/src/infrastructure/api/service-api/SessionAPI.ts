
import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SessionMetadata, DialogTurnData, SessionStorageScope } from '@/shared/types/session-history';

function remoteSessionFields(
  remoteConnectionId?: string,
  remoteSshHost?: string,
  storageScope?: SessionStorageScope
): Record<string, string> {
  const o: Record<string, string> = {};
  if (remoteConnectionId) {
    o.remote_connection_id = remoteConnectionId;
  }
  if (remoteSshHost) {
    o.remote_ssh_host = remoteSshHost;
  }
  if (storageScope) {
    o.storage_scope = storageScope;
  }
  return o;
}

export class SessionAPI {
  async listSessions(
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('list_persisted_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('list_persisted_sessions', error, { workspacePath });
    }
  }

  async loadSessionTurns(
    sessionId: string,
    workspacePath?: string,
    limit?: number,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<DialogTurnData[]> {
    try {
      const request: Record<string, unknown> = {
        session_id: sessionId,
        workspace_path: workspacePath,
        ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
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
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<void> {
    try {
      await api.invoke('save_session_turn', {
        request: {
          turn_data: turnData,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_turn', error, { turnData, workspacePath });
    }
  }

  async saveSessionMetadata(
    metadata: SessionMetadata,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<void> {
    try {
      await api.invoke('save_session_metadata', {
        request: {
          metadata,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_metadata', error, { metadata, workspacePath });
    }
  }

  async deleteSession(
    sessionId: string,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<void> {
    try {
      await api.invoke('delete_persisted_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('delete_persisted_session', error, { sessionId, workspacePath });
    }
  }

  async touchSessionActivity(
    sessionId: string,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<void> {
    try {
      await api.invoke('touch_session_activity', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('touch_session_activity', error, { sessionId, workspacePath });
    }
  }

  async loadSessionMetadata(
    sessionId: string,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: SessionStorageScope
  ): Promise<SessionMetadata | null> {
    try {
      return await api.invoke('load_persisted_session_metadata', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteSessionFields(remoteConnectionId, remoteSshHost, storageScope),
        }
      });
    } catch (error) {
      throw createTauriCommandError('load_persisted_session_metadata', error, { sessionId, workspacePath });
    }
  }
}

export const sessionAPI = new SessionAPI();
