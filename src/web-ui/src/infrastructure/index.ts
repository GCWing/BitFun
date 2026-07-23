/**
 * Infrastructure unified exports.
 */

// Event bus
export * from './event-bus';

// API layer
export * from './api';

// Contexts (explicit exports to avoid name collisions)
export { ChatProvider } from './contexts/ChatProvider';
export { useChat } from './contexts/ChatContext';
export { WorkspaceProvider } from './contexts/WorkspaceProvider';
export { useWorkspaceContext } from './contexts/WorkspaceContext';

// Configuration
export * from './config';

// Infrastructure hooks
export * from './hooks/useAIInitialization';

// Infrastructure lifecycle
import { initializeConfigInfrastructure } from './config';
import { globalEventBus } from './event-bus';

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Infrastructure');

export async function initializeInfrastructure(): Promise<void> {
  log.info('Initializing infrastructure systems');
  
  try {
    // Initialize configuration infrastructure
    await initializeConfigInfrastructure();
    
    // Notify that infrastructure is ready
    globalEventBus.emit('infrastructure:ready');
    
    log.info('Infrastructure systems initialized successfully');
  } catch (error) {
    log.error('Failed to initialize infrastructure systems', error);
    throw error;
  }
}

export async function destroyInfrastructure(): Promise<void> {
  log.info('Shutting down infrastructure systems');
  
  // Notify shutdown
  globalEventBus.emit('infrastructure:shutdown');
  
  // Destroy event bus last
  globalEventBus.destroy();
}

// Backward-compatible aliases
export const initializeCore = initializeInfrastructure;
export const destroyCore = destroyInfrastructure;
