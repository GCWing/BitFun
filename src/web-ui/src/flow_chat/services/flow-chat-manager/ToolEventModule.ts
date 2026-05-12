/**
 * Tool event handling module
 * Handles various tool lifecycle events
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { parsePartialJson } from '../../../shared/utils/partialJsonParser';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, FlowToolItem, ToolEventOptions, DialogTurn } from './types';
import { immediateSaveDialogTurn } from './PersistenceModule';
import { applyPendingAcpPermissionForTool } from './AcpPermissionToolCardModule';
import type {
  CancelledToolEvent,
  CompletedToolEvent,
  ConfirmationNeededToolEvent,
  EarlyDetectedToolEvent,
  FailedToolEvent,
  FlowToolEvent,
  ParamsPartialToolEvent,
  ProgressToolEvent,
  StartedToolEvent,
} from '../EventBatcher';

const log = createLogger('ToolEventModule');
const pendingTerminalSessionIds = new Map<string, string>();

interface ToolTerminalReadyEvent {
  tool_use_id: string;
  terminal_session_id: string;
}

/**
 * Unified tool event handler
 * Supports both main session and subagent scenarios
 */
export function processToolEvent(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  toolEvent: FlowToolEvent,
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
      flushPendingBatchedEvents(context);
      handleStarted(store, sessionId, turnId, dialogTurn, toolEvent, options);
      break;
    }
    
    case 'Completed': {
      flushPendingBatchedEvents(context);
      handleCompleted(context, store, sessionId, turnId, toolEvent, options, onTodoWriteResult);
      break;
    }
    
    case 'Failed': {
      flushPendingBatchedEvents(context);
      handleFailed(context, store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'Cancelled': {
      flushPendingBatchedEvents(context);
      handleCancelled(context, store, sessionId, turnId, toolEvent);
      break;
    }
    
    case 'ConfirmationNeeded': {
      flushPendingBatchedEvents(context);
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

function flushPendingBatchedEvents(context: FlowChatContext): void {
  if (context.eventBatcher.getBufferSize() > 0) {
    context.eventBatcher.flushNow();
  }
}

function updateToolItem(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolId: string,
  updates: Record<string, any>,
  silent = false
): void {
  if (silent) {
    store.updateModelRoundItemSilent(sessionId, turnId, toolId, updates as any);
    return;
  }

  store.updateModelRoundItem(sessionId, turnId, toolId, updates as any);
}

function applyPendingTerminalSessionId(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolId: string,
  silent = false
): void {
  const terminalSessionId = pendingTerminalSessionIds.get(toolId);
  if (!terminalSessionId) {
    return;
  }

  updateToolItem(store, sessionId, turnId, toolId, {
    terminalSessionId,
  }, silent);
  pendingTerminalSessionIds.delete(toolId);
}

function isTodoWriteSuccessResult(result: unknown): result is Record<string, unknown> {
  return typeof result === 'object' && result !== null && (result as { success?: unknown }).success === true;
}

function isWriteLikeToolName(toolName: string): boolean {
  return ['write', 'write_notebook', 'file_write', 'Write'].includes(toolName);
}

function shouldIgnoreParamsPartial(status: FlowToolItem['status'], toolName: string): boolean {
  if (isWriteLikeToolName(toolName)) {
    return ['completed', 'error', 'cancelled', 'pending_confirmation', 'confirmed'].includes(status);
  }

  return ['running', 'completed', 'error', 'cancelled', 'pending_confirmation', 'confirmed'].includes(status);
}

function applyParamsPartial(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: ParamsPartialToolEvent,
  silent = false
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  if (existingItem && existingItem.type === 'tool') {
    const existingToolItem = existingItem as FlowToolItem;
    const prevBuffer = existingToolItem._paramsBuffer || '';
    const isWriteTool = isWriteLikeToolName(toolEvent.tool_name);
    if (shouldIgnoreParamsPartial(existingToolItem.status, toolEvent.tool_name)) {
      return;
    }

    const incomingParams = toolEvent.params || '';
    const isWriteFullParamsSnapshot = isWriteTool && incomingParams.trimStart().startsWith('{');
    const newBuffer = isWriteFullParamsSnapshot ? incomingParams : prevBuffer + incomingParams;
    
    let parsedParams: Record<string, any> = {};
    try {
      parsedParams = parsePartialJson(newBuffer);
    } catch {
    }
    
    const isEditTool = ['edit', 'search_replace', 'Edit'].includes(toolEvent.tool_name);
    const hasContentField = parsedParams && ('content' in parsedParams || 'contents' in parsedParams);
    const hasNewString = parsedParams && 'new_string' in parsedParams;
    
    let status: 'streaming' | 'receiving' = 'streaming';
    if ((isWriteTool && hasContentField) || (isEditTool && hasNewString)) {
      status = 'receiving';
    }
    
    updateToolItem(store, sessionId, turnId, toolEvent.tool_id, {
      toolCall: {
        input: parsedParams,
        id: toolEvent.tool_id
      },
      partialParams: parsedParams,
      _paramsBuffer: newBuffer,
      status,
      isParamsStreaming: true,
      _contentSize: isWriteTool && hasContentField ? ((parsedParams.content || parsedParams.contents || '').length) : undefined
    }, silent);
    applyPendingTerminalSessionId(store, sessionId, turnId, toolEvent.tool_id, silent);
    applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
  }
}

function applyProgress(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: ProgressToolEvent,
  silent = false
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  if (existingItem) {
    updateToolItem(store, sessionId, turnId, toolEvent.tool_id, {
      _progressMessage: toolEvent.message,
      _progressPercentage: toolEvent.percentage
    }, silent);
  }
}

export function processToolParamsPartialInternal(
  sessionId: string,
  turnId: string,
  toolEvent: ParamsPartialToolEvent
): void {
  applyParamsPartial(FlowChatStore.getInstance(), sessionId, turnId, toolEvent, false);
}

export function processToolProgressInternal(
  sessionId: string,
  turnId: string,
  toolEvent: ProgressToolEvent
): void {
  applyProgress(FlowChatStore.getInstance(), sessionId, turnId, toolEvent, true);
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
  toolEvent: EarlyDetectedToolEvent,
  options?: ToolEventOptions
): void {
  flushPendingBatchedEvents(context);
  
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
    applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
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
    applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
  }
}

/**
 * Handle tool params partial update event
 */
function handleParamsPartial(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: ParamsPartialToolEvent
): void {
  applyParamsPartial(store, sessionId, turnId, toolEvent);
}

/**
 * Handle tool started event
 */
function handleStarted(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  dialogTurn: DialogTurn,
  toolEvent: StartedToolEvent,
  options?: ToolEventOptions
): void {
  const existingItem = store.findToolItem(sessionId, turnId, toolEvent.tool_id);
  
  const toolCallData = {
    input: toolEvent.params,
    id: toolEvent.tool_id,
    ...(typeof toolEvent.timeout_seconds === 'number' && {
      timeout_seconds: toolEvent.timeout_seconds
    })
  };

  if (existingItem) {
    store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
      toolCall: toolCallData,
      status: 'running',
      isParamsStreaming: false,
      partialParams: undefined
    } as any);
    applyPendingTerminalSessionId(store, sessionId, turnId, toolEvent.tool_id);
    applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
  } else {
    const toolItem: FlowToolItem = {
      id: toolEvent.tool_id,
      type: 'tool',
      toolName: toolEvent.tool_name,
      terminalSessionId: pendingTerminalSessionIds.get(toolEvent.tool_id),
      toolCall: toolCallData,
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
      pendingTerminalSessionIds.delete(toolEvent.tool_id);
      applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
    } else {
      const lastModelRound = dialogTurn.modelRounds[dialogTurn.modelRounds.length - 1];
      if (lastModelRound) {
        store.addModelRoundItem(sessionId, turnId, toolItem, lastModelRound.id);
        pendingTerminalSessionIds.delete(toolEvent.tool_id);
        applyPendingAcpPermissionForTool(store, toolEvent.tool_id);
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
  toolEvent: CompletedToolEvent,
  options?: ToolEventOptions,
  onTodoWriteResult?: (sessionId: string, turnId: string, result: any) => void
): void {
  if (!options?.isSubagent && toolEvent.tool_name === 'TodoWrite' && isTodoWriteSuccessResult(toolEvent.result)) {
    onTodoWriteResult?.(sessionId, turnId, toolEvent.result);
  }
  
  const updates = {
    toolResult: {
      result: toolEvent.result,
      success: true,
      resultForAssistant: toolEvent.result_for_assistant,
      duration_ms: toolEvent.duration_ms
    },
    status: 'completed' as const,
    requiresConfirmation: false,
    acpPermission: undefined,
    isParamsStreaming: false,
    endTime: Date.now(),
    durationMs: toolEvent.duration_ms,
    queueWaitMs: toolEvent.queue_wait_ms,
    preflightMs: toolEvent.preflight_ms,
    confirmationWaitMs: toolEvent.confirmation_wait_ms,
    executionMs: toolEvent.execution_ms
  };

  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, updates as any);

  store.clearSessionNeedsAttention(sessionId);

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
  toolEvent: FailedToolEvent
): void {
  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
    toolResult: {
      result: null,
      success: false,
      error: toolEvent.error,
      duration_ms: toolEvent.duration_ms
    },
    status: 'error',
    requiresConfirmation: false,
    acpPermission: undefined,
    endTime: Date.now(),
    durationMs: toolEvent.duration_ms,
    queueWaitMs: toolEvent.queue_wait_ms,
    preflightMs: toolEvent.preflight_ms,
    confirmationWaitMs: toolEvent.confirmation_wait_ms,
    executionMs: toolEvent.execution_ms
  } as any);

  store.clearSessionNeedsAttention(sessionId);

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
  toolEvent: CancelledToolEvent
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
    requiresConfirmation: false,
    acpPermission: undefined,
    endTime: Date.now(),
    durationMs: toolEvent.duration_ms,
    queueWaitMs: toolEvent.queue_wait_ms,
    preflightMs: toolEvent.preflight_ms,
    confirmationWaitMs: toolEvent.confirmation_wait_ms,
    executionMs: toolEvent.execution_ms
  } as any);

  store.clearSessionNeedsAttention(sessionId);

  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle tool confirmation needed event
 */
function handleConfirmationNeeded(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: ConfirmationNeededToolEvent
): void {
  store.updateModelRoundItem(sessionId, turnId, toolEvent.tool_id, {
    requiresConfirmation: true,
    status: 'pending_confirmation'
  } as any);

  const state = store.getState();
  const activeSessionId = state.activeSessionId;
  if (sessionId !== activeSessionId) {
    const attentionKind = toolEvent.tool_name === 'AskUserQuestion' ? 'ask_user' : 'tool_confirm';
    store.setSessionNeedsAttention(sessionId, attentionKind);
  }
}

/**
 * Handle tool execution progress event
 */
function handleProgress(
  store: FlowChatStore,
  sessionId: string,
  turnId: string,
  toolEvent: ProgressToolEvent
): void {
  applyProgress(store, sessionId, turnId, toolEvent);
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

export function handleToolTerminalReady(
  event: ToolTerminalReadyEvent
): void {
  const { tool_use_id, terminal_session_id } = event;
  if (!tool_use_id || !terminal_session_id) {
    return;
  }

  const store = FlowChatStore.getInstance();
  const state = store.getState();

  for (const [sessionId, session] of state.sessions) {
    for (const dialogTurn of session.dialogTurns) {
      const toolItem = store.findToolItem(sessionId, dialogTurn.id, tool_use_id);
      if (!toolItem) {
        continue;
      }

      store.updateModelRoundItem(sessionId, dialogTurn.id, tool_use_id, {
        terminalSessionId: terminal_session_id,
      } as any);
      pendingTerminalSessionIds.delete(tool_use_id);
      return;
    }
  }

  pendingTerminalSessionIds.set(tool_use_id, terminal_session_id);
  log.debug('Cached terminal session for pending tool item', {
    toolUseId: tool_use_id,
    terminalSessionId: terminal_session_id,
  });
}
