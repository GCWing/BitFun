/**
 * Session persistence types.
 *
 * Used by session lists and persistence metadata in the frontend.
 */

export type SessionKind = 'normal' | 'btw' | 'review' | 'deep_review';
export type PersistedSessionKind = 'standard' | 'subagent';
export type SessionTitleSource = 'text' | 'i18n';

export interface SessionCustomMetadata extends Record<string, unknown> {
  kind?: SessionKind;
  parentSessionId?: string | null;
  parentRequestId?: string | null;
  parentDialogTurnId?: string | null;
  parentTurnIndex?: number | null;
  lastFinishedAt?: number | null;
  titleSource?: SessionTitleSource | null;
  titleKey?: string | null;
  titleParams?: Record<string, unknown> | null;
}

export interface SessionMetadata {
  sessionId: string;
  sessionName: string;
  agentType: string;
  sessionKind?: PersistedSessionKind;
  modelName: string;
  createdAt: number;
  lastActiveAt: number;
  turnCount: number;
  messageCount: number;
  toolCallCount: number;
  status: SessionStatus;
  snapshotSessionId?: string;
  tags: string[];
  customMetadata?: SessionCustomMetadata;
  todos?: any[];
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  /** Backend unified workspace identity field: localhost for local, SSH host for remote. */
  workspaceHostname?: string;
}

export type SessionStatus = 'active' | 'archived' | 'completed';
export type DialogTurnKind = 'user_dialog' | 'manual_compaction';

export interface SessionList {
  sessions: SessionMetadata[];
  lastUpdated: number;
  version: string;
}

export interface DialogTurnData {
  turnId: string;
  turnIndex: number;
  sessionId: string;
  timestamp: number;
  kind?: DialogTurnKind;
  userMessage: UserMessageData;
  modelRounds: ModelRoundData[];
  startTime: number;
  endTime?: number;
  durationMs?: number;
  status: TurnStatus;
}

export interface UserMessageData {
  id: string;
  content: string;
  timestamp: number;
  metadata?: Record<string, any>;
}

export interface ModelRoundData {
  id: string;
  turnId: string;
  roundIndex: number;
  timestamp: number;
  textItems: TextItemData[];
  toolItems: ToolItemData[];
  thinkingItems?: ThinkingItemData[];
  startTime: number;
  endTime?: number;
  status: string;
}

export interface TextItemData {
  id: string;
  content: string;
  isStreaming: boolean;
  timestamp: number;
  status?: string;
  orderIndex?: number;
  isMarkdown?: boolean;
  isSubagentItem?: boolean;
  parentTaskToolId?: string;
  subagentSessionId?: string;
}

export interface ThinkingItemData {
  id: string;
  content: string;
  isStreaming: boolean;
  isCollapsed: boolean;
  timestamp: number;
  orderIndex?: number;
  status?: string;
  isSubagentItem?: boolean;
  parentTaskToolId?: string;
  subagentSessionId?: string;
}

export interface ToolItemData {
  id: string;
  toolName: string;
  toolCall: ToolCallData;
  toolResult?: ToolResultData;
  aiIntent?: string;
  startTime: number;
  endTime?: number;
  durationMs?: number;
  orderIndex?: number;
  status?: string;
  interruptionReason?: 'app_restart';
  isSubagentItem?: boolean;
  parentTaskToolId?: string;
  subagentSessionId?: string;
}

export interface ToolCallData {
  input: any;
  id: string;
}

export interface ToolResultData {
  result: any;
  success: boolean;
  resultForAssistant?: string;
  error?: string;
  durationMs?: number;
}

export type TurnStatus = 'inprogress' | 'completed' | 'error' | 'cancelled';
