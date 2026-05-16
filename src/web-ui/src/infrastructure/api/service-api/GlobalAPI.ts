 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface ApplicationState {
  status: AppStatus;
  workspace?: WorkspaceInfo;
  version: string;
  uptime: number;
}

export interface AppStatus {
  isInitialized: boolean;
  hasError: boolean;
  errorMessage?: string;
}

export interface ProjectStatistics {
  totalFiles: number;
  totalLines: number;
  totalSize: number;
  filesByLanguage: Record<string, number>;
  filesByExtension: Record<string, number>;
  lastUpdated: string;
}

export interface WorkspaceIdentity {
  name?: string | null;
  creature?: string | null;
  vibe?: string | null;
  emoji?: string | null;
}

export interface WorkspaceWorktreeInfo {
  path: string;
  branch?: string | null;
  mainRepoPath: string;
  isMain: boolean;
}

export interface WorkspaceInfo {
  id: string;
  name: string;
  rootPath: string;
  workspaceType: string;
  workspaceKind: string;
  assistantId?: string | null;
  languages: string[];
  openedAt: string;
  lastAccessed: string;
  description?: string | null;
  tags: string[];
  statistics?: ProjectStatistics | null;
  identity?: WorkspaceIdentity | null;
  worktree?: WorkspaceWorktreeInfo | null;
  connectionId?: string;
  connectionName?: string;
  /** With `rootPath`, forms logical key `{sshHost}:{rootPath}`; local uses `localhost`. */
  sshHost?: string;
}

export interface UpdateAppStatusRequest {
  status: AppStatus;
}

export interface OpenWorkspaceRequest {
  path: string;
}

export interface OpenRemoteWorkspaceRequest {
  remotePath: string;
  connectionId: string;
  connectionName: string;
  /** Passed through to Rust so session files map to ~/.bitfun/remote_ssh/{host}/... before/during connect. */
  sshHost?: string;
}

export type CreateAssistantWorkspaceRequest = Record<string, never>;

export interface CloseWorkspaceRequest {
  workspaceId: string;
}

export interface SetActiveWorkspaceRequest {
  workspaceId: string;
}

export interface ReorderOpenedWorkspacesRequest {
  workspaceIds: string[];
}

export interface DeleteAssistantWorkspaceRequest {
  workspaceId: string;
}

export interface ResetAssistantWorkspaceRequest {
  workspaceId: string;
}

export interface ScanWorkspaceInfoRequest {
  workspacePath: string;
}

export class GlobalAPI {
   
  async initializeGlobalState(): Promise<string> {
    try {
      return await api.invoke('initialize_global_state', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('initialize_global_state', error);
    }
  }

   
  async getAppState(): Promise<ApplicationState> {
    try {
      return await api.invoke('get_app_state', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_app_state', error);
    }
  }

   
  async updateAppStatus(status: AppStatus): Promise<void> {
    try {
      await api.invoke('update_app_status', { 
        request: { status } 
      });
    } catch (error) {
      throw createTauriCommandError('update_app_status', error, { status });
    }
  }

   
  async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('open_workspace', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('open_workspace', error, { path });
    }
  }

  async openRemoteWorkspace(
    remotePath: string,
    connectionId: string,
    connectionName: string,
    sshHost?: string
  ): Promise<WorkspaceInfo> {
    try {
      const h = sshHost?.trim();
      return await api.invoke('open_remote_workspace', {
        request: {
          remotePath,
          connectionId,
          connectionName,
          ...(h ? { sshHost: h } : {}),
        },
      });
    } catch (error) {
      throw createTauriCommandError('open_remote_workspace', error, {
        remotePath,
        connectionId,
        connectionName,
        sshHost,
      });
    }
  }

  async createAssistantWorkspace(): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('create_assistant_workspace', {
        request: {},
      });
    } catch (error) {
      throw createTauriCommandError('create_assistant_workspace', error);
    }
  }

   
  async closeWorkspace(workspaceId: string): Promise<void> {
    try {
      await api.invoke('close_workspace', { 
        request: { workspaceId } 
      });
    } catch (error) {
      throw createTauriCommandError('close_workspace', error, { workspaceId });
    }
  }

  async setActiveWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('set_active_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('set_active_workspace', error, { workspaceId });
    }
  }

  async reorderOpenedWorkspaces(workspaceIds: string[]): Promise<void> {
    try {
      await api.invoke('reorder_opened_workspaces', {
        request: { workspaceIds }
      });
    } catch (error) {
      throw createTauriCommandError('reorder_opened_workspaces', error, { workspaceIds });
    }
  }

  async deleteAssistantWorkspace(workspaceId: string): Promise<void> {
    try {
      await api.invoke('delete_assistant_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('delete_assistant_workspace', error, { workspaceId });
    }
  }

  async resetAssistantWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('reset_assistant_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('reset_assistant_workspace', error, { workspaceId });
    }
  }

   
  // In-flight deduplicator: if many components call getCurrentWorkspace at the
  // same time (e.g. 20+ Markdown blocks mounting after a workspace switch) only
  // one Tauri IPC round-trip is made; all callers share the same Promise.
  private _getCurrentWorkspaceInFlight: Promise<WorkspaceInfo | null> | null = null;

  async getCurrentWorkspace(): Promise<WorkspaceInfo | null> {
    if (this._getCurrentWorkspaceInFlight) {
      return this._getCurrentWorkspaceInFlight;
    }
    this._getCurrentWorkspaceInFlight = (async () => {
      try {
        return await api.invoke<WorkspaceInfo | null>('get_current_workspace', {
          request: {}
        });
      } catch (error) {
        throw createTauriCommandError('get_current_workspace', error);
      } finally {
        this._getCurrentWorkspaceInFlight = null;
      }
    })();
    return this._getCurrentWorkspaceInFlight;
  }

   
  async getRecentWorkspaces(): Promise<WorkspaceInfo[]> {
    try {
      return await api.invoke('get_recent_workspaces', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_recent_workspaces', error);
    }
  }

  async removeRecentWorkspace(workspaceId: string): Promise<void> {
    try {
      await api.invoke('remove_recent_workspace', {
        request: { workspaceId },
      });
    } catch (error) {
      throw createTauriCommandError('remove_recent_workspace', error, { workspaceId });
    }
  }

  async cleanupInvalidWorkspaces(): Promise<number> {
    try {
      return await api.invoke('cleanup_invalid_workspaces');
    } catch (error) {
      throw createTauriCommandError('cleanup_invalid_workspaces', error);
    }
  }

  async getOpenedWorkspaces(): Promise<WorkspaceInfo[]> {
    try {
      return await api.invoke('get_opened_workspaces', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('get_opened_workspaces', error);
    }
  }

   
  async scanWorkspaceInfo(workspacePath: string): Promise<WorkspaceInfo | null> {
    try {
      return await api.invoke('scan_workspace_info', { 
        request: { workspacePath } 
      });
    } catch (error) {
      throw createTauriCommandError('scan_workspace_info', error, { workspacePath });
    }
  }

   
  async getCurrentWorkspacePath(): Promise<string | undefined> {
    try {
      const workspace = await this.getCurrentWorkspace();
      return workspace?.rootPath;
    } catch (error) {
      throw createTauriCommandError('get_current_workspace', error);
    }
  }
}


export const globalAPI = new GlobalAPI();
