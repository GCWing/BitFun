/**
 * Workspace Git initializer
 * 
 * Responsibilities:
 * - Monitor workspace open/close/switch events
 * - Auto-refresh/cleanup Git state
 * - Keep Git state synced with workspace changes
 */

import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { gitStateManager } from '../state/GitStateManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceGitInitializer');

class WorkspaceGitInitializer {
  private static instance: WorkspaceGitInitializer | null = null;
  private removeListener: (() => void) | null = null;
  private currentWorkspacePath: string | null = null;

  private constructor() {}

  static getInstance(): WorkspaceGitInitializer {
    if (!this.instance) {
      this.instance = new WorkspaceGitInitializer();
    }
    return this.instance;
  }

  start(): void {
    if (this.removeListener) {
      return;
    }

    this.removeListener = workspaceManager.addEventListener(async (event) => {
      switch (event.type) {
        case 'workspace:opened':
          await this.handleWorkspaceOpened(event.workspace.rootPath);
          break;

        case 'workspace:closed':
          await this.handleWorkspaceClosed(event.workspaceId);
          break;

        case 'workspace:switched':
          await this.handleWorkspaceSwitched(event.workspace.rootPath);
          break;
      }
    });

    const currentState = workspaceManager.getState();
    if (currentState.currentWorkspace) {
      this.handleWorkspaceOpened(currentState.currentWorkspace.rootPath);
    }
  }

  stop(): void {
    if (this.removeListener) {
      this.removeListener();
      this.removeListener = null;
    }
  }

  private async handleWorkspaceOpened(workspacePath: string): Promise<void> {
    try {
      this.currentWorkspacePath = workspacePath;
      await gitStateManager.refresh(workspacePath, {
        layers: ['basic'],
        reason: 'mount',
        force: true,
        source: 'workspace_git_initializer',
      });
    } catch (error) {
      log.error('Failed to initialize Git state', { workspacePath, error });
    }
  }

  private async handleWorkspaceClosed(workspaceId: string): Promise<void> {
    try {
      if (this.currentWorkspacePath) {
        gitStateManager.invalidateCache(this.currentWorkspacePath, ['basic', 'status', 'detailed']);
      }
      this.currentWorkspacePath = null;
    } catch (error) {
      log.error('Failed to cleanup Git state', { workspaceId, error });
    }
  }

  private async handleWorkspaceSwitched(workspacePath: string): Promise<void> {
    try {
      if (this.currentWorkspacePath && this.currentWorkspacePath !== workspacePath) {
        gitStateManager.invalidateCache(this.currentWorkspacePath, ['basic', 'status', 'detailed']);
      }
      this.currentWorkspacePath = workspacePath;
      await gitStateManager.refresh(workspacePath, {
        layers: ['basic'],
        reason: 'mount',
        force: true,
        source: 'workspace_git_initializer',
      });
    } catch (error) {
      log.error('Failed to initialize Git state for switched workspace', { workspacePath, error });
    }
  }
}

export const workspaceGitInitializer = WorkspaceGitInitializer.getInstance();
