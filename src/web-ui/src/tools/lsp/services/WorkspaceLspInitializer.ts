/**
 * Workspace LSP initializer.
 *
 * Listens to workspace open/close/switch events and initializes the LSP system.
 * Optionally pre-starts servers based on project detection.
 */

import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { WorkspaceLspManager } from './WorkspaceLspManager';
import { lspConfigService } from './LspConfigService';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceLspInitializer');

interface ProjectInfo {
  languages: string[];
  primaryLanguage?: string;
  fileCount: Record<string, number>;
  projectTypes: string[];
  totalFiles: number;
}

interface InitializeWorkspaceOptions {
  prestartServers?: boolean;
}

class WorkspaceLspInitializer {
  private static instance: WorkspaceLspInitializer | null = null;
  private removeListener: (() => void) | null = null;
  
  private constructor() {
  }
  
  static getInstance(): WorkspaceLspInitializer {
    if (!this.instance) {
      this.instance = new WorkspaceLspInitializer();
    }
    return this.instance;
  }
  
  /** Start listening to workspace events. */
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
          await this.handleWorkspaceClosed();
          break;
          
        case 'workspace:switched':
          await this.handleWorkspaceSwitched(event.workspace.rootPath);
          break;
      }
    });
  }
  
  /** Stop listening. */
  stop(): void {
    if (this.removeListener) {
      this.removeListener();
      this.removeListener = null;
    }
  }
  
  private async handleWorkspaceOpened(workspacePath: string): Promise<void> {
    await this.initializeWorkspace(workspacePath, { prestartServers: true });
  }

  async initializeWorkspace(
    workspacePath: string,
    options: InitializeWorkspaceOptions = {}
  ): Promise<void> {
    try {
      const manager = WorkspaceLspManager.getOrCreate(workspacePath);
      
      await manager.initialize();
      
      log.info('LSP initialized for workspace', { workspacePath });

      if (!options.prestartServers || !lspConfigService.isAutoStartEnabled()) {
        return;
      }

      this.detectAndPreStartServers(workspacePath).catch(error => {
        log.error('Failed to detect and pre-start servers', { workspacePath, error });
      });
      
    } catch (error) {
      log.error('Failed to initialize LSP', { workspacePath, error });
    }
  }

  /** Detect project languages and pre-start corresponding servers (best-effort). */
  private async detectAndPreStartServers(workspacePath: string): Promise<void> {
    try {
      const projectInfo = await invoke<ProjectInfo>('lsp_detect_project', {
        request: { workspacePath }
      });

      if (!projectInfo.languages || projectInfo.languages.length === 0) {
        return;
      }

      log.debug('Project detected', { workspacePath, languages: projectInfo.languages });

      for (const language of projectInfo.languages) {
        this.preStartServer(workspacePath, language).catch(error => {
          log.error('Failed to pre-start server', { workspacePath, language, error });
        });
      }

    } catch (error) {
      log.warn('Project detection failed', { workspacePath, error });
    }
  }

  /** Pre-start a server for a given language (best-effort). */
  private async preStartServer(workspacePath: string, language: string): Promise<void> {
    try {
      await invoke('lsp_prestart_server', {
        request: {
          workspacePath,
          language
        }
      });
      
      log.debug('Server pre-started', { workspacePath, language });
    } catch (error) {
      log.error('Failed to pre-start server', { workspacePath, language, error });
      throw error;
    }
  }
  
  private async handleWorkspaceClosed(): Promise<void> {
    try {
      // TODO: clean up the manager for the currently active workspace (if needed).
    } catch (error) {
      log.error('Failed to cleanup LSP', { error });
    }
  }
  
  private async handleWorkspaceSwitched(workspacePath: string): Promise<void> {
    try {
      const manager = WorkspaceLspManager.getOrCreate(workspacePath);
      
      await manager.initialize();
      
      log.info('LSP initialized for switched workspace', { workspacePath });
    } catch (error) {
      log.error('Failed to initialize LSP', { workspacePath, error });
    }
  }
}

// Singleton export
export const workspaceLspInitializer = WorkspaceLspInitializer.getInstance();
