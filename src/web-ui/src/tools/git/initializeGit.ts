import { createLogger } from '@/shared/utils/logger';

import { workspaceGitInitializer } from './services/WorkspaceGitInitializer';

const log = createLogger('Git');

/**
 * Initialize Git module and start monitoring workspace changes.
 */
export function initializeGit(): void {
  try {
    workspaceGitInitializer.start();
    log.info('Git module initialized');
  } catch (error) {
    log.error('Failed to initialize Git module', { error });
  }
}
