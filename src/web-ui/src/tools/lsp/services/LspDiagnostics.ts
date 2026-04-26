/**
 * LSP diagnostics helpers (primarily for debugging).
 */

import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import { measureAsync } from '@/shared/utils/timing';

const log = createLogger('LspDiagnostics');

export interface LspDiagnosticInfo {
  backendInitialized: boolean;
  workspaceManagers: string[];
  error?: string;
}

export class LspDiagnostics {
  /** Check backend LSP status. */
  static async checkBackendStatus(workspacePath: string): Promise<LspDiagnosticInfo> {
    try {
      const result = await measureAsync(() => invoke('lsp_get_all_server_states', {
        request: { workspacePath },
      }));
      log.debug('Got server states', {
        workspacePath,
        durationMs: result.durationMs,
      });
      
      return {
        backendInitialized: true,
        workspaceManagers: [workspacePath],
      };
    } catch (error) {
      log.error('Failed to get server states', { workspacePath, error });
      
      return {
        backendInitialized: false,
        workspaceManagers: [],
        error: error instanceof Error ? error.message : String(error)
      };
    }
  }
  
  /** Test a basic backend API call. */
  static async testBasicConnection(): Promise<boolean> {
    try {
      await invoke('get_health_status');
      return true;
    } catch (error) {
      log.error('Basic connection test failed', { error });
      return false;
    }
  }
  
  /** Run a fuller diagnostic routine and log results. */
  static async runFullDiagnostics(workspacePath: string): Promise<void> {
    log.info('Running full LSP diagnostics', { workspacePath });
    
    const basicConnection = await this.testBasicConnection();
    log.info('Basic connection test', { passed: basicConnection });
    
    if (!basicConnection) {
      log.error('Basic Tauri connection failed, cannot proceed with diagnostics');
      return;
    }
    
    const backendStatus = await this.checkBackendStatus(workspacePath);
    log.info('Backend status', {
      initialized: backendStatus.backendInitialized,
      workspaceManagers: backendStatus.workspaceManagers,
      error: backendStatus.error
    });
    
    if (backendStatus.backendInitialized) {
      log.info('LSP backend is operational', {
        troubleshooting: [
          'TypeScript LSP plugin is installed',
          'Server startup logs in backend',
          'File paths and URIs are correct'
        ]
      });
    } else {
      log.error('LSP backend is not operational', {
        fixes: [
          'Restart the application',
          'Check backend logs for initialization errors',
          'Ensure workspace is properly opened'
        ],
        error: backendStatus.error
      });
    }
    
    log.info('Diagnostics complete');
  }
}

if (typeof window !== 'undefined') {
  (window as any).LspDiagnostics = LspDiagnostics;
}
