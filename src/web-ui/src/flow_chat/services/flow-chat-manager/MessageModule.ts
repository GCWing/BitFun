/**
 * Message handling module
 * Handles message sending, cancellation, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
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

const log = createLogger('MessageModule');

const ONE_SHOT_AGENT_TYPES_FOR_SESSION = new Set(['Init']);

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
      // Images are attached for multimodal primary models or reduced to text placeholders for text-only models.
      // We don't run a separate frontend "image pre-analysis" phase here.
      status: 'pending',
      startTime: Date.now()
    };

    context.flowChatStore.addDialogTurn(sessionId, dialogTurn);
    const pinRequest: FlowChatPinTurnToTopRequest = {
      sessionId,
      turnId: dialogTurnId,
      behavior: 'auto',
      source: 'send-message',
      pinMode: 'sticky-latest',
    };
    globalEventBus.emit(FLOWCHAT_PIN_TURN_TO_TOP_EVENT, pinRequest, 'MessageModule');

    const startOk = await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
      taskId: sessionId,
      dialogTurnId,
    });
    // START is only valid from IDLE/ERROR (see STATE_TRANSITIONS). If the previous turn left the
    // machine in PROCESSING/FINISHING, transition fails — but the backend still runs this turnId.
    // Sync context so TextChunk/ModelRound events are not dropped (turn_id_mismatch).
    if (!startOk) {
      const machine = stateMachineManager.get(sessionId);
      if (machine) {
        machine.getContext().currentDialogTurnId = dialogTurnId;
      }
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

    const currentAgentType = (agentType?.trim() || session.mode || 'agentic').trim();

    if (
      agentType?.trim() &&
      !ONE_SHOT_AGENT_TYPES_FOR_SESSION.has(currentAgentType) &&
      session.mode !== currentAgentType
    ) {
      context.flowChatStore.updateSessionMode(sessionId, currentAgentType);
    }

    try {
      await ensureBackendSession(context, sessionId);
    } catch (createError: any) {
      log.warn('Backend session create/restore failed', { sessionId: sessionId, error: createError });
    }

    await syncSessionModelSelection(context, sessionId, currentAgentType);

    const updatedSession = context.flowChatStore.getState().sessions.get(sessionId);
    if (!updatedSession) {
      throw new Error(`Session lost after adding dialog turn: ${sessionId}`);
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
