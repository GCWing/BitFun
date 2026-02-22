/**
 * Tool module initialization.
 * Single entry point for all tool modules.
 */

import { initializeLsp } from './lsp';
import { initializeGit } from './git';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Tools');

export async function initializeAllTools(): Promise<void> {
  try {
    await initializeLsp();
    initializeGit();
    log.info('All tool modules initialized');
  } catch (error) {
    log.error('Failed to initialize tool modules', { error });
  }
}

// Export all tool modules.
export * from './editor';
export * from './file-system';
export * from './git';
export * from './lsp';
export * from './mermaid-editor';
export * from './snapshot_system';
export * from './terminal';
export * from './workspace';
