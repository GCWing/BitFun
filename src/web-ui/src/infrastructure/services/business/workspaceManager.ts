 

import { WorkspaceInfo, globalStateAPI } from '../../../shared/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceManager');


export type WorkspaceEvent = 
  | { type: 'workspace:opened', workspace: WorkspaceInfo }
  | { type: 'workspace:closed', workspaceId: string }
  | { type: 'workspace:switched', workspace: WorkspaceInfo }
  | { type: 'workspace:updated', workspace: WorkspaceInfo }
  | { type: 'workspace:loading', loading: boolean }
  | { type: 'workspace:error', error: string | null };


export type WorkspaceEventListener = (event: WorkspaceEvent) => void;


export interface WorkspaceState {
  currentWorkspace: WorkspaceInfo | null;
  recentWorkspaces: WorkspaceInfo[];
  loading: boolean;
  error: string | null;
}

 
class WorkspaceManager {
  private static instance: WorkspaceManager | null = null;
  private state: WorkspaceState;
  private listeners: Set<WorkspaceEventListener> = new Set();
  private isInitialized: boolean = false;
  private isInitializing: boolean = false;

  private constructor() {
    this.state = {
      currentWorkspace: null,
      recentWorkspaces: [],
      loading: true, // Start as loading until initialize() completes
      error: null,
    };
  }

   
  public static getInstance(): WorkspaceManager {
    if (!WorkspaceManager.instance) {
      WorkspaceManager.instance = new WorkspaceManager();
    }
    return WorkspaceManager.instance;
  }

   
  public getState(): WorkspaceState {
    return { ...this.state };
  }

   
  public addEventListener(listener: WorkspaceEventListener): () => void {
    this.listeners.add(listener);
    
    
    return () => {
      this.listeners.delete(listener);
    };
  }

   
  private emit(event: WorkspaceEvent): void {
    log.debug('Emitting event', { type: event.type });
    this.listeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Event listener execution error', { eventType: event.type, error });
      }
    });
  }

   
  private updateState(updates: Partial<WorkspaceState>, event?: WorkspaceEvent): void {
    const prevState = { ...this.state };
    this.state = { ...this.state, ...updates };
    
    log.debug('State updated', { updates });

    if (event) {
      this.emit(event);
    }
  }

   
  private setLoading(loading: boolean): void {
    this.updateState(
      { loading },
      { type: 'workspace:loading', loading }
    );
  }

   
  private setError(error: string | null): void {
    this.updateState(
      { error },
      { type: 'workspace:error', error }
    );
  }

   
  public async initialize(): Promise<void> {
    
    if (this.isInitialized || this.isInitializing) {
      return;
    }

    try {
      this.isInitializing = true;
      
      log.info('Initializing workspace state');

      
      const initResult = await globalStateAPI.initializeGlobalState();
      log.debug('Backend initialization completed', { result: initResult });

      
      const recentWorkspaces = await globalStateAPI.getRecentWorkspaces();

      log.debug('Recent workspaces loaded', {
        count: recentWorkspaces.length
      });

      
      const currentWorkspace = await globalStateAPI.getCurrentWorkspace();
      
      if (currentWorkspace) {
        log.info('Restored workspace detected', { workspaceName: currentWorkspace.name });
        
        
        this.updateState({
          currentWorkspace,
          recentWorkspaces,
          loading: false,
          error: null
        }, {
          type: 'workspace:opened',
          workspace: currentWorkspace
        });
      } else {
        log.debug('No restored workspace detected, waiting for user selection');
        
        
        
        
        this.updateState({
          currentWorkspace: null,
          recentWorkspaces,
          loading: false,
          error: null
        });
      }

      
      if (recentWorkspaces.length > 0) {
        this.emit({
          type: 'workspace:loading',
          loading: false
        });
      }

      this.isInitialized = true;
      log.info('Workspace state initialization completed');

    } catch (error) {
      log.error('Failed to initialize workspace state', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      this.updateState({
        currentWorkspace: null,
        recentWorkspaces: [],
        loading: false,
        error: errorMessage
      });
    } finally {
      this.isInitializing = false;
    }
  }

   
  public async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      
      this.updateState(
        { loading: true, error: null },
        { type: 'workspace:loading', loading: true }
      );

      log.info('Opening workspace', { path });

      
      const [workspace, recentWorkspaces] = await Promise.all([
        globalStateAPI.openWorkspace(path),
        globalStateAPI.getRecentWorkspaces()
      ]);
      
      log.info('Workspace opened', {
        id: workspace.id,
        name: workspace.name,
        rootPath: workspace.rootPath,
        type: workspace.workspaceType
      });

      
      this.updateState(
        {
          currentWorkspace: workspace,
          recentWorkspaces,
          loading: false,
          error: null
        },
        {
          type: 'workspace:opened',
          workspace
        }
      );

      
      globalStateAPI.startFileWatch(workspace.rootPath, true).catch(err => {
        log.warn('Failed to start file watch', { rootPath: workspace.rootPath, error: err });
      });

      return workspace;
    } catch (error) {
      log.error('Failed to open workspace', { path, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      
      this.updateState(
        { loading: false, error: errorMessage },
        { type: 'workspace:error', error: errorMessage }
      );
      throw error;
    }
  }

   
  public async closeWorkspace(): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Closing current workspace');

      const currentWorkspaceId = this.state.currentWorkspace?.id;
      const currentRootPath = this.state.currentWorkspace?.rootPath;
      
      
      if (currentRootPath) {
        try {
          await globalStateAPI.stopFileWatch(currentRootPath);
        } catch (error) {
          log.warn('Failed to stop file watch', { rootPath: currentRootPath, error });
        }
      }
      
      await globalStateAPI.closeWorkspace();

      
      const recentWorkspaces = await globalStateAPI.getRecentWorkspaces();

      this.updateState(
        {
          currentWorkspace: null,
          recentWorkspaces,
          loading: false,
          error: null
        },
        {
          type: 'workspace:closed',
          workspaceId: currentWorkspaceId || ''
        }
      );

      log.info('Workspace closed');
    } catch (error) {
      log.error('Failed to close workspace', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.setError(errorMessage);
      this.setLoading(false);
      throw error;
    }
  }

   
  public async switchWorkspace(workspace: WorkspaceInfo): Promise<WorkspaceInfo> {
    try {
      log.info('Switching workspace', {
        from: this.state.currentWorkspace?.name,
        to: workspace.name
      });
      
      
      if (this.state.currentWorkspace?.id === workspace.id) {
        log.debug('Workspace is already current, skipping switch');
        return workspace;
      }

      
      this.updateState(
        { loading: true, error: null },
        { type: 'workspace:loading', loading: true }
      );

      
      if (this.state.currentWorkspace?.rootPath) {
        globalStateAPI.stopFileWatch(this.state.currentWorkspace.rootPath).catch(err => {
          log.warn('Failed to stop file watch', { rootPath: this.state.currentWorkspace?.rootPath, error: err });
        });
      }

      
      if (this.state.currentWorkspace) {
        await globalStateAPI.closeWorkspace();
      }

      
      const newWorkspace = await globalStateAPI.openWorkspace(workspace.rootPath);
      
      log.info('New workspace opened', {
        id: newWorkspace.id,
        name: newWorkspace.name,
        rootPath: newWorkspace.rootPath
      });

      
      this.updateState(
        {
          currentWorkspace: newWorkspace,
          loading: false,
          error: null
        },
        {
          type: 'workspace:switched',
          workspace: newWorkspace
        }
      );

      
      globalStateAPI.getRecentWorkspaces().then(recentWorkspaces => {
        this.updateState(
          { recentWorkspaces },
          { type: 'workspace:updated', workspace: newWorkspace }
        );
      }).catch(err => {
        log.warn('Failed to load recent workspaces', { error: err });
      });

      
      globalStateAPI.startFileWatch(newWorkspace.rootPath, true).catch(err => {
        log.warn('Failed to start file watch', { rootPath: newWorkspace.rootPath, error: err });
      });

      return newWorkspace;
    } catch (error) {
      log.error('Failed to switch workspace', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      this.updateState(
        { loading: false, error: errorMessage },
        { type: 'workspace:error', error: errorMessage }
      );
      
      throw error;
    }
  }

   
  public async scanWorkspaceInfo(): Promise<WorkspaceInfo | null> {
    try {
      if (!this.state.currentWorkspace?.rootPath) {
        throw new Error('No current workspace available for scanning');
      }

      this.setLoading(true);
      this.setError(null);

      log.debug('Scanning workspace info');

      const updatedWorkspace = await globalStateAPI.scanWorkspaceInfo(this.state.currentWorkspace.rootPath);

      if (updatedWorkspace) {
        log.debug('Workspace info updated', {
          statistics: updatedWorkspace.statistics,
          languages: updatedWorkspace.languages
        });

        this.updateState(
          {
            currentWorkspace: updatedWorkspace,
            loading: false,
            error: null
          },
          {
            type: 'workspace:updated',
            workspace: updatedWorkspace
          }
        );
      } else {
        this.setLoading(false);
      }

      return updatedWorkspace;
    } catch (error) {
      log.error('Failed to scan workspace info', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.setError(errorMessage);
      this.setLoading(false);
      throw error;
    }
  }

   
  public async refreshRecentWorkspaces(): Promise<void> {
    try {
      const recentWorkspaces = await globalStateAPI.getRecentWorkspaces();
      
      this.updateState({
        recentWorkspaces
      });

      log.debug('Recent workspaces refreshed', { count: recentWorkspaces.length });
    } catch (error) {
      log.error('Failed to refresh recent workspaces', { error });
    }
  }

   
  public hasWorkspace(): boolean {
    return !!this.state.currentWorkspace;
  }

   
  public getWorkspaceName(): string {
    return this.state.currentWorkspace?.name || '';
  }

   
  public getWorkspacePath(): string {
    return this.state.currentWorkspace?.rootPath || '';
  }


}


export const workspaceManager = WorkspaceManager.getInstance();


export { WorkspaceManager };


