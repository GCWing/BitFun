/**
 * LSP Integration Hook - Provides convenient LSP integration for the editor.
 * @module useLspIntegration
 */

import { useEffect } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { lspService } from '@/tools/lsp/services/LspService';

const log = createLogger('useLspIntegration');

/**
 * Initializes LSP and sets the workspace root.
 * Falls back gracefully if LSP fails - editor continues without language services.
 */
export function useLspInitialization(workspacePath?: string) {
  useEffect(() => {
    const initLsp = async () => {
      try {
        await lspService.initialize();
        
        if (workspacePath) {
          await lspService.openWorkspace(workspacePath);
        }
      } catch (error) {
        log.warn('Failed to initialize LSP', error);
      }
    };

    initLsp();
  }, [workspacePath]);
}















