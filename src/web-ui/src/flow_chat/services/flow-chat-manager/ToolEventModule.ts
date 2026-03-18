/**
 * Tool event handling module
 * Handles various tool lifecycle events
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { parsePartialJson } from '../../../shared/utils/partialJsonParser';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, FlowToolItem, ToolEventOptions, DialogTurn } from './types';
import { immediateSaveDialogTurn } from './PersistenceModule';

const log = createLogger('ToolEventModule');

/**
 * Unified tool event handler
 * Supports both main session and subagent scenarios
 */
export function processToolEvent(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  toolEvent: any,
  options?: ToolEventOptions,
  onTodoWriteResult?: (sessionId: string, turnId: string, result: any) => void
): void {
  const store = FlowChatStore.getInstance();
  const state = store.getState();
  const session = state.sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (processToolEvent)', { sessionId });
    return;
  }

  const dialogTurn = session.dialogTurns.find((turn: DialogTurn) => turn.id === turnId);
  if (!dialogTurn) {
    log.debug('Dialog turn not found (processToolEvent)', { turnId });
    return;
  }

  switch (toolEvent.event_type) {
    case 'EarlyDetected': {
      handleEarlyDetected(context, store, sessionId, turnId, dialogTurn, toolEvent, options);
      break;
    }
    
    case 'ParamsPartial': {
      handleParamsPartial(store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'Started': {
      handleStarted(store, sessionId, turnId, dialogTurn, toolEvent, options);
      break;
    }
    
    case 'Completed': {
      handleCompleted(context, store, sessionId, turnId, toolEvent, options, onTodoWriteResult);
      break;
    }
    
    case 'Failed': {
      handleFailed(context, store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'Cancelled': {
      handleCancelled(context, store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'ConfirmationNeeded': {
      handleConfirmationNeeded(store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'Progress': {
      handleProgress(store, sessionId, turnId, toolEvent);
      break;
    }
    
    default:
      break;
  }
}

/**
 * Handle tool early detection event
 */
function handleEarlyDetected(
  context: FlowChatContext,
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  dialogTurn: DialogTurn,
  toolEvent: any,
  options?: ToolEventOptions
): void {
  context.eventBatcher.flushNow();
  
  const shouldDisplayInMainFlow = toolEvent.tool_name === 'submit_code_review' || 
                                 toolEvent.tool_name === 'AskUserQuestion';
  
  const preparingToolItem: FlowToolItem = {
    id: toolEvent.tool_id,
    type: 'tool',
    toolName: toolEvent.tool_name,
    toolCall: {
      input: {},
      id: toolEvent.tool_id
    },
    timestamp: options?.parentTimestamp ? options.parentTimestamp + 2 : Date.now(),
    status: 'preparing',
    requiresConfirmation: false,
    isParamsStreaming: true,
    startTime: options?.parentTimestamp ? options.parentTimestamp + 2 : Date.now(),
    ...(options?.isSubagent && !shouldDisplayInMainFlow && {
      isSubagentItem: true,
      parentTaskToolId: options.parentToolId,
      subagentSessionId: options.subagentSessionId
    })
  };
  
  if (options?.isSubagent && options.parentToolId && !shouldDisplayInMainFlow) {
    store.insertModelRoundItemAfterTool(sessionId, turnId, options.parentToolId, preparingToolItem);
  } else {
    let lastModelRound = dialogTurn.modelRounds[dialogTurn.modelRounds.length - 1];
    if (!lastModelRound) {
      const newRoundId = `round_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      lastModelRound = {
        id: newRoundId,
        index: 0,
        items: [],
        isStreaming: true,
        isComplete: false,
        status: 'streaming',
        startTime: Date.now()
      };
      store.addModelRound(sessionId, turnId, lastModelRound);
    }
    
    store.addModelRoundItem(sessionId, turnId, preparingToolItem, lastModelRound.id);
  }
}

/**
 * Handle tool params partial update event
 */
function handleParamsPartial(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  if (existingItem && existingItem.type === 'tool') {
    const prevBuffer = (existingItem as FlowToolItem)._paramsBuffer || '';
    const newBuffer = prevBuffer + (toolEvent.params || '');
    
    let parsedParams: Record<string, any> = {};
    try {
      parsedParams = parsePartialJson(newBuffer);
    } catch (e) {
    }
    
    const isWriteTool = ['write', 'write_notebook', 'file_write', 'Write'].includes(toolEvent.tool_name);
    const isEditTool = ['edit', 'search_replace', 'Edit'].includes(toolEvent.tool_name);
    const hasContentField = parsedParams && ('content' in parsedParams || 'contents' in parsedParams);
    const hasNewString = parsedParams && 'new_string' in parsedParams;
    
    let status: 'streaming' | 'receiving' = 'streaming';
    if ((isWriteTool && hasContentField) || (isEditTool && hasNewString)) {
      status = 'receiving';
    }
    
    store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
      toolCall: {
        input: parsedParams,
        id: toolEvent.tool_id
      },
      partialParams: parsedParams,
      _paramsBuffer: newBuffer,
      status,
      isParamsStreaming: true,
      _contentSize: hasContentField ? ((parsedParams.content || parsedParams.contents || '').length) : undefined
    } as any);
  }
}

/**
 * Handle tool started event
 */
function handleStarted(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  dialogTurn: DialogTurn,
  toolEvent: any,
  options?: ToolEventOptions
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  if (existingItem) {
    store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
      toolCall: {
        input: toolEvent.params,
        id: toolEvent.tool_id
      },
      status: 'running',
      isParamsStreaming: false,
      partialParams: undefined
    } as any);
  } else {
    const toolItem: FlowToolItem = {
      id: toolEvent.tool_id,
      type: 'tool',
      toolName: toolEvent.tool_name,
      toolCall: {
        input: toolEvent.params,
        id: toolEvent.tool_id
      },
      timestamp: options?.parentTimestamp ? options.parentTimestamp + 2 : Date.now(),
      status: 'running',
      requiresConfirmation: false,
      startTime: options?.parentTimestamp ? options.parentTimestamp + 2 : Date.now(),
      ...(options?.isSubagent && {
        isSubagentItem: true,
        parentTaskToolId: options.parentToolId,
        subagentSessionId: options.subagentSessionId
      })
    };
    
    if (options?.isSubagent && options.parentToolId) {
      store.insertModelRoundItemAfterTool(sessionId, turnId, options.parentToolId, toolItem);
    } else {
      const lastModelRound = dialogTurn.modelRounds[dialogTurn.modelRounds.length - 1];
      if (lastModelRound) {
        store.addModelRoundItem(sessionId, turnId, toolItem, lastModelRound.id);
      } else {
        log.error('Tool Started event without ModelRound (backend bug)', {
          sessionId,
          turnId,
          toolId: toolEvent.tool_id,
          toolName: toolEvent.tool_name
        });
      }
    }
  }
}

/**
 * Handle tool execution completed event
 */
function handleCompleted(
  context: FlowChatContext,
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any,
  options?: ToolEventOptions,
  onTodoWriteResult?: (sessionId: string, turnId: string, result: any) => void
): void {
  if (!options?.isSubagent && toolEvent.tool_name === 'TodoWrite' && toolEvent.result?.success) {
    onTodoWriteResult?.(sessionId, turnId, toolEvent.result);
  }
  
  const updates: any = {
    toolResult: {
      result: toolEvent.result,
      success: true,
      resultForAssistant: toolEvent.result_for_assistant,
      duration_ms: toolEvent.duration_ms
    },
    status: 'completed',
    endTime: Date.now()
  };

  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, updates);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle tool execution failed event
 */
function handleFailed(
  context: FlowChatContext,
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any
): void {
  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
    toolResult: {
      result: null,
      success: false,
      error: toolEvent.error,
      duration_ms: toolEvent.duration_ms
    },
    status: 'error',
    endTime: Date.now()
  } as any);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle tool cancelled event
 */
function handleCancelled(
  context: FlowChatContext,
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any
): void {
  const existingToolItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  const currentStatus = existingToolItem?.status;
  const finalStatus = currentStatus === 'confirmed' ? 'confirmed' : 'cancelled';
  
  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
    toolResult: {
      result: null,
      success: false,
      error: toolEvent.reason || 'User cancelled operation',
      duration_ms: toolEvent.duration_ms
    },
    status: finalStatus,
    endTime: Date.now()
  } as any);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle tool confirmation needed event
 */
function handleConfirmationNeeded(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any
): void {
  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
    requiresConfirmation: true,
    status: 'pending_confirmation'
  } as any);
}

/**
 * Handle tool execution progress event
 */
function handleProgress(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: any
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  if (existingItem) {
    store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
      _progressMessage: toolEvent.message,
      _progressPercentage: toolEvent.percentage
    } as any);
  }
}

/**
 * Handle backend independent tool execution progress event
 */
export function handleToolExecutionProgress(
  event: any
): void {
  const eventData = (event as any).value || event;
  const { tool_use_id, progress_message, percentage } = eventData;

  const store = FlowChatStore.getInstance();
  const state = store.getState();
  
  let found = false;
  
  for (const [sessionId, session] of state.sessions) {
    for (const dialogTurn of session.dialogTurns) {
      const toolItem = store.findToolItem(sessionId, dialogTurn.id, tool_use_id);
      
      if (toolItem) {
        const existingLogs: string[] = Array.isArray((toolItem as any)._progressLogs)
          ? (toolItem as any)._progressLogs
          : [];
        const lastLog = existingLogs.length > 0 ? existingLogs[existingLogs.length - 1] : undefined;
        const shouldAppend =
          typeof progress_message === 'string' &&
          progress_message.trim().length > 0 &&
          progress_message !== lastLog;
        const nextLogs = shouldAppend ? [...existingLogs, progress_message].slice(-200) : existingLogs;

        store.updateModelRoundItem(sessionId, dialogTurn.id, tool_use_id, {
          _progressMessage: progress_message,
          _progressPercentage: percentage,
          _progressLogs: nextLogs
        } as any);
        
        found = true;
        break;
      }
    }
    if (found) break;
  }
  
  if (!found) {
    log.debug('Tool item not found', { tool_use_id });
  }
}
