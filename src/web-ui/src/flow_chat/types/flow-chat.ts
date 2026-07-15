/**
 * Flow Chat type definitions
 * Supports mixed streaming output.
 */

import type {
  DialogTurnKind,
  SessionKind,
  SessionTitleSource,
} from '@/shared/types/session-history';
import type { ReviewTargetEvidence, ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

// Base type for streaming items.
export interface FlowItem {
  id: string;
  type: 'text' | 'tool' | 'image-analysis' | 'thinking' | 'user-steering';
  timestamp: number;
  status: 'pending' | 'queued' | 'waiting' | 'preparing' | 'running' | 'streaming' | 'receiving' | 'completed' | 'cancelled' | 'rejected' | 'error' | 'analyzing' | 'pending_confirmation' | 'confirmed'; // Includes error, analyzing, and confirmation states.
  attemptId?: string;
  attemptIndex?: number;

  /**
   * Session-scoped subagent linkage.
   * Used by parent Task tools and subagent-targeted runtime status markers.
   */
  subagentSessionId?: string;
}

export interface FlowTextItem extends FlowItem {
  type: 'text';
  content: string;
  isStreaming: boolean;
  isMarkdown?: boolean;
  /**
   * Transient runtime status rendered in the current conversation only.
   * It is not persisted as assistant content.
   */
  runtimeStatus?: {
    phase: 'waiting_model' | 'streaming' | 'waiting_tool' | 'running_tool' | 'waiting_permission' | 'saving' | 'recovering';
    scope: 'main' | 'subagent' | 'tool';
    messageKey?: string;
  };
}

export interface FlowThinkingItem extends FlowItem {
  type: 'thinking';
  content: string;
  isStreaming: boolean;
  isCollapsed: boolean; // Whether the thinking block is collapsed.
}

export interface FlowToolItem extends FlowItem {
  type: 'tool';
  toolName: string;
  terminalSessionId?: string;
  interruptionReason?: 'app_restart' | 'retry_superseded';
  toolCall: {
    input: any;
    id: string;
    timeout_seconds?: number;
  };
  toolResult?: {
    result: any;
    success: boolean;
    resultForAssistant?: string;
    error?: string;
    duration_ms?: number;
  };
  requiresConfirmation?: boolean;
  userConfirmed?: boolean;
  acpPermission?: {
    permissionId: string;
    sessionId?: string;
    toolCallId?: string;
    requestedAt: number;
    options?: Array<{
      optionId: string;
      name: string;
      kind: 'allow_once' | 'allow_always' | 'reject_once' | 'reject_always';
    }>;
    toolCall?: {
      toolCallId?: string;
      title?: string;
      rawInput?: unknown;
      content?: unknown;
    };
  };
  aiIntent?: string; // AI rationale for calling the tool.
  startTime?: number;  // Tool start time.
  confirmationTimeoutAt?: number;
  endTime?: number;    // Tool end time.
  durationMs?: number;
  queueWaitMs?: number;
  preflightMs?: number;
  confirmationWaitMs?: number;
  executionMs?: number;

  /** Subagent model identity captured on the parent Task tool. */
  subagentModelId?: string;
  subagentModelDisplayName?: string;

  /** Child dialog turn produced by this parent Task call. */
  subagentDialogTurnId?: string;
  
  // Streaming parameter buffering.
  isParamsStreaming?: boolean;  // Params are streaming in.
  partialParams?: Record<string, any>;  // Partial params during streaming.
  _paramsBuffer?: string;  // Internal buffer for accumulated params.
}

export interface ToolRejectOptions {
  permissionOptionId?: string;
  instruction?: string;
}

export interface FlowImageAnalysisItem extends FlowItem {
  type: 'image-analysis';
  imageContext: import('@/shared/types/context').ImageContext;
  result?: ImageAnalysisResult | null;
  error?: string;
}

/**
 * A user-authored "steering" message injected mid-turn via the
 * `steer_dialog_turn` Tauri command. Rendered inline inside the running
 * model round so the user can see the message they steered the agent with.
 */
export interface FlowUserSteeringItem extends FlowItem {
  type: 'user-steering';
  steeringId: string;
  content: string;
  /** Round index reported by the backend at injection time. */
  roundIndex: number;
}

export type AnyFlowItem =
  | FlowTextItem
  | FlowThinkingItem
  | FlowToolItem
  | FlowImageAnalysisItem
  | FlowUserSteeringItem;

export interface ImageAnalysisResult {
  image_id: string;
  summary: string;              // Short summary.
  detailed_description: string; // Detailed description.
  detected_elements: string[];  // Key detected elements.
  confidence: number;           // Confidence score (0-1).
  analysis_time_ms: number;     // Analysis duration.
}

export interface ModelRoundRenderHints {
  /**
   * Keep all round items in the normal transcript instead of merging
   * collapsible tools and adjacent narrative into an explore group.
   */
  disableExploreGrouping?: boolean;
}

export interface ModelRoundAttempt {
  id: string;
  index: number;
  status: 'streaming' | 'completed' | 'superseded' | 'failed' | 'cancelled';
  items: AnyFlowItem[];
}

// Model round: output from a single model call.
export interface ModelRound {
  id: string;
  index: number;
  roundGroupId?: string;
  items: AnyFlowItem[];
  attempts?: ModelRoundAttempt[];
  historyRounds?: ModelRound[];
  isStreaming: boolean;
  isComplete: boolean;
  status: 'pending' | 'streaming' | 'completed' | 'cancelled' | 'rejected' | 'error' | 'pending_confirmation';
  startTime: number;
  endTime?: number;
  durationMs?: number;
  providerId?: string;
  modelId?: string;
  modelAlias?: string;
  firstChunkMs?: number;
  firstVisibleOutputMs?: number;
  streamDurationMs?: number;
  attemptCount?: number;
  failureCategory?: string;
  tokenDetails?: unknown;
  error?: string;
  renderHints?: ModelRoundRenderHints;
}

// Token usage stats.
export interface TokenUsage {
  inputTokens: number;
  outputTokens?: number;
  totalTokens: number;
  timestamp: number;
}

export interface AcpContextUsage {
  used: number;
  size: number;
  cost?: {
    amount: number;
    currency: string;
  };
  timestamp: number;
}

// Dialog turn: user input + full AI response across model rounds.
export interface DialogTurn {
  id: string;
  sessionId: string; // Used for event filtering.
  kind?: DialogTurnKind;
  /**
   * Mode used when this user-facing dialog turn was submitted.
   * Local utility turns may leave this empty.
   */
  agentType?: string;
  userMessage: {
    id: string;
    content: string;
    timestamp: number;
    hasImages?: boolean;
    metadata?: Record<string, any>;
    images?: Array<{
      id: string;
      name: string;
      dataUrl?: string;
      imagePath?: string;
      mimeType?: string;
    }>;
  };
  
  // Image analysis phase (only when images exist).
  imageAnalysisPhase?: {
    items: FlowImageAnalysisItem[];
    status: 'analyzing' | 'completed' | 'error';
    startTime: number;
    endTime?: number;
  };
  
  enhancedMessage?: string;
  
  modelRounds: ModelRound[];  // Model rounds in chronological order.
  status: 'pending' | 'image_analyzing' | 'processing' | 'finishing' | 'completed' | 'cancelling' | 'cancelled' | 'error'; // Includes image_analyzing.
  startTime: number;
  endTime?: number;
  error?: string;
  tokenUsage?: TokenUsage;
  todos?: TodoItem[];
  backendTurnIndex?: number;
  /** Whether the turn completed successfully. */
  success?: boolean;
  /** Why the turn finished. */
  finishReason?: string;
  /** Whether the turn produced a final assistant response visible to the user. */
  hasFinalResponse?: boolean;
}

export interface FlowChatState {
  sessions: Map<string, Session>;
  activeSessionId: string | null;
}

export interface TodoItem {
  id: string;
  content: string; // Imperative task description.
  status: 'pending' | 'in_progress' | 'completed';
}

export type SessionHistoryState =
  | 'new'
  | 'metadata-only'
  | 'hydrating'
  | 'ready'
  | 'failed';

export type SessionContextRestoreState =
  | 'ready'
  | 'pending'
  | 'failed';

// Session state.
export interface Session {
  sessionId: string;
  title?: string;
  /**
   * Untouched default sessions keep an i18n key so locale changes can re-render
   * their title. Once a real title is generated or renamed, we freeze it as text.
   */
  titleSource?: SessionTitleSource;
  titleI18nKey?: string;
  titleI18nParams?: Record<string, unknown>;
  titleStatus?: 'generating' | 'generated' | 'failed';
  dialogTurns: DialogTurn[];
  
  // Derived status from deriveSessionStatus():
  // - 'active': sessionId === activeSessionId
  // - 'error': state machine state === ERROR
  // - 'idle': otherwise
  status: 'active' | 'idle' | 'error';
  /** Persisted backend status retained while historical turns are metadata-only. */
  persistedStatus?: 'active' | 'archived' | 'completed';
  
  config: SessionConfig;
  createdAt: number;
  lastActiveAt: number;
  lastFinishedAt?: number;
  updatedAt?: number;
  
  // Persist the last error; real-time errors come from context.errorMessage.
  error: string | null;
  
  // Historical sessions are persisted and require lazy loading.
  isHistorical?: boolean;

  /**
   * Lazy history lifecycle for persisted sessions:
   * - 'new': an empty local session that should show the normal welcome state.
   * - 'metadata-only': persisted metadata is visible, but turns have not hydrated yet.
   * - 'hydrating': history is currently being restored / loaded.
   * - 'ready': turns are available, or the session no longer needs lazy history.
   * - 'failed': hydrate failed and the UI should offer retry instead of showing a new session.
   */
  historyState?: SessionHistoryState;

  /**
   * Backend runtime-context lifecycle for sessions restored through the fast
   * history view path. Turns may be visible while model context is still loaded
   * lazily; message sending must ensure this becomes 'ready' first.
   */
  contextRestoreState?: SessionContextRestoreState;

  /**
   * True when the session currently contains only a tail preview of persisted
   * history. Destructive history actions must wait for the full history hydrate
   * so UI indexes cannot drift from persisted backend turn indexes.
   */
  isPartial?: boolean;
  loadedTurnCount?: number;
  totalTurnCount?: number;
  
  todos?: TodoItem[];
  
  currentTokenUsage?: TokenUsage;
  currentAcpContextUsage?: AcpContextUsage;
  maxContextTokens?: number;
  
  /**
   * Current/default mode selection in the chat input for this session.
   * This controls what the next dialog turn should use by default.
   */
  mode?: string;
  /**
   * Mode of the last surviving user dialog turn in the current session
   * history. Rollback and turn truncation should follow this value.
   */
  lastUserDialogMode?: string;
  /**
   * Mode of the most recent user submission accepted by the runtime.
   * This is used for prompt-cache guard semantics and does not rewind on
   * rollback.
   */
  lastSubmittedMode?: string;

  // Workspace this session belongs to. Used for sidebar display filtering.
  // Sessions are always kept in store for event processing; only display is filtered.
  workspacePath?: string;

  /** Stable backend id — always set for new sessions; do not infer workspace from path alone. */
  workspaceId?: string;

  /** SSH remote: same `workspacePath` on different hosts must not share coordinator/persistence. */
  remoteConnectionId?: string;

  /** SSH config host for `~/.bitfun/remote_ssh/{host}/...` session paths when disconnected. */
  remoteSshHost?: string;

  /**
   * Optional parent session id for hierarchical sessions.
   * Used by /btw "side threads" and potentially other derived sessions.
   */
  parentSessionId?: string;

  /** Session kind for UI grouping. */
  sessionKind: SessionKind;

  /**
   * For hidden subagent sessions, records which parent Task tool launched it.
   * Helps reopen the real child session from parent task cards and header lists.
   */
  parentToolCallId?: string;

  /** Logical subagent id / type used to launch this hidden subagent session. */
  subagentType?: string;

  /** Whether `/goal` mode is active for this session. */
  goalModeActive?: boolean;

  /** Latest thread goal snapshot for UI (/goal menu, edit, resume). */
  threadGoal?: {
    goalId: string;
    objective: string;
    status: string;
    tokensUsed?: number;
    tokenBudget?: number | null;
    timeUsedSeconds?: number;
    updatedAt?: number;
    autoContinuationCount?: number;
  };

  /**
   * Lightweight markers for /btw threads created from this session.
   * Stored only on the parent session for quick navigation.
   */
  btwThreads?: Array<{
    requestId: string;
    childSessionId: string;
    title: string;
    status: 'running' | 'done' | 'error';
    createdAt: number;
    parentDialogTurnId?: string;
    /** 1-based turn index in the parent session when /btw was asked (best-effort). */
    parentTurnIndex?: number;
    error?: string;
  }>;

  /**
   * For /btw child sessions: where this side thread was asked from in the parent session.
   * This is best-effort and may be missing for older sessions.
   */
  btwOrigin?: {
    requestId?: string;
    parentSessionId?: string;
    parentDialogTurnId?: string;
    parentTurnIndex?: number;
  };

  /**
   * Set when a session finishes (completed / error / cancelled) while not the active session.
   * Cleared after the user switches to it and the content renders.
   * 'completed' → green dot, 'error' → red dot, 'interrupted' → red dot (partial stream recovery).
   */
  hasUnreadCompletion?: 'completed' | 'error' | 'interrupted';

  /**
   * Set when a session requires user attention while not the active session.
   * This is a high-priority alert that takes precedence over hasUnreadCompletion.
   * 'ask_user' → session has pending AskUserQuestion waiting for answer
   * 'tool_confirm' → session has pending tool confirmations
   * Cleared when the user switches to the session or the pending action is resolved.
   */
  needsUserAttention?: 'ask_user' | 'tool_confirm';

  /** Per-run reviewer manifest for Deep Review child sessions. */
  deepReviewRunManifest?: ReviewTeamRunManifest;

  /** Immutable target identity used to associate Review results with a PR or Git target. */
  reviewTargetEvidence?: ReviewTargetEvidence;

  /** Original file scope for Review remediation follow-ups. */
  reviewTargetFilePaths?: string[];

  /**
   * Runtime-only session that should stay in memory but never be persisted or
   * shown in the main session navigation.
   */
  isTransient?: boolean;

  /** Transient UI session backed by a real agent session. */
  agentBackedTransient?: boolean;
}

export interface SessionConfig {
  modelName?: string;
  agentType?: string;
  context?: Record<string, string>;
  workspacePath?: string;
  /** Binds session to `WorkspaceInfo.id` (path alone is insufficient for remotes). */
  workspaceId?: string;
  /** Disambiguates sessions when multiple remote workspaces share the same `workspacePath`. */
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

/**
 * A user message queued by the frontend while the session's current dialog turn
 * is still running. Items are persisted per-session and consumed in FIFO order
 * once the session returns to IDLE; users may also "send now" to inject the item
 * mid-turn (Codex-style steering) via the new `steer_dialog_turn` Tauri command.
 */
export interface QueuedMessage {
  id: string;
  sessionId: string;
  /** Rendered content sent to the model (may equal `displayMessage`). */
  content: string;
  /** Original user-visible text used by the queue UI and steering display. */
  displayMessage?: string;
  timestamp: number;
  /**
   * Lifecycle:
   * - `queued`: waiting in FIFO; eligible for auto-drain when session goes IDLE.
   * - `sending`: drain in progress (becoming a new dialog turn).
   * - `sending_now`: user explicitly steered the running turn with this item.
   * - `failed`: auto-restored from a failed turn — auto-drain is suppressed
   *   and the user must edit / send-now / delete to clear it (avoids reentry
   *   into the same failure loop).
   */
  status: 'queued' | 'sending' | 'sending_now' | 'failed';
  retryCount: number;
  /** Agent type / mode in effect when the user enqueued the message. */
  agentType?: string;
  /** Image / attachment payloads forwarded to `start_dialog_turn` when drained. */
  imageContexts?: unknown[];
  imageDisplayData?: unknown[];
  localDialogTurnId?: string;
}

export interface ParsedChunk {
  type: 'text' | 'tool_call' | 'tool_result';
  content: string;
  toolInfo?: {
    tool: string;
    input: any;
    id: string;
  };
  toolResult?: {
    id: string;
    result: any;
    success: boolean;
    error?: string;
  };
}

export interface ToolCardConfig {
  toolName: string;
  displayName: string;
  icon: string;
  requiresConfirmation: boolean;
  resultDisplayType: 'hidden' | 'summary' | 'detailed';
  description?: string;
  displayMode?: 'compact' | 'standard' | 'detailed' | 'terminal';
  primaryColor?: string;
}

export type ToolCardDisplayContext = 'default' | 'subagent-projection';

export interface ToolCardProps {
  toolItem: FlowToolItem;
  config: ToolCardConfig;
  interruptionNote?: string | null;
  onOpenInEditor?: (filePath: string) => void;
  onOpenInPanel?: (panelType: string, data: any) => void;
  onExpand?: () => void;
  sessionId?: string;
  turnId?: string;
  displayContext?: ToolCardDisplayContext;
  /** Callback for MCP App ui/message requests. Returns whether the message was handled successfully. */
  onMcpAppMessage?: (params: import('@/infrastructure/api/service-api/MCPAPI').McpUiMessageParams) => Promise<import('@/infrastructure/api/service-api/MCPAPI').McpUiMessageResult>;
}

// Flow Chat callbacks for layered events.
export interface FlowChatCallbacks {
  onDialogTurnStart?: (dialogTurnId: string, userMessage: string) => void;
  onDialogTurnComplete?: (dialogTurnId: string, totalModelRounds: number) => void;
  onModelRoundStart?: (dialogTurnId: string, modelRoundId: string, roundIndex: number) => void;
  onModelRoundContent?: (
    dialogTurnId: string, 
    modelRoundId: string, 
    contentType: 'text' | 'tool_call' | 'tool_result' | 'thinking',
    content: string,
    metadata?: any
  ) => void;
  onModelRoundEnd?: (dialogTurnId: string, modelRoundId: string, status: string) => void;
  onTaskComplete?: (totalDialogTurns: number, result?: any) => void;
  onTaskError?: (error: string, dialogTurnId?: string, modelRoundId?: string) => void;
}

// Flow Chat actions.
export interface FlowChatActions {
  sendMessage: (message: string, sessionId?: string) => Promise<void>;
  createSession: (config?: Partial<SessionConfig>) => Promise<string>;
  switchSession: (sessionId: string) => void;
  confirmTool: (toolId: string, updatedInput?: any) => void;
  rejectTool: (toolId: string) => void;
  clearSession: (sessionId?: string) => void;
  deleteSession: (sessionId: string) => Promise<void>; // Now async.
  retryLastMessage: () => void;
}

// Flow Chat configuration.
export interface FlowChatConfig {
  enableMarkdown: boolean;
  autoScroll: boolean;
  showTimestamps: boolean;
  maxHistoryRounds: number;
  enableVirtualScroll: boolean;
  theme: 'light' | 'dark' | 'auto';
}
