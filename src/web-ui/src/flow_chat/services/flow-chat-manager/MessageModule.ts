/**
 * Message handling module
 * Handles message sending, cancellation, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { aiExperienceConfigService } from '@/infrastructure/config/services';
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { notificationService } from '../../../shared/notification-system';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { generateTempTitle } from '../../utils/titleUtils';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, DialogTurn } from './types';
import { ensureBackendSession, retryCreateBackendSession } from './SessionModule';
import { cleanupSessionBuffers } from './TextChunkModule';
import type { ImageContextData as ImageInputContextData } from '@/infrastructure/api/service-api/ImageContextTypes';

const log = createLogger('MessageModule');

function normalizeModelSelection(
  modelId: string | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): string {
  const value = modelId?.trim();
  if (!value || value === 'auto') return 'auto';

  if (value === 'primary' || value === 'fast') {
    const resolvedDefaultId = value === 'primary' ? defaultModels.primary : defaultModels.fast;
    const matchedModel = models.find(model => model.id === resolvedDefaultId);
    return matchedModel ? value : 'auto';
  }

  const matchedModel = models.find(model =>
    model.id === value || model.name === value || model.model_name === value,
  );
  return matchedModel ? value : 'auto';
}

async function syncSessionModelSelection(
  context: FlowChatContext,
  sessionId: string,
  agentType: string,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }

  const [agentModels, allModels, defaultModels] = await Promise.all([
    configManager.getConfig<Record<string, string>>('ai.agent_models') || {},
    configManager.getConfig<AIModelConfig[]>('ai.models') || [],
    configManager.getConfig<DefaultModelsConfig>('ai.default_models') || {},
  ]);

  const desiredModelId = normalizeModelSelection(agentModels[agentType], allModels, defaultModels);
  const currentModelId = (session.config.modelName || 'auto').trim() || 'auto';
  const shouldForceAutoSync = desiredModelId === 'auto';
  if (!shouldForceAutoSync && desiredModelId === currentModelId) {
    return;
  }

  if (currentModelId !== desiredModelId) {
    context.flowChatStore.updateSessionModelName(sessionId, desiredModelId);
  }
  await agentAPI.updateSessionModel({
    sessionId,
    modelName: desiredModelId,
  });

  log.info('Session model synchronized before send', {
    sessionId,
    agentType,
    previousModelId: currentModelId,
    nextModelId: desiredModelId,
    forcedAutoSync: shouldForceAutoSync,
  });
}

/**
 * Send message and handle response
 * @param message - Message sent to backend
 * @param sessionId - Session ID
 * @param displayMessage - Optional, message for UI display
 * @param agentType - Agent type
 * @param switchToMode - Optional, switch UI mode selector to this mode (if not provided, mode remains unchanged)
 */
export async function sendMessage(
  context: FlowChatContext,
  message: string,
  sessionId: string,
  displayMessage?: string,
  agentType?: string,
  switchToMode?: string,
  options?: {
    imageContexts?: ImageInputContextData[];
    imageDisplayData?: Array<{ id: string; name: string; dataUrl?: string; imagePath?: string; mimeType?: string }>;
  }
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }

  // Switch UI mode if specified
  if (switchToMode && switchToMode !== session.mode) {
    context.flowChatStore.updateSessionMode(sessionId, switchToMode);
    window.dispatchEvent(new CustomEvent('bitfun:session-switched', {
      detail: { sessionId, mode: switchToMode }
    }));
  }

  try {
    const isFirstMessage = session.dialogTurns.length === 0 && session.titleStatus !== 'generated';
    
    const dialogTurnId = `dialog_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const hasImages = (options?.imageContexts?.length ?? 0) > 0;

    const dialogTurn: DialogTurn = {
      id: dialogTurnId,
      sessionId: sessionId,
      userMessage: {
        id: `user_${Date.now()}`,
        content: displayMessage || message,
        timestamp: Date.now(),
        hasImages,
        images: options?.imageDisplayData,
      },
      modelRounds: [],
      // Images are handled by the agent/tooling (e.g. `view_image`) or sent directly to multimodal
      // primary models. We don't run a separate frontend "image pre-analysis" phase here.
      status: 'pending',
      startTime: Date.now()
    };

    context.flowChatStore.addDialogTurn(sessionId, dialogTurn);
    
    await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
      taskId: sessionId,
      dialogTurnId,
    });

    if (isFirstMessage) {
      handleTitleGeneration(context, sessionId, message);
    }

    context.processingManager.registerStatus({
      sessionId: sessionId,
      status: 'thinking',
      message: '',
      metadata: { sessionId: sessionId, dialogTurnId }
    });

    const currentAgentType = agentType || session.mode || 'agentic';
    await syncSessionModelSelection(context, sessionId, currentAgentType);

    const updatedSession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!updatedSession) {
      throw new Error(`Session lost after adding dialog turn: ${sessionId}`);
    }
    
    try {
      await ensureBackendSession(context, sessionId);
    } catch (createError: any) {
      log.warn('Backend session create/restore failed', { sessionId: sessionId, error: createError });
    }
    
    context.contentBuffers.set(sessionId, new Map());
    context.activeTextItems.set(sessionId, new Map());

    const workspacePath = updatedSession.workspacePath;
    
    try {
      await agentAPI.startDialogTurn({
        sessionId: sessionId,
        userInput: message,
        originalUserInput: displayMessage || message,
        turnId: dialogTurnId,
        agentType: currentAgentType,
        workspacePath,
        imageContexts: options?.imageContexts,
      });
    } catch (error: any) {
      if (error?.message?.includes('Session does not exist') || error?.message?.includes('Not found')) {
        log.warn('Backend session still not found, retrying creation', {
          sessionId: sessionId,
          dialogTurnsCount: updatedSession.dialogTurns.length
        });
        
        await retryCreateBackendSession(context, sessionId);
        
        await agentAPI.startDialogTurn({
          sessionId: sessionId,
          userInput: message,
          originalUserInput: displayMessage || message,
          turnId: dialogTurnId,
          agentType: currentAgentType,
          workspacePath,
          imageContexts: options?.imageContexts,
        });
      } else {
        throw error;
      }
    }

    const sessionStateMachine = stateMachineManager.get(sessionId);
    if (sessionStateMachine) {
      sessionStateMachine.getContext().taskId = sessionId;
    }

  } catch (error) {
    log.error('Failed to send message', { sessionId: sessionId, error });
    
    const errorMessage = error instanceof Error ? error.message : 'Failed to send message';
    
    const currentState = stateMachineManager.getCurrentState(sessionId);
    if (currentState === SessionExecutionState.PROCESSING) {
      stateMachineManager.transition(sessionId, SessionExecutionEvent.ERROR_OCCURRED, {
        error: errorMessage
      });
    }
    
    const state = context.flowChatStore.getState();
    const currentSession = state.sessions.get(sessionId);
    if (currentSession && currentSession.dialogTurns.length > 0) {
      const lastDialogTurn = currentSession.dialogTurns[currentSession.dialogTurns.length - 1];
      context.flowChatStore.deleteDialogTurn(sessionId, lastDialogTurn.id);
    }
    
    notificationService.error(errorMessage, {
      title: 'Thinking process error',
      duration: 5000
    });
    
    throw error;
  }
}

function handleTitleGeneration(
  context: FlowChatContext,
  sessionId: string,
  message: string
): void {
  const tempTitle = generateTempTitle(message, 20);

  if (aiExperienceConfigService.isSessionTitleGenerationEnabled()) {
    // Set temp title while waiting for coordinator's auto-generated AI title
    // (delivered via SessionTitleGenerated event).
    context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generating');
  } else {
    context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generated');
  }
}

export async function cancelCurrentTask(context: FlowChatContext): Promise<boolean> {
  try {
    const state = context.flowChatStore.getState();
    const sessionId = state.activeSessionId;
    
    if (!sessionId) {
      log.debug('No active session to cancel');
      return false;
    }
    
    const currentState = stateMachineManager.getCurrentState(sessionId);
    const success = currentState === SessionExecutionState.PROCESSING 
      ? await stateMachineManager.transition(sessionId, SessionExecutionEvent.USER_CANCEL)
      : false;
    
    if (success) {
      markCurrentTurnItemsAsCancelled(context, sessionId);
      cleanupSessionBuffers(context, sessionId);
    }
    
    return success;
    
  } catch (error) {
    log.error('Failed to cancel current task', error);
    return false;
  }
}

export function markCurrentTurnItemsAsCancelled(
  context: FlowChatContext,
  sessionId: string
): void {
  const state = context.flowChatStore.getState();
  const session = state.sessions.get(sessionId);
  if (!session) return;
  
  const lastDialogTurn = session.dialogTurns[session.dialogTurns.length - 1];
  if (!lastDialogTurn) return;
  
  if (lastDialogTurn.status === 'completed' || lastDialogTurn.status === 'cancelled') {
    return;
  }
  
  lastDialogTurn.modelRounds.forEach(round => {
    round.items.forEach(item => {
      if (item.status === 'completed' || item.status === 'cancelled' || item.status === 'error') {
        return;
      }
      
      context.flowChatStore.updateModelRoundItem(sessionId, lastDialogTurn.id, item.id, {
        status: 'cancelled',
        ...(item.type === 'text' && { isStreaming: false }),
        ...(item.type === 'tool' && { 
          isParamsStreaming: false,
          endTime: Date.now()
        })
      } as any);
    });
  });
  
  context.flowChatStore.updateDialogTurn(sessionId, lastDialogTurn.id, turn => ({
    ...turn,
    status: 'cancelled',
    endTime: Date.now()
  }));
}
