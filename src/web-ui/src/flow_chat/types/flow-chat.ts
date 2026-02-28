/**
 * Flow Chat type definitions
 * Supports mixed streaming output.
 */

// Base type for streaming items.
export interface FlowItem {
  id: string;
  type: 'text' | 'tool' | 'image-analysis' | 'thinking';
  timestamp: number;
  status: 'pending' | 'preparing' | 'running' | 'streaming' | 'completed' | 'cancelled' | 'error' | 'analyzing' | 'pending_confirmation' | 'confirmed'; // Includes error, analyzing, and confirmation states.
  
  // Subagent markers.
  parentTaskToolId?: string; // Parent Task tool ID.
  isSubagentItem?: boolean; // Whether this item is from a subagent.
  subagentSessionId?: string; // Subagent session ID (debug only).
}

export interface FlowTextItem extends FlowItem {
  type: 'text';
  content: string;
  isStreaming: boolean;
  isMarkdown?: boolean;
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
  toolCall: {
    input: any;
    id: string;
  };
  toolResult?: {
    result: any;
    success: boolean;
    error?: string;
    duration_ms?: number;
  };
  requiresConfirmation?: boolean;
  userConfirmed?: boolean;
  aiIntent?: string; // AI rationale for calling the tool.
  startTime?: number;  // Tool start time.
  endTime?: number;    // Tool end time.
  
  // Streaming parameter buffering.
  isParamsStreaming?: boolean;  // Params are streaming in.
  partialParams?: Record<string, any>;  // Partial params during streaming.
  _paramsBuffer?: string;  // Internal buffer for accumulated params.
}

export interface FlowImageAnalysisItem extends FlowItem {
  type: 'image-analysis';
  imageContext: import('@/shared/types/context').ImageContext;
  result?: ImageAnalysisResult | null;
  error?: string;
}

export interface ImageAnalysisResult {
  image_id: string;
  summary: string;              // Short summary.
  detailed_description: string; // Detailed description.
  detected_elements: string[];  // Key detected elements.
  confidence: number;           // Confidence score (0-1).
  analysis_time_ms: number;     // Analysis duration.
}

// Model round: output from a single model call.
export interface ModelRound {
  id: string;
  index: number;
  items: (FlowTextItem | FlowToolItem | FlowImageAnalysisItem | FlowThinkingItem)[];
  isStreaming: boolean;
  isComplete: boolean;
  status: 'pending' | 'streaming' | 'completed' | 'cancelled' | 'error' | 'pending_confirmation';
  startTime: number;
  endTime?: number;
  error?: string;
}

// Token usage stats.
export interface TokenUsage {
  inputTokens: number;
  outputTokens?: number;
  totalTokens: number;
  timestamp: number;
}

// Dialog turn: user input + full AI response across model rounds.
export interface DialogTurn {
  id: string;
  sessionId: string; // Used for event filtering.
  userMessage: {
    id: string;
    content: string;
    timestamp: number;
    hasImages?: boolean;
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
  status: 'pending' | 'image_analyzing' | 'processing' | 'completed' | 'cancelling' | 'cancelled' | 'error'; // Includes image_analyzing.
  startTime: number;
  endTime?: number;
  error?: string;
  tokenUsage?: TokenUsage;
  todos?: TodoItem[];
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

// Session state.
export interface Session {
  sessionId: string;
  title?: string;
  titleStatus?: 'generating' | 'generated' | 'failed';
  dialogTurns: DialogTurn[];
  
  // Derived status from deriveSessionStatus():
  // - 'active': sessionId === activeSessionId
  // - 'error': state machine state === ERROR
  // - 'idle': otherwise
  status: 'active' | 'idle' | 'error';
  
  config: SessionConfig;
  createdAt: number;
  lastActiveAt: number;
  
  // Persist the last error; real-time errors come from context.errorMessage.
  error: string | null;
  
  // Historical sessions are persisted and require lazy loading.
  isHistorical?: boolean;
  
  todos?: TodoItem[];
  
  currentTokenUsage?: TokenUsage;
  maxContextTokens?: number;
  
  // Session mode is synced to the input when switching sessions.
  mode?: string;
}

export interface SessionConfig {
  modelName?: string;
  context?: Record<string, string>;
}

export interface QueuedMessage {
  id: string;
  sessionId: string;
  content: string;
  timestamp: number;
  status: 'queued' | 'processing' | 'completed' | 'failed';
  retryCount: number;
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

export interface ToolCardProps {
  toolItem: FlowToolItem;
  config: ToolCardConfig;
  onConfirm?: (updatedInput?: any) => void;  // toolId is known within the card.
  onReject?: () => void;
  onOpenInEditor?: (filePath: string) => void;
  onOpenInPanel?: (panelType: string, data: any) => void;
  onExpand?: () => void;
  sessionId?: string;
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


