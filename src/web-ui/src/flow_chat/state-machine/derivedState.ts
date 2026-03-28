/**
 * Derived state computation
 * Computes derived state for UI components based on state machine state
 * 
 * Design principles:
 * - Main state (currentState) determines macro behavior (input enabled, cancelable, etc.)
 * - Processing phase (processingPhase) determines detailed display (progress text, icons, etc.)
 */

import {
  SessionExecutionState,
  ProcessingPhase,
  SessionStateMachine,
  SessionDerivedState,
} from './types';

/** Optional live chat input draft while PROCESSING (mirrors input box); used so send mode stays `split` when user has typed a follow-up. */
export type DeriveSessionOptions = {
  processingInputDraftTrimmed?: string;
};

export function deriveSessionState(
  machine: SessionStateMachine,
  options?: DeriveSessionOptions
): SessionDerivedState {
  const { currentState, context } = machine;
  const { processingPhase } = context;
  const draftTrimmed =
    currentState === SessionExecutionState.PROCESSING ||
    currentState === SessionExecutionState.FINISHING ||
    currentState === SessionExecutionState.ERROR
      ? options?.processingInputDraftTrimmed?.trim() ?? ''
      : '';

  const plannerStats = context.planner?.todos
    ?       {
        completed: context.planner.todos.filter(t => t.status === 'completed').length,
        inProgress: context.planner.todos.filter(t => t.status === 'in_progress').length,
        pending: context.planner.todos.filter(t => t.status === 'pending').length
      }
    : null;

  const plannerProgress = plannerStats
    ? (plannerStats.completed / context.planner!.todos.length) * 100
    : 0;

  const isProcessing =
    currentState === SessionExecutionState.PROCESSING ||
    currentState === SessionExecutionState.FINISHING;
  const isError = currentState === SessionExecutionState.ERROR;
  const isIdle = currentState === SessionExecutionState.IDLE;
  const canCancel = currentState === SessionExecutionState.PROCESSING;
  
  return {
    isInputDisabled: false,
    
    showSendButton: !isProcessing,
    showCancelButton: canCancel,
    
    sendButtonMode: getSendButtonMode(
      currentState,
      processingPhase,
      context.queuedInput,
      context.pendingToolConfirmations.size > 0,
      draftTrimmed
    ),
    
    inputPlaceholder: 'How can I help you...',
    
    showPlanner: isProcessing &&
                 context.planner !== null &&
                 context.planner.isActive &&
                 context.planner.todos.length > 0,
    
    plannerProgress,
    plannerStats,
    
    showProgressBar: isProcessing,
    
    progressBarMode: getProgressBarMode(processingPhase),
    
    progressBarValue: getProgressBarValue(processingPhase, context),
    
    progressBarLabel: getProgressBarLabel(processingPhase, context),
    
    progressBarColor: getProgressBarColor(processingPhase),
    
    isProcessing,
    canCancel,
    canSendNewMessage: isIdle || isError,
    
    hasQueuedInput:
      (context.queuedInput?.trim()?.length ?? 0) > 0 ||
      ((currentState === SessionExecutionState.PROCESSING ||
        currentState === SessionExecutionState.FINISHING ||
        currentState === SessionExecutionState.ERROR) &&
        draftTrimmed.length > 0),
    queuedInput: context.queuedInput ?? null,
    
    hasError: isError,
    errorType: context.errorMessage ? detectErrorType(context.errorMessage) : null,
    canRetry: isError,
  };
}

function getSendButtonMode(
  state: SessionExecutionState,
  phase: ProcessingPhase | null,
  queuedInput: string | null,
  hasPendingConfirmations: boolean,
  processingDraftTrimmed: string
): SessionDerivedState['sendButtonMode'] {
  if (state === SessionExecutionState.ERROR) {
    const hasQueued = (queuedInput?.trim()?.length ?? 0) > 0 || processingDraftTrimmed.length > 0;
    return hasQueued ? 'split' : 'retry';
  }

  if (state === SessionExecutionState.PROCESSING || state === SessionExecutionState.FINISHING) {
    if (phase === ProcessingPhase.TOOL_CONFIRMING || hasPendingConfirmations) {
      return 'confirm';
    }
    if (state === SessionExecutionState.FINISHING) {
      return 'send';
    }
    const hasFollowUpDraft =
      (queuedInput?.trim()?.length ?? 0) > 0 || processingDraftTrimmed.length > 0;
    return hasFollowUpDraft ? 'split' : 'cancel';
  }

  return 'send';
}

function getProgressBarMode(phase: ProcessingPhase | null): SessionDerivedState['progressBarMode'] {
  switch (phase) {
    case ProcessingPhase.COMPACTING:
      return 'indeterminate';

    case ProcessingPhase.TOOL_CALLING:
      return 'segmented';
    
    case ProcessingPhase.STREAMING:
      return 'determinate';

    case ProcessingPhase.FINALIZING:
      return 'indeterminate';
    
    default:
      return 'indeterminate';
  }
}

function getProgressBarValue(
  phase: ProcessingPhase | null,
  context: SessionStateMachine['context']
): number {
  if (phase === ProcessingPhase.STREAMING) {
    const estimatedTotal = 500;
    const current = context.stats.textCharsGenerated;
    return Math.min((current / estimatedTotal) * 100, 90);
  }
  
  if (phase === ProcessingPhase.TOOL_CALLING && context.planner) {
    const { completed, inProgress, pending } = context.planner.todos.reduce(
      (acc, todo) => {
        acc[todo.status]++;
        return acc;
      },
      { completed: 0, inProgress: 0, pending: 0 } as Record<string, number>
    );

    const total = completed + inProgress + pending;
    return total > 0 ? (completed / total) * 100 : 0;
  }
  
  return 0;
}

function getProgressBarLabel(
  phase: ProcessingPhase | null,
  context: SessionStateMachine['context']
): string {
  switch (phase) {
    case ProcessingPhase.COMPACTING:
      return 'Compressing session context...';

    case ProcessingPhase.STARTING:
      return 'Connecting to AI...';
    
    case ProcessingPhase.THINKING:
      return 'Thinking...';
    
    case ProcessingPhase.STREAMING:
      const chars = context.stats.textCharsGenerated;
      const duration = context.stats.startTime 
        ? ((Date.now() - context.stats.startTime) / 1000).toFixed(1)
        : '0';
      return `Generating response (${chars} chars) · ${duration}s`;

    case ProcessingPhase.FINALIZING:
      return 'Finalizing response...';
    
    case ProcessingPhase.TOOL_CALLING:
      const toolsExecuted = context.stats.toolsExecuted;
      return `Executing tools... (${toolsExecuted} completed)`;
    
    case ProcessingPhase.TOOL_CONFIRMING:
      return 'Waiting for tool confirmation...';
    
    default:
      return '';
  }
}

function getProgressBarColor(phase: ProcessingPhase | null): string {
  switch (phase) {
    case ProcessingPhase.COMPACTING:
      return '#0f766e';

    case ProcessingPhase.STARTING:
      return '#3b82f6';
    
    case ProcessingPhase.THINKING:
      return '#3b82f6';
    
    case ProcessingPhase.STREAMING:
      return 'linear-gradient(90deg, #3b82f6, #8b5cf6)';
    
    case ProcessingPhase.TOOL_CALLING:
      return '#8b5cf6';
    
    case ProcessingPhase.TOOL_CONFIRMING:
      return '#f59e0b';
    
    default:
      return '#3b82f6';
  }
}

function detectErrorType(errorMessage: string): SessionDerivedState['errorType'] {
  const msg = errorMessage.toLowerCase();
  
  if (msg.includes('network') || msg.includes('timeout')) {
    return 'network';
  }
  
  if (msg.includes('model') || msg.includes('overload')) {
    return 'model';
  }
  
  if (msg.includes('permission') || msg.includes('api key') || msg.includes('401') || msg.includes('403')) {
    return 'permission';
  }
  
  return 'unknown';
}

