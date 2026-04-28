/**
 * Shared types for FlowChatManager modules.
 */

import type { FlowChatStore } from '../../store/FlowChatStore';
import type { EventBatcher } from '../EventBatcher';
import type { processingStatusManager } from '../ProcessingStatusManager';
import type { FlowToolEvent } from '../EventBatcher';

/**
 * Shared context for FlowChatManager modules.
 */
export interface FlowChatContext {
  flowChatStore: FlowChatStore;
  processingManager: typeof processingStatusManager;
  eventBatcher: EventBatcher;
  pendingTurnCompletions: Map<string, {
    turnId: string;
    lastActivityAt: number;
    timer: ReturnType<typeof setTimeout> | null;
    /** Set when the turn completed with a partial stream recovery. */
    partialRecoveryReason?: string;
  }>;
  /** In-flight historical session hydration: sessionId -> promise */
  pendingHistoryLoads: Map<string, Promise<void>>;
  /** Content buffers: sessionId -> (roundId -> content) */
  contentBuffers: Map<string, Map<string, string>>;
  /** Active text items: sessionId -> (roundId -> textItemId) */
  activeTextItems: Map<string, Map<string, string>>;
  /** Debounced save timers: key = "sessionId:turnId" */
  saveDebouncers: Map<string, ReturnType<typeof setTimeout>>;
  /** Last save timestamps: key = "sessionId:turnId" */
  lastSaveTimestamps: Map<string, number>;
  /** Last save content hashes: key = "sessionId:turnId" */
  lastSaveHashes: Map<string, string>;
  /** In-flight save tasks: key = "sessionId:turnId" */
  turnSaveInFlight: Map<string, Promise<void>>;
  /** Pending save marks for coalesced serial execution */
  turnSavePending: Set<string>;
  /** Transient runtime status timers: key = "sessionId:turnId:roundId:scope" */
  runtimeStatusTimers: Map<string, ReturnType<typeof setTimeout>>;
  /** Session IDs that the user explicitly cancelled; used to skip unread marking */
  userCancelledSessionIds: Set<string>;
  currentWorkspacePath: string | null;
}

/**
 * Tool event handling options.
 */
export interface ToolEventOptions {
  /** Whether the event is from a subagent. */
  isSubagent?: boolean;
  /** Parent tool ID. */
  parentToolId?: string;
  /** Subagent session ID. */
  subagentSessionId?: string;
  /** Parent tool timestamp. */
  parentTimestamp?: number;
}

export interface SubagentTextChunkData {
  sessionId: string;
  turnId: string;
  roundId: string;
  text: string;
  contentType: string;
  isThinkingEnd?: boolean;
}

export interface SubagentToolEventData {
  sessionId: string;
  turnId: string;
  toolEvent: FlowToolEvent;
}

export type { SessionConfig, DialogTurn, ModelRound, FlowTextItem, FlowToolItem } from '../../types/flow-chat';
