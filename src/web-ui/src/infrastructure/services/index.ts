/**
 * Core services unified exports.
 *
 * Service layering: API → Business → Infrastructure
 */

// API layer: external IO and data access
export * from './api/aiService';

// Business layer: domain logic and orchestration
export * from './business/agentService';
export * from './business/workspaceManager';

// Infrastructure layer: low-level technical services
export * from './infra/contextManager';
