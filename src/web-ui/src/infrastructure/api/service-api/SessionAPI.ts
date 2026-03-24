
import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SessionMetadata, DialogTurnData } from '@/shared/types/session-history';

function remoteConnField(remoteConnectionId?: string): Record<string, string> {
  return remoteConnectionId ? { remote_connection_id: remoteConnectionId } : {};
}

export class SessionAPI {
  async listSessions(workspacePath: string, remoteConnectionId?: string): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('list_persisted_sessions', {
        request: {
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('list_persisted_sessions', error, { workspacePath });
    }
  }

  async loadSessionTurns(
    sessionId: string,
    workspacePath: string,
    limit?: number,
    remoteConnectionId?: string
  ): Promise<DialogTurnData[]> {
    try {
      const request: Record<string, unknown> = {
        session_id: sessionId,
        workspace_path: workspacePath,
        ...remoteConnField(remoteConnectionId),
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
    remoteConnectionId?: string
  ): Promise<void> {
    try {
      await api.invoke('save_session_turn', {
        request: {
          turn_data: turnData,
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_turn', error, { turnData, workspacePath });
    }
  }

  async saveSessionMetadata(
    metadata: SessionMetadata,
    workspacePath: string,
    remoteConnectionId?: string
  ): Promise<void> {
    try {
      await api.invoke('save_session_metadata', {
        request: {
          metadata,
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_metadata', error, { metadata, workspacePath });
    }
  }

  async deleteSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string
  ): Promise<void> {
    try {
      await api.invoke('delete_persisted_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('delete_persisted_session', error, { sessionId, workspacePath });
    }
  }

  async touchSessionActivity(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string
  ): Promise<void> {
    try {
      await api.invoke('touch_session_activity', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('touch_session_activity', error, { sessionId, workspacePath });
    }
  }

  async loadSessionMetadata(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string
  ): Promise<SessionMetadata | null> {
    try {
      return await api.invoke('load_persisted_session_metadata', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath,
          ...remoteConnField(remoteConnectionId),
        }
      });
    } catch (error) {
      throw createTauriCommandError('load_persisted_session_metadata', error, { sessionId, workspacePath });
    }
  }
}

export const sessionAPI = new SessionAPI();
