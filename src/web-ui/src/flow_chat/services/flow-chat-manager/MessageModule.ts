/**
 * Message handling module
 * Handles message sending, cancellation, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
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
import { globalEventBus } from '@/infrastructure/event-bus';
import {
  FLOWCHAT_PIN_TURN_TO_TOP_EVENT,
  type FlowChatPinTurnToTopRequest,
} from '../../events/flowchatNavigation';
import {
  isTransientBtwSession,
  sendMessageToTransientBtwSession,
} from '../BtwThreadService';

const log = createLogger('MessageModule');

const ONE_SHOT_AGENT_TYPES_FOR_SESSION = new Set(['Init']);

function acpClientIdFromMode(mode: string | undefined): string | null {
  const value = mode?.trim();
  if (!value?.startsWith('acp:')) return null;
  const clientId = value.slice('acp:'.length).trim();
  return clientId || null;
}

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

  let createdLocalTurnId: string | null = null;

  try {
    const refreshedSession = context.flowChatStore.getState().sessions.get(sessionId) ?? session;
    const currentAgentType = (agentType?.trim() || refreshedSession.mode || 'agentic').trim();
    const acpClientId = acpClientIdFromMode(currentAgentType);

    if (
      !acpClientId &&
      agentType?.trim() &&
      !ONE_SHOT_AGENT_TYPES_FOR_SESSION.has(currentAgentType) &&
      refreshedSession.mode !== currentAgentType
    ) {
      context.flowChatStore.updateSessionMode(sessionId, currentAgentType);
    }

    if (context.pendingHistoryLoads.has(sessionId)) {
      throw new Error('Session history is still restoring, please retry once loading finishes');
    }

    if (isTransientBtwSession(refreshedSession)) {
      if ((options?.imageContexts?.length ?? 0) > 0) {
        throw new Error('Transient /btw sessions do not support image attachments yet');
      }

      const parentSessionId = refreshedSession.parentSessionId?.trim();
      if (!parentSessionId) {
        throw new Error(`Transient /btw session is missing parentSessionId: ${sessionId}`);
      }

      await sendMessageToTransientBtwSession({
        parentSessionId,
        childSessionId: sessionId,
        question: message,
        childSessionName: refreshedSession.title,
        modelId: refreshedSession.config.modelName,
      });
      return;
    }

    if (!acpClientId) {
      await ensureBackendSession(context, sessionId);
    }

    const readySession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!readySession) {
      throw new Error(`Session lost before starting dialog turn: ${sessionId}`);
    }

    const isFirstMessage = readySession.dialogTurns.length === 0 && readySession.titleStatus !== 'generated';
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
      // Images are attached for multimodal primary models or reduced to text placeholders for text-only models.
      // We don't run a separate frontend "image pre-analysis" phase here.
      status: 'pending',
      startTime: Date.now()
    };

    context.flowChatStore.addDialogTurn(sessionId, dialogTurn);
    createdLocalTurnId = dialogTurnId;
    const pinRequest: FlowChatPinTurnToTopRequest = {
      sessionId,
      turnId: dialogTurnId,
      behavior: 'auto',
      source: 'send-message',
      pinMode: 'sticky-latest',
    };
    globalEventBus.emit(FLOWCHAT_PIN_TURN_TO_TOP_EVENT, pinRequest, 'MessageModule');

    const isRestoringHistoricalSession =
      readySession.isHistorical || context.pendingHistoryLoads.has(sessionId);
    if (isRestoringHistoricalSession) {
      context.processingManager.clearSessionStatus(sessionId);
      context.flowChatStore.deleteDialogTurn(sessionId, dialogTurnId);
      throw new Error('Session history is still restoring, please retry once loading finishes');
    }

    const startOk = await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
      taskId: sessionId,
      dialogTurnId,
    });
    if (!startOk) {
      const currentState = stateMachineManager.getCurrentState(sessionId);
      throw new Error(`Session is still busy finishing the previous turn (current state: ${currentState})`);
    }

    if (isFirstMessage) {
      handleTitleGeneration(context, sessionId, message);
    }

    context.processingManager.registerStatus({
      sessionId: sessionId,
      status: 'thinking',
      message: '',
      metadata: { sessionId: sessionId, dialogTurnId }
    });

    if (!acpClientId) {
      await syncSessionModelSelection(context, sessionId, currentAgentType);
    }

    const updatedSession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!updatedSession) {
      throw new Error(`Session lost after adding dialog turn: ${sessionId}`);
    }
    
    context.contentBuffers.set(sessionId, new Map());
    context.activeTextItems.set(sessionId, new Map());

    const workspacePath = updatedSession.workspacePath;
    
    if (acpClientId) {
      await ACPClientAPI.startDialogTurn({
        sessionId,
        clientId: acpClientId,
        userInput: message,
        originalUserInput: displayMessage || message,
        turnId: dialogTurnId,
        workspacePath,
        remoteConnectionId: updatedSession.remoteConnectionId,
        remoteSshHost: updatedSession.remoteSshHost,
      });
    } else {
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
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.ERROR_OCCURRED, {
        error: errorMessage
      });
      await stateMachineManager.transition(sessionId, SessionExecutionEvent.RESET);
    }
    
    const state = context.flowChatStore.getState();
    const currentSession = state.sessions.get(sessionId);
    if (createdLocalTurnId && currentSession) {
      context.flowChatStore.deleteDialogTurn(sessionId, createdLocalTurnId);
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
  // Show a readable placeholder immediately; backend later confirms the
  // authoritative title via AI or local fallback generation.
  context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generating');
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
      context.userCancelledSessionIds.add(sessionId);
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
