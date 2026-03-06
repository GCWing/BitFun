/**
 * Message handling module
 * Handles message sending, cancellation, and other operations
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { agentAPI } from '@/infrastructure/api';
import { aiExperienceConfigService } from '@/infrastructure/config/services';
import { notificationService } from '../../../shared/notification-system';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { generateTempTitle } from '../../utils/titleUtils';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, DialogTurn } from './types';
import { ensureBackendSession, retryCreateBackendSession } from './SessionModule';
import { cleanupSessionBuffers } from './TextChunkModule';
import type { ImageContextData as ImageInputContextData } from '@/infrastructure/api/service-api/ImageAnalysisAPI';

const log = createLogger('MessageModule');

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
    
    const dialogTurn: DialogTurn = {
      id: dialogTurnId,
      sessionId: sessionId,
      userMessage: {
        id: `user_${Date.now()}`,
        content: displayMessage || message,
        timestamp: Date.now()
      },
      modelRounds: [],
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

    const currentAgentType = agentType || 'agentic';
    
    let turnResponse;
    try {
      turnResponse = await agentAPI.startDialogTurn({
        sessionId: sessionId,
        userInput: message,
        turnId: dialogTurnId,
        agentType: currentAgentType,
        imageContexts: options?.imageContexts,
      });
    } catch (error: any) {
      if (error?.message?.includes('Session does not exist') || error?.message?.includes('Not found')) {
        log.warn('Backend session still not found, retrying creation', {
          sessionId: sessionId,
          dialogTurnsCount: updatedSession.dialogTurns.length
        });
        
        await retryCreateBackendSession(context, sessionId);
        
        turnResponse = await agentAPI.startDialogTurn({
          sessionId: sessionId,
          userInput: message,
          turnId: dialogTurnId,
          agentType: currentAgentType,
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
  context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generating');
  
  if (aiExperienceConfigService.isSessionTitleGenerationEnabled()) {
    agentAPI.generateSessionTitle(sessionId, message, 20)
      .then((_aiTitle) => {
      })
      .catch((error) => {
        log.debug('AI title generation failed, keeping temp title', { sessionId, error });
        context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generated');
      });
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
