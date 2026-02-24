/**
 * Session management module
 * Handles session creation, switching, deletion, and other operations
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { agentAPI, globalAPI } from '@/infrastructure/api';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import type { FlowChatContext, SessionConfig } from './types';
import { saveNewSessionMetadata, touchSessionActivity, cleanupSaveState } from './PersistenceModule';

const log = createLogger('SessionModule');

/**
 * Get model's maximum token count
 */
export async function getModelMaxTokens(modelName?: string): Promise<number> {
  try {
    const configManager = await import('@/infrastructure/config/services/ConfigManager').then(m => m.configManager);
    const models = await configManager.getConfig<any[]>('ai.models') || [];
    
    if (modelName) {
      const model = models.find(m => m.name === modelName || m.id === modelName);
      if (model?.context_window) {
        return model.context_window;
      }
    }
    
    const defaultModels = await configManager.getConfig<Record<string, string>>('ai.default_models');
    const primaryModelId = defaultModels?.primary;
    
    if (primaryModelId) {
      const primaryModel = models.find(m => m.id === primaryModelId);
      if (primaryModel?.context_window) {
        return primaryModel.context_window;
      }
    }
    
    log.debug('Model context_window config not found, using default', { modelName });
    return 128128;
  } catch (error) {
    log.warn('Failed to get model max tokens', { modelName, error });
    return 128128;
  }
}

/**
 * Create new chat session (managed by backend)
 */
export async function createChatSession(
  context: FlowChatContext,
  config: SessionConfig,
  mode?: string
): Promise<string> {
  try {
    const sessionCount = context.flowChatStore.getState().sessions.size + 1;
    const sessionName = i18nService.t('flow-chat:session.newWithIndex', { count: sessionCount });
    
    const maxContextTokens = await getModelMaxTokens(config.modelName);
    const agentType = mode || 'agentic';
    
    const response = await agentAPI.createSession({
      sessionName,
      agentType,
      config: {
        modelName: config.modelName || 'default',
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        maxContextTokens: maxContextTokens,
        enableContextCompression: true,
      }
    });
    
    context.flowChatStore.createSession(
      response.sessionId, 
      config, 
      undefined,
      sessionName,
      maxContextTokens,
      mode
    );
    
    await saveNewSessionMetadata(response.sessionId, config, sessionName, mode);

    return response.sessionId;
  } catch (error) {
    log.error('Failed to create chat session', { config, error });
    
    notificationService.error('Failed to create chat session', {
      duration: 3000
    });
    throw error;
  }
}

/**
 * Switch to specified session
 */
export async function switchChatSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  try {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    
    if (session?.isHistorical) {
      try {
        const workspacePath = await globalAPI.getCurrentWorkspacePath();
        if (!workspacePath) {
          throw new Error('Cannot get workspace path');
        }
        
        await context.flowChatStore.loadSessionHistory(sessionId, workspacePath);
        
        try {
          await agentAPI.restoreSession(sessionId);
          
          context.flowChatStore.setState(prev => {
            const newSessions = new Map(prev.sessions);
            const sess = newSessions.get(sessionId);
            if (sess) {
              newSessions.set(sessionId, { ...sess, isHistorical: false });
            }
            return { ...prev, sessions: newSessions };
          });
        } catch (restoreError: any) {
          log.warn('Historical session restore failed, creating new session', { sessionId, error: restoreError });
          const currentSession = context.flowChatStore.getState().sessions.get(sessionId);
          if (currentSession) {
            await agentAPI.createSession({
              sessionId: sessionId,
              sessionName: currentSession.title || `Session ${sessionId.slice(0, 8)}`,
              agentType: currentSession.mode || 'agentic',
              config: {
                modelName: currentSession.config.modelName || 'default',
                enableTools: true,
                safeMode: true
              }
            });
            
            context.flowChatStore.setState(prev => {
              const newSessions = new Map(prev.sessions);
              const sess = newSessions.get(sessionId);
              if (sess) {
                newSessions.set(sessionId, { ...sess, isHistorical: false });
              }
              return { ...prev, sessions: newSessions };
            });
          }
        }
      } catch (error) {
        log.error('Failed to load session history', { sessionId, error });
        notificationService.warning('Failed to load session history, showing empty session', {
          duration: 3000
        });
      }
    }
    
    context.flowChatStore.switchSession(sessionId);

    touchSessionActivity(sessionId).catch(error => {
      log.debug('Failed to touch session activity', { sessionId, error });
    });
  } catch (error) {
    log.error('Failed to switch chat session', { sessionId, error });
    notificationService.error('Failed to switch session', {
      duration: 3000
    });
    throw error;
  }
}

/**
 * Delete session (cascading delete Terminal)
 */
export async function deleteChatSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  try {
    await context.flowChatStore.deleteSession(sessionId);
    context.processingManager.clearSessionStatus(sessionId);
    cleanupSaveState(context, sessionId);
  } catch (error) {
    log.error('Failed to delete chat session', { sessionId, error });
    notificationService.error('Failed to delete session', {
      duration: 3000
    });
    throw error;
  }
}

/**
 * Ensure backend session exists (check before sending message)
 */
export async function ensureBackendSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  
  const isHistoricalSession = session.isHistorical === true;
  const isFirstTurn = session.dialogTurns.length <= 1;
  const needsBackendSetup = isHistoricalSession || isFirstTurn;
  
  if (needsBackendSetup) {
    try {
      await agentAPI.restoreSession(sessionId);
      
      if (isHistoricalSession) {
        context.flowChatStore.setState(prev => {
          const newSessions = new Map(prev.sessions);
          const sess = newSessions.get(sessionId);
          if (sess) {
            newSessions.set(sessionId, { ...sess, isHistorical: false });
          }
          return { ...prev, sessions: newSessions };
        });
      }
    } catch (restoreError: any) {
      log.debug('Session restore failed, creating new session', { sessionId, error: restoreError });
      await agentAPI.createSession({
        sessionId: sessionId,
        sessionName: session.title || `Session ${sessionId.slice(0, 8)}`,
        agentType: session.mode || 'agentic',
        config: {
          modelName: session.config.modelName || 'default',
          enableTools: true,
          safeMode: true
        }
      });
    }
  }
}

/**
 * Retry creating backend session (retry after message send failure)
 */
export async function retryCreateBackendSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  
  await agentAPI.createSession({
    sessionId: sessionId,
    sessionName: session.title || `Session ${sessionId.slice(0, 8)}`,
    agentType: session.mode || 'agentic',
    config: {
      modelName: session.config.modelName || 'default',
      enableTools: true,
      safeMode: true
    }
  });
}
