/**
 * Workspace management feature exports.
 */

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Workspace');

export * from './types';

export { default as WorkspaceManager } from './components/WorkspaceManager';

export async function initializeWorkspaceFeature(): Promise<void> {
  // No special initialization; components load on demand.
  log.info('Workspace feature ready');
}

export const WorkspaceFeatureMetadata = {
  name: 'Workspace',
  version: '1.0.0',
  description: 'Workspace and file system management',
  dependencies: ['core'],
  capabilities: [
    'file-system-operations',
    'workspace-management',
    'file-search',
    'file-watching',
    'directory-navigation'
  ]
} as const;
