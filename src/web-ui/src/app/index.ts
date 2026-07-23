/**
 * App feature exports.
 */

// Types
export * from './types';

// Services
export * from './services';

// React Hooks
export * from './hooks';

// Components
export * from './components';

// Default instance
export { appManager } from './services/AppManager';

// App feature initialization
import { appManager } from './services/AppManager';
import { globalEventBus } from '../infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('App');

export async function initializeAppFeature(): Promise<void> {
  log.info('Initializing App feature');
  
  try {
    // Listen for app shutdown to clean up resources
    globalEventBus.on('app:shutdown', () => {
      appManager.destroy();
    });
    
    globalEventBus.emit('feature:app:ready');
    log.info('App feature initialized');
  } catch (error) {
    log.error('Failed to initialize App feature', error);
    throw error;
  }
}

// App feature metadata
export const AppFeatureMetadata = {
  name: 'App',
  version: '1.0.0',
  description: 'Unified application layout and state management',
  dependencies: ['core'],
  capabilities: [
    'unified-layout',
    'state-management',
    'chat-sessions',
    'extension-management',
    'file-preview'
  ]
} as const;
