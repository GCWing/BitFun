/**
 * Session state machine type definitions
 * Based on industry best practices: XState, Redux, tokio
 */

/**
 * Session execution state.
 * 
 * Design philosophy:
 * - IDLE: idle, waiting for user input
 * - PROCESSING: running (dialog turn executing, including model thinking, output, tool execution, etc.)
 * - FINISHING: backend has reported completion, but the UI is still draining late data events
 * - ERROR: error state
 * 
 * Cancellation logic:
 * - User clicks cancel → immediately switch to IDLE
 * - UI immediately shows "cancelled"
 * - Asynchronously notify backend, no wait required
 * - No longer accept any events for that dialog turn
 * 
 * Sub-phases stored via context.processingPhase, do not affect main state
 */
export enum SessionExecutionState {
  IDLE = 'idle',
  PROCESSING = 'processing',
  FINISHING = 'finishing',
  ERROR = 'error',
}

/**
 * Session display/management state (seven-state projection).
 *
 * This is a distinct layer from the runtime {@link SessionExecutionState}
 * above. The runtime state owns execution and retry semantics, while this
 * enum is the user-facing projection used by the session sidebar, DAG member
 * nodes, and Session tool queries. The two layers do not conflict.
 */
export enum SessionDisplayState {
  /** Zero messages. */
  STANDBY = 'standby',
  /** A turn is actively executing. */
  PROCESSING = 'processing',
  /** Has conversation history and is idle. */
  COMPLETED = 'completed',
  /** Unresponsive beyond the watchdog timeout. */
  HUNG = 'hung',
  /** Interrupted. */
  INTERRUPTED = 'interrupted',
  /** Needs user attention (question mark). */
  PENDING_ATTENTION = 'pending_attention',
  /** Completed and already viewed (green dot cleared). */
  VIEWED = 'viewed',
}

export const SESSION_DISPLAY_STATES: readonly SessionDisplayState[] = [
  SessionDisplayState.STANDBY,
  SessionDisplayState.PROCESSING,
  SessionDisplayState.COMPLETED,
  SessionDisplayState.HUNG,
  SessionDisplayState.INTERRUPTED,
  SessionDisplayState.PENDING_ATTENTION,
  SessionDisplayState.VIEWED,
] as const;

/** True when a runtime execution state projects to a busy display state. */
export function isBusyDisplayState(state: SessionDisplayState): boolean {
  return state === SessionDisplayState.PROCESSING;
}

/**
 * Processing phase (only valid in PROCESSING state)
 * Used for UI detailed display, does not affect main state logic
 */
export enum ProcessingPhase {
  STARTING = 'starting',
  COMPACTING = 'compacting',
  THINKING = 'thinking',
  STREAMING = 'streaming',
  FINALIZING = 'finalizing',
  TOOL_CALLING = 'tool_calling',
  TOOL_CONFIRMING = 'tool_confirming',
}

/**
 * State transition events
 */
export enum SessionExecutionEvent {
  START = 'start',
  COMPACTION_STARTED = 'compaction_started',
  MODEL_ROUND_START = 'model_round_start',
  TEXT_CHUNK_RECEIVED = 'text_chunk_received',
  TOOL_DETECTED = 'tool_detected',
  TOOL_STARTED = 'tool_started',
  TOOL_COMPLETED = 'tool_completed',
  TOOL_CONFIRMATION_NEEDED = 'tool_confirmation_needed',
  TOOL_CONFIRMED = 'tool_confirmed',
  TOOL_REJECTED = 'tool_rejected',
  BACKEND_STREAM_COMPLETED = 'backend_stream_completed',
  FINISHING_SETTLED = 'finishing_settled',
  /** A background ExecCommand child process is still running for the session
   * after the dialog turn reported completion (R-WF-25). Keeps the session
   * visually PROCESSING instead of settling to IDLE. */
  BACKGROUND_COMMAND_RUNNING = 'background_command_running',
  USER_CANCEL = 'user_cancel',
  USER_CANCEL_FAILED = 'user_cancel_failed',
  ERROR_OCCURRED = 'error_occurred',
  RESET = 'reset',
}

export interface TodoItem {
  id: string;
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
}

/**
 * State machine context (runtime data)
 */
export interface SessionStateMachineContext {
  taskId: string | null;
  currentDialogTurnId: string | null;
  currentModelRoundId: string | null;
  pendingToolConfirmations: Set<string>;
  errorMessage: string | null;
  queuedInput: string | null;
  
  processingPhase: ProcessingPhase | null;
  
  planner: {
    todos: TodoItem[];
    isActive: boolean;
  } | null;
  
  stats: {
    startTime: number | null;
    textCharsGenerated: number;
    toolsExecuted: number;
  };
  
  version: number;
  lastUpdateTime: number;
  backendSyncedAt: number | null;
  errorRecovery: {
    errorCount: number;
    lastErrorTime: number | null;
    errorType: string | null;
    recoverable: boolean;
  };
}

export interface SessionStateMachine {
  sessionId: string;
  currentState: SessionExecutionState;
  context: SessionStateMachineContext;
  transitionHistory: StateTransition[];
}

export interface StateTransition {
  from: SessionExecutionState;
  event: SessionExecutionEvent;
  to: SessionExecutionState;
  timestamp: number;
  payload?: any;
  success: boolean;
}

/**
 * Derived state (for UI components)
 */
export interface SessionDerivedState {
  isInputDisabled: boolean;
  showSendButton: boolean;
  showCancelButton: boolean;
  sendButtonMode: 'send' | 'cancel' | 'split' | 'confirm' | 'retry';
  inputPlaceholder: string;
  
  showPlanner: boolean;
  plannerProgress: number;
  plannerStats: {
    completed: number;
    inProgress: number;
    pending: number;
  } | null;
  
  showProgressBar: boolean;
  progressBarMode: 'indeterminate' | 'determinate' | 'segmented';
  progressBarValue: number;
  progressBarLabel: string;
  progressBarColor: string;
  
  isProcessing: boolean;
  canCancel: boolean;
  canSendNewMessage: boolean;
  hasQueuedInput: boolean;
  queuedInput: string | null;

  hasError: boolean;
  errorType: 'network' | 'model' | 'permission' | 'unknown' | null;
  canRetry: boolean;
}

export type StateTransitionTable = Record<
  SessionExecutionState,
  Partial<Record<SessionExecutionEvent, SessionExecutionState>>
>;

