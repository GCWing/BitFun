 

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

export interface WorkspaceInfo {
  name: string;
  rootPath: string;
  type: string;
  filesCount: number;
}

export interface UpdateAppStatusRequest {
  status: AppStatus;
}

export interface OpenWorkspaceRequest {
  path: string;
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

  async closeWorkspace(): Promise<void> {
    try {
      await api.invoke('close_workspace', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('close_workspace', error);
    }
  }

  async getCurrentWorkspace(): Promise<WorkspaceInfo | null> {
    try {
      return await api.invoke('get_current_workspace', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_current_workspace', error);
    }
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
