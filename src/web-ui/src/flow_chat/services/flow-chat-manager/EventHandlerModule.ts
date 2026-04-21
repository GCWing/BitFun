/**
 * Event handling module
 * Initializes event listeners and handles various Agentic events
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { agenticEventListener, type AgenticEventCallbacks } from '../AgenticEventListener';
import { 
  generateTextChunkKey, 
  generateToolEventKey,
  parseEventKey,
  type FlowToolEvent,
  type SubagentParentInfo,
  type TextChunkEventData,
  type ToolEventData,
  type ParamsPartialToolEvent
} from '../EventBatcher';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type {
  ImageAnalysisEvent,
  SessionModelAutoMigratedEvent,
} from '@/infrastructure/api/service-api/AgentAPI';
import { i18nService } from '@/infrastructure/i18n';
import { MCPAPI } from '@/infrastructure/api/service-api/MCPAPI';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { FlowChatContext, DialogTurn, ModelRound, FlowToolItem } from './types';

const pendingImageAnalysisTurns = new Map<string, string>();
import { 
  debouncedSaveDialogTurn, 
  immediateSaveDialogTurn, 
  saveDialogTurnToDisk,
  cleanupSaveState,
  updateSessionMetadata,
} from './PersistenceModule';
import { settleInterruptedDialogTurn } from '../../utils/dialogTurnStability';
import { 
  processNormalTextChunkInternal, 
  processThinkingChunkInternal,
  completeActiveTextItems,
  cleanupSessionBuffers
} from './TextChunkModule';
import { 
  processToolEvent, 
  processToolParamsPartialInternal,
  processToolProgressInternal,
  handleToolExecutionProgress,
  handleToolTerminalReady,
} from './ToolEventModule';
import {
  routeTextChunkToToolCardInternal,
  routeToolEventToToolCardInternal
} from './SubagentModule';

const log = createLogger('EventHandlerModule');
const TURN_COMPLETION_QUIET_WINDOW_MS = 500;

interface MCPInteractionRequestEvent {
  interactionId: string;
  serverId: string;
  serverName: string;
  method: string;
  params?: unknown;
}

function isStreamingExecutionState(state: SessionExecutionState): boolean {
  return state === SessionExecutionState.PROCESSING || state === SessionExecutionState.FINISHING;
}

function logDroppedDataEvent(
  eventName: string,
  sessionId: string,
  turnId: string | null,
  details: Record<string, unknown>
): void {
  log.debug('Dropped agentic data event', {
    eventName,
    sessionId,
    turnId,
    ...details,
  });
}

/**
 * Event filtering mechanism: determines if an event should be processed
 */
export function shouldProcessEvent(
  sessionId: string,
  turnId: string | null,
  eventType: 'data' | 'control' | 'state_sync',
  eventName = 'unknown'
): boolean {
  const machine = stateMachineManager.get(sessionId);
  if (!machine) {
    if (eventType === 'data') {
      logDroppedDataEvent(eventName, sessionId, turnId, { reason: 'missing_state_machine' });
    }
    return false;
  }

  const currentState = machine.getCurrentState();
  const context = machine.getContext();

  if (eventType === 'state_sync') {
    return true;
  }

  if (eventType === 'control') {
    if (currentState === SessionExecutionState.IDLE || currentState === SessionExecutionState.ERROR) {
      return true;
    }
    return false;
  }

  if (!isStreamingExecutionState(currentState)) {
    logDroppedDataEvent(eventName, sessionId, turnId, {
      reason: 'state_not_accepting_data',
      currentState,
      currentDialogTurnId: context.currentDialogTurnId,
    });
    return false;
  }

  if (turnId && context.currentDialogTurnId !== turnId) {
    logDroppedDataEvent(eventName, sessionId, turnId, {
      reason: 'turn_id_mismatch',
      sessionId,
      currentState,
      currentDialogTurnId: context.currentDialogTurnId,
    });
    return false;
  }

  return true;
}

/**
 * Map backend state to frontend state
 */
export function mapBackendStateToFrontend(backendState: any): SessionExecutionState {
  if (typeof backendState === 'object' && backendState !== null) {
    if ('Idle' in backendState) {
      return SessionExecutionState.IDLE;
    }
    if ('Processing' in backendState) {
      return SessionExecutionState.PROCESSING;
    }
    if ('Error' in backendState) {
      return SessionExecutionState.ERROR;
    }
  }
  
  if (typeof backendState === 'string') {
    switch (backendState) {
      case 'Idle':
      case 'Completed':
      case 'Cancelled':
        return SessionExecutionState.IDLE;
        
      case 'Processing':
      case 'WaitingForToolResponse':
      case 'Paused':
        return SessionExecutionState.PROCESSING;
        
      case 'Error':
        return SessionExecutionState.ERROR;
        
      default:
        log.warn('Unknown backend state', { backendState });
        return SessionExecutionState.IDLE;
    }
  }
  
  log.warn('Unable to parse backend state', { backendState });
  return SessionExecutionState.IDLE;
}

/**
 * Initialize global event listeners
 * Returns a cleanup function that removes all registered listeners
 */
export async function initializeEventListeners(
  context: FlowChatContext,
  onTodoWriteResult: (sessionId: string, turnId: string, result: any) => void
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  const unlistenProgress = await listen('backend-event-toolexecutionprogress', (event: any) => {
    handleToolExecutionProgress(event.payload);
  });
  const unlistenTerminalReady = await listen('backend-event-toolterminalready', (event: any) => {
    const eventData = (event.payload as any)?.value || event.payload;
    handleToolTerminalReady(eventData);
  });
  const unlistenMcpInteractionRequest = await listen('backend-event-mcpinteractionrequest', (event: any) => {
    void handleMcpInteractionRequest((event.payload as any)?.value || event.payload);
  });

  const callbacks: AgenticEventCallbacks = {
    onSessionCreated: (event) => {
      handleSessionCreated(context, event);
    },
    onSessionDeleted: (event) => {
      handleSessionDeleted(context, event);
    },
    onSessionStateChanged: (event) => {
      handleSessionStateChanged(event);
    },
    onImageAnalysisStarted: (event) => {
      handleImageAnalysisStarted(context, event as ImageAnalysisEvent);
    },
    onImageAnalysisCompleted: (event) => {
      handleImageAnalysisCompleted(context, event as ImageAnalysisEvent);
    },
    onDialogTurnStarted: (event) => {
      handleDialogTurnStarted(context, event);
    },
    onTextChunk: (event) => {
      handleTextChunk(context, event);
    },
    onToolEvent: (event) => {
      handleToolEvent(context, event, onTodoWriteResult);
    },
    onModelRoundStarted: (event) => {
      handleModelRoundStart(context, event);
    },
    onDialogTurnCompleted: (event) => {
      handleDialogTurnComplete(context, event, onTodoWriteResult);
    },
    onDialogTurnFailed: (event) => {
      handleDialogTurnFailed(context, event);
    },
    onDialogTurnCancelled: (event) => {
      handleDialogTurnCancelled(context, event, onTodoWriteResult);
    },
    onTokenUsageUpdated: (event) => {
      handleTokenUsageUpdate(event);
    },
    onContextCompressionStarted: (event) => {
      handleCompressionStarted(context, event);
    },
    onContextCompressionCompleted: (event) => {
      handleCompressionCompleted(context, event);
    },
    onContextCompressionFailed: (event) => {
      handleCompressionFailed(context, event);
    },
    onSessionTitleGenerated: (event) => {
      handleSessionTitleGenerated(event);
    },
    onSessionModelAutoMigrated: (event) => {
      handleSessionModelAutoMigrated(event);
    }
  };

  await agenticEventListener.startListening(callbacks);

  return () => {
    unlistenProgress();
    unlistenTerminalReady();
    unlistenMcpInteractionRequest();
    agenticEventListener.stopListening();
  };
}

async function handleMcpInteractionRequest(rawEvent: unknown): Promise<void> {
  const event = rawEvent as MCPInteractionRequestEvent | undefined;
  const interactionId = event?.interactionId;
  const method = event?.method;

  if (!interactionId || !method) {
    log.warn('Received invalid MCP interaction request event', { rawEvent });
    return;
  }

  const emitted = globalEventBus.emit('mcp:interaction:request', event);
  if (!emitted) {
    log.warn('No MCP interaction UI handler registered, rejecting request', {
      interactionId,
      method,
    });
    try {
      await MCPAPI.submitMCPInteractionResponse({
        interactionId,
        approve: false,
        error: {
          message: 'No MCP interaction UI handler registered',
        },
      });
    } catch (submitError) {
      log.error('Failed to submit MCP interaction auto-rejection', {
        interactionId,
        method,
        submitError,
      });
      notificationService.error(`MCP interaction failed: ${method}`);
    }
  }
}

/**
 * Handle session created event (e.g. remote mobile created a session)
 */
function handleSessionCreated(context: FlowChatContext, event: any): void {
  const { sessionId, sessionName, agentType } = event;

  const store = FlowChatStore.getInstance();
  const existing = store.getState().sessions.get(sessionId);
  if (existing) return;

  store.addExternalSession(
    sessionId,
    sessionName || 'Remote Session',
    agentType || 'agentic',
    resolveExternalSessionWorkspacePath(context, event),
    undefined,
    extractEventRemoteConnectionId(event),
    extractEventRemoteSshHost(event)
  );
}

function resolveExternalSessionWorkspacePath(
  context: FlowChatContext,
  event?: Record<string, unknown> | null,
): string | undefined {
  const candidate =
    (typeof event?.workspacePath === 'string' && event.workspacePath) ||
    (typeof event?.workspace_path === 'string' && event.workspace_path) ||
    context.currentWorkspacePath ||
    undefined;

  return candidate || undefined;
}

function extractEventRemoteConnectionId(event?: Record<string, unknown> | null): string | undefined {
  if (!event) return undefined;
  const id =
    (typeof event.remoteConnectionId === 'string' && event.remoteConnectionId) ||
    (typeof event.remote_connection_id === 'string' && event.remote_connection_id) ||
    undefined;
  return id?.trim() || undefined;
}

function extractEventRemoteSshHost(event?: Record<string, unknown> | null): string | undefined {
  if (!event) return undefined;
  const h =
    (typeof event.remoteSshHost === 'string' && event.remoteSshHost) ||
    (typeof event.remote_ssh_host === 'string' && event.remote_ssh_host) ||
    undefined;
  return h?.trim() || undefined;
}

function clearPendingTurnCompletion(
  context: FlowChatContext,
  sessionId: string,
  turnId?: string
): void {
  const pending = context.pendingTurnCompletions.get(sessionId);
  if (!pending) {
    return;
  }

  if (turnId && pending.turnId !== turnId) {
    return;
  }

  if (pending.timer) {
    clearTimeout(pending.timer);
  }

  context.pendingTurnCompletions.delete(sessionId);
}

function touchPendingTurnCompletion(
  context: FlowChatContext,
  sessionId: string,
  turnId: string
): void {
  const pending = context.pendingTurnCompletions.get(sessionId);
  if (!pending || pending.turnId !== turnId) {
    return;
  }

  pending.lastActivityAt = Date.now();
  schedulePendingTurnCompletion(context, sessionId, turnId);
}

function schedulePendingTurnCompletion(
  context: FlowChatContext,
  sessionId: string,
  turnId: string
): void {
  const pending = context.pendingTurnCompletions.get(sessionId);
  if (!pending || pending.turnId !== turnId) {
    return;
  }

  if (pending.timer) {
    clearTimeout(pending.timer);
  }

  pending.timer = setTimeout(() => {
    finalizePendingTurnCompletion(context, sessionId, turnId);
  }, TURN_COMPLETION_QUIET_WINDOW_MS);
}

function beginTurnCompletion(context: FlowChatContext, sessionId: string, turnId: string): void {
  clearPendingTurnCompletion(context, sessionId);

  context.pendingTurnCompletions.set(sessionId, {
    turnId,
    lastActivityAt: Date.now(),
    timer: null,
  });

  schedulePendingTurnCompletion(context, sessionId, turnId);
}

function flushPendingBatchedEvents(context: FlowChatContext): void {
  if (context.eventBatcher.getBufferSize() > 0) {
    context.eventBatcher.flushNow();
  }
}

function finalizeTurnCompletionState(
  context: FlowChatContext,
  sessionId: string,
  turnId: string
): void {
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);

  if (!session) {
    clearPendingTurnCompletion(context, sessionId, turnId);
    return;
  }

  completeActiveTextItems(context, sessionId, turnId);

  const sessionContentBuffer = context.contentBuffers.get(sessionId);
  if (sessionContentBuffer) {
    sessionContentBuffer.clear();
  }

  context.flowChatStore.markSessionFinished(sessionId);

  context.flowChatStore.updateDialogTurn(sessionId, turnId, turn => {
    const updatedModelRounds = turn.modelRounds.map((round) => {
      if (round.isStreaming) {
        return {
          ...round,
          isStreaming: false,
          isComplete: true,
          status: 'completed' as const,
          endTime: Date.now()
        };
      }
      return round;
    });

    return {
      ...turn,
      modelRounds: updatedModelRounds,
      status: 'completed' as const,
      endTime: Date.now()
    };
  });

  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (isStreamingExecutionState(currentState)) {
    stateMachineManager.transition(sessionId, SessionExecutionEvent.FINISHING_SETTLED);
  } else {
    log.debug('Skipping FINISHING_SETTLED transition', { currentState, sessionId, turnId });
  }

  const dialogTurn = store.getState().sessions.get(sessionId)?.dialogTurns.find(t => t.id === turnId);
  if (dialogTurn) {
    appendPlanDisplayItemsIfNeeded(context, sessionId, turnId, dialogTurn);
  }

  saveDialogTurnToDisk(context, sessionId, turnId).catch(error => {
    log.warn('Failed to save dialog turn (non-critical)', { sessionId, turnId, error });
  });

  clearPendingTurnCompletion(context, sessionId, turnId);
}

function finalizePendingTurnCompletion(
  context: FlowChatContext,
  sessionId: string,
  turnId: string
): void {
  const pending = context.pendingTurnCompletions.get(sessionId);
  if (!pending || pending.turnId !== turnId) {
    return;
  }

  const elapsed = Date.now() - pending.lastActivityAt;
  if (elapsed < TURN_COMPLETION_QUIET_WINDOW_MS) {
    schedulePendingTurnCompletion(context, sessionId, turnId);
    return;
  }

  flushPendingBatchedEvents(context);
  finalizeTurnCompletionState(context, sessionId, turnId);
}

function finalizePendingTurnCompletionNow(context: FlowChatContext, sessionId: string): void {
  const pending = context.pendingTurnCompletions.get(sessionId);
  if (!pending) {
    return;
  }

  if (pending.timer) {
    clearTimeout(pending.timer);
  }

  flushPendingBatchedEvents(context);
  finalizeTurnCompletionState(context, sessionId, pending.turnId);
}

/**
 * Handle session title generated event (AI or fallback auto-generation)
 */
function handleSessionTitleGenerated(event: any): void {
  const { sessionId, title } = event;
  if (!sessionId || !title) return;

  const store = FlowChatStore.getInstance();
  store.updateSessionTitle(sessionId, title, 'generated');
}

function handleSessionModelAutoMigrated(event: SessionModelAutoMigratedEvent): void {
  const { sessionId, previousModelId, newModelId, reason } = event;
  if (!sessionId || !newModelId) return;

  const store = FlowChatStore.getInstance();
  store.updateSessionModelName(sessionId, newModelId);

  const description = i18nService.t('flow-chat:model.autoMigrated.description', {
    previous: previousModelId || 'unknown',
    next: newModelId,
  });
  const reasonText = reason
    ? ' ' + i18nService.t('flow-chat:model.autoMigrated.reason', { reason })
    : '';

  notificationService.warning(description + reasonText, {
    title: i18nService.t('flow-chat:model.autoMigrated.title'),
    duration: 6000,
  });
}

/**
 * Handle session deleted event (backend already deleted; only remove from store)
 */
function handleSessionDeleted(context: FlowChatContext, event: any): void {
  const { sessionId } = event;
  
  const store = FlowChatStore.getInstance();
  const removedSessionIds = store.getCascadeSessionIds(sessionId);
  if (removedSessionIds.length === 0) return;

  log.info('Remote session deleted', { sessionId });
  removedSessionIds.forEach(id => {
    clearPendingTurnCompletion(context, id);
    pendingImageAnalysisTurns.delete(id);
    stateMachineManager.delete(id);
    context.processingManager.clearSessionStatus(id);
    cleanupSaveState(context, id);
    cleanupSessionBuffers(context, id);
  });
  store.removeSession(sessionId);
}

/**
 * Handle backend session state sync event
 */
function handleSessionStateChanged(event: any): void {
  const { sessionId, newState } = event;
  
  const machine = stateMachineManager.get(sessionId);
  if (!machine) {
    log.debug('State sync: state machine not found', { sessionId });
    return;
  }
  
  const frontendState = mapBackendStateToFrontend(newState);
  const currentFrontendState = machine.getCurrentState();
  const isExpectedFinishingDrift =
    currentFrontendState === SessionExecutionState.FINISHING &&
    frontendState === SessionExecutionState.IDLE;
  
  const context = machine.getContext();
  (context as any).backendSyncedAt = Date.now();
  
  if (currentFrontendState !== frontendState && !isExpectedFinishingDrift) {
    log.warn('Frontend and backend state mismatch', {
      sessionId,
      frontend: currentFrontendState,
      backend: frontendState,
      rawBackendState: newState
    });
  }
}

/**
 * Handle image analysis started event (backend vision pre-analysis).
 *
 * Two paths:
 * - Desktop: MessageModule already created the turn locally → just update its status.
 * - Remote (mobile/bot): No turn exists yet → create a temporary turn.
 */
function handleImageAnalysisStarted(context: FlowChatContext, event: ImageAnalysisEvent): void {
  const { sessionId, imageCount, userInput, imageMetadata } = event as any;

  const store = FlowChatStore.getInstance();
  let session = store.getState().sessions.get(sessionId);

  if (!session) {
    store.addExternalSession(
      sessionId,
      'Remote Session',
      'agentic',
      resolveExternalSessionWorkspacePath(context, event as any),
      undefined,
      extractEventRemoteConnectionId(event as any),
      extractEventRemoteSshHost(event as any)
    );
    session = store.getState().sessions.get(sessionId);
  }

  // Desktop path: the turn was created by MessageModule before the backend call.
  if (session) {
    const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
    if (lastTurn && (lastTurn.status === 'pending' || lastTurn.status === 'processing' || lastTurn.status === 'image_analyzing')) {
      store.updateDialogTurn(sessionId, lastTurn.id, turn => ({
        ...turn,
        status: 'image_analyzing' as const,
        userMessage: { ...turn.userMessage, hasImages: true },
      }));
      log.info('Image analysis started: updated existing turn', {
        sessionId,
        turnId: lastTurn.id,
        imageCount,
      });
      return;
    }
  }

  // Extract image display data from metadata (same logic as handleDialogTurnStarted)
  const metaImages = imageMetadata?.images;
  const hasMetaImages = Array.isArray(metaImages) && metaImages.length > 0;
  const images = hasMetaImages
    ? metaImages.map((img: any) => ({
        id: img.id || img.name || `img-${Date.now()}`,
        name: img.name || 'image',
        dataUrl: img.data_url,
        imagePath: img.image_path,
        mimeType: img.mime_type,
      }))
    : undefined;
  const displayInput = imageMetadata?.original_text
    ? cleanRemoteUserInput(imageMetadata.original_text)
    : cleanRemoteUserInput(userInput || '');

  // Remote path: create a temporary turn so the desktop UI shows activity.
  const tempTurnId = `_img_analysis_${sessionId}_${Date.now()}`;

  const tempTurn: DialogTurn = {
    id: tempTurnId,
    sessionId,
    userMessage: {
      id: `user_img_${Date.now()}`,
      content: displayInput,
      timestamp: Date.now(),
      hasImages: true,
      images,
    },
    modelRounds: [],
    status: 'image_analyzing',
    startTime: Date.now(),
  };

  store.addDialogTurn(sessionId, tempTurn);
  pendingImageAnalysisTurns.set(sessionId, tempTurnId);

  context.contentBuffers.set(sessionId, new Map());
  context.activeTextItems.set(sessionId, new Map());

  stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
    taskId: sessionId,
    dialogTurnId: tempTurnId,
  }).catch(error => {
    log.error('State machine transition failed on image analysis start', { sessionId, error });
  });

  log.info('Image analysis started: created temp turn for remote', {
    sessionId,
    tempTurnId,
    imageCount,
  });
}

/**
 * Handle image analysis completed event.
 * Updates the turn status so the UI transitions from "analyzing" to "processing".
 */
function handleImageAnalysisCompleted(_context: FlowChatContext, event: ImageAnalysisEvent): void {
  const { sessionId, success, durationMs } = event;

  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);

  if (session) {
    const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
    if (lastTurn && lastTurn.status === 'image_analyzing') {
      store.updateDialogTurn(sessionId, lastTurn.id, turn => ({
        ...turn,
        status: 'processing' as const,
      }));
    }
  }

  log.info('Image analysis completed', { sessionId, success, durationMs });
}

/**
 * Strip agent-internal XML wrapper tags from user input before displaying.
 * Handles both normal and forwarded-agent envelopes.
 */
function cleanRemoteUserInput(raw: string): string {
  const s = raw.trim();
  const userQueryMatch = s.match(/<user_query>\s*([\s\S]*?)\s*<\/user_query>/);
  if (userQueryMatch) {
    return userQueryMatch[1].trim();
  }

  return s
    .replace(/<system(?:_|-)reminder>[\s\S]*?<\/system(?:_|-)reminder>/g, '')
    .trim();
}

function handleDialogTurnStarted(context: FlowChatContext, event: any): void {
  const { sessionId, turnId, turnIndex, userInput, originalUserInput, userMessageMetadata, subagentParentInfo } = event;

  if (subagentParentInfo) {
    return;
  }

  finalizePendingTurnCompletionNow(context, sessionId);
  clearPendingTurnCompletion(context, sessionId, turnId);

  const store = FlowChatStore.getInstance();

  // Clean up temp image analysis turn if one exists for this session
  const tempTurnId = pendingImageAnalysisTurns.get(sessionId);
  const hadTempTurn = !!tempTurnId;
  if (tempTurnId) {
    store.deleteDialogTurn(sessionId, tempTurnId);
    pendingImageAnalysisTurns.delete(sessionId);

    // State machine was already transitioned to PROCESSING by ImageAnalysisStarted.
    // Update the context's dialogTurnId to the real turn ID.
    const machine = stateMachineManager.get(sessionId);
    if (machine) {
      const ctx = machine.getContext();
      ctx.currentDialogTurnId = turnId;
    }

    log.info('Replaced temp image analysis turn with real turn', {
      sessionId,
      tempTurnId,
      realTurnId: turnId,
    });
  }

  const state = store.getState();
  const session = state.sessions.get(sessionId);

  if (!session) {
    log.warn('DialogTurnStarted: session not in store, creating placeholder', { sessionId, sessionsCount: state.sessions.size });
    store.addExternalSession(
      sessionId,
      'Remote Session',
      'agentic',
      resolveExternalSessionWorkspacePath(context, event),
      undefined,
      extractEventRemoteConnectionId(event),
      extractEventRemoteSshHost(event)
    );
  }

  // Extract image display data from metadata (sent by coordinator for all platforms)
  const metaImages = userMessageMetadata?.images;
  const hasImages = Array.isArray(metaImages) && metaImages.length > 0;
  const images = hasImages
    ? metaImages.map((img: any) => ({
        id: img.id || img.name || `img-${Date.now()}`,
        name: img.name || 'image',
        dataUrl: img.data_url,
        imagePath: img.image_path,
        mimeType: img.mime_type,
      }))
    : undefined;
  const displayContent = originalUserInput
    ? cleanRemoteUserInput(originalUserInput)
    : cleanRemoteUserInput(userInput || '');
  const turnKind =
    userMessageMetadata?.kind === 'manual_compaction' ? 'manual_compaction' : 'user_dialog';
  const triggerSource = userMessageMetadata?.triggerSource as import('@/shared/types/session-history').TriggerSource | undefined;

  const freshSession = store.getState().sessions.get(sessionId);
  const dialogTurn = freshSession?.dialogTurns.find((turn: DialogTurn) => turn.id === turnId);
  if (!dialogTurn) {
    const newTurn: DialogTurn = {
      id: turnId,
      sessionId,
      kind: turnKind,
      userMessage: {
        id: `user_remote_${Date.now()}`,
        content: displayContent,
        timestamp: Date.now(),
        hasImages,
        triggerSource,
        metadata: userMessageMetadata,
        images,
      },
      modelRounds: [],
      status: 'pending',
      startTime: Date.now(),
      backendTurnIndex: typeof turnIndex === 'number' ? turnIndex : undefined,
    };
    store.addDialogTurn(sessionId, newTurn);

    context.contentBuffers.set(sessionId, new Map());
    context.activeTextItems.set(sessionId, new Map());

    if (!hadTempTurn) {
      stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
        taskId: sessionId,
        dialogTurnId: turnId,
      }).catch(error => {
        log.error('State machine transition failed on dialog turn start', { sessionId, error });
      });
    }
    return;
  }

  if (typeof turnIndex === 'number' && dialogTurn.backendTurnIndex === undefined) {
    store.updateDialogTurn(sessionId, turnId, turn => ({
      ...turn,
      kind: turn.kind || turnKind,
      userMessage: {
        ...turn.userMessage,
        triggerSource: turn.userMessage.triggerSource ?? triggerSource,
        metadata: turn.userMessage.metadata || userMessageMetadata,
      },
      backendTurnIndex: turnIndex,
    }));
  }

  // User may have pre-added this turn from the composer while the previous turn was still running;
  // START failed then (PROCESSING/FINISHING cannot take START). When the backend dispatches this
  // turn, align currentDialogTurnId so streaming events are not dropped.
  const machine = stateMachineManager.get(sessionId);
  if (machine) {
    const ctx = machine.getContext();
    if (ctx.currentDialogTurnId !== turnId) {
      ctx.currentDialogTurnId = turnId;
    }
    if (machine.getCurrentState() === SessionExecutionState.IDLE) {
      void stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
        taskId: sessionId,
        dialogTurnId: turnId,
      });
    }
  }
}

/**
 * Handle text chunk event
 */
function handleTextChunk(context: FlowChatContext, event: any): void {
  const { sessionId, turnId, roundId, text, contentType = 'text', isThinkingEnd = false, subagentParentInfo } = event;
  
  const parentSessionId = subagentParentInfo?.sessionId;
  const parentTurnId = subagentParentInfo?.dialogTurnId;
  
  const targetSessionId = parentSessionId || sessionId;
  const targetTurnId = parentTurnId || turnId;
  
  if (!shouldProcessEvent(targetSessionId, targetTurnId, 'data', 'TextChunk')) {
    return;
  }
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(targetSessionId);
  
  if (!session) {
    if (!context.contentBuffers.has(targetSessionId)) {
      log.debug('Session not found (text chunk event)', { sessionId: targetSessionId });
    }
    return;
  }

  const dialogTurn = session.dialogTurns.find((turn: DialogTurn) => turn.id === targetTurnId);
  if (!dialogTurn) {
    log.debug('Dialog turn not found', { turnId: targetTurnId });
    return;
  }

  if (!subagentParentInfo) {
    touchPendingTurnCompletion(context, sessionId, turnId);
    const currentState = stateMachineManager.getCurrentState(sessionId);
    if (isStreamingExecutionState(currentState)) {
      stateMachineManager.transition(sessionId, SessionExecutionEvent.TEXT_CHUNK_RECEIVED, {
        content: text,
      }).catch(error => {
        log.error('State machine transition failed on text chunk', { sessionId, error });
      });
    }
  }

  const eventData: TextChunkEventData = {
    sessionId,
    turnId,
    roundId,
    text,
    contentType: contentType as 'text' | 'thinking',
    isThinkingEnd,
    subagentParentInfo
  };
  
  const key = generateTextChunkKey(eventData);
  
  context.eventBatcher.add(
    key,
    eventData,
    'accumulate',
    (existing, incoming) => ({
      ...existing,
      text: existing.text + incoming.text,
      isThinkingEnd: existing.isThinkingEnd || incoming.isThinkingEnd
    })
  );
}

/**
 * Process batched events
 */
export function processBatchedEvents(
  context: FlowChatContext,
  events: Array<{ key: string; payload: any }>,
  onTodoWriteResult: (sessionId: string, turnId: string, result: any) => void
): void {
  if (events.length === 0) return;
  
  context.flowChatStore.beginSilentMode();
  
  try {
    for (const { key, payload } of events) {
      const parsed = parseEventKey(key);
      if (!parsed) continue;
      
      const { isSubagent, eventType } = parsed;
      
      if (eventType === 'text') {
        if (isSubagent) {
          const { sessionId, turnId, roundId, text, contentType, isThinkingEnd, subagentParentInfo } = payload;
          const parentSessionId = subagentParentInfo?.sessionId;
          const parentToolId = subagentParentInfo?.toolCallId;
          
          if (parentSessionId && parentToolId) {
            routeTextChunkToToolCardInternal(context, parentSessionId, parentToolId, {
              sessionId,
              turnId,
              roundId,
              text,
              contentType,
              isThinkingEnd
            });
          }
        } else {
          const { sessionId, turnId, roundId, text, contentType, isThinkingEnd } = payload;
          if (contentType === 'thinking') {
            processThinkingChunkInternal(context, sessionId, turnId, roundId, text, isThinkingEnd);
          } else {
            processNormalTextChunkInternal(context, sessionId, turnId, roundId, text);
          }
          
          debouncedSaveDialogTurn(context, sessionId, turnId, 2000);
        }
      } else if (eventType === 'tool:params') {
        if (isSubagent) {
          const { subagentParentInfo } = payload;
          const parentSessionId = subagentParentInfo?.sessionId;
          const parentToolId = subagentParentInfo?.toolCallId;
          
          if (parentSessionId && parentToolId) {
            routeToolEventToToolCardInternal(context, parentSessionId, parentToolId, payload, onTodoWriteResult);
          }
        } else {
          const { sessionId, turnId, toolEvent } = payload;
          processToolParamsPartialInternal(sessionId, turnId, toolEvent);
        }
      } else if (eventType === 'tool:progress') {
        if (isSubagent) {
          const { subagentParentInfo } = payload;
          const parentSessionId = subagentParentInfo?.sessionId;
          const parentToolId = subagentParentInfo?.toolCallId;
          
          if (parentSessionId && parentToolId) {
            routeToolEventToToolCardInternal(context, parentSessionId, parentToolId, payload, onTodoWriteResult);
          }
        } else {
          const { sessionId, turnId, toolEvent } = payload;
          processToolProgressInternal(sessionId, turnId, toolEvent);
        }
      }
    }
  } finally {
    context.flowChatStore.endSilentMode();
  }
}

/**
 * Handle tool event
 */
function handleToolEvent(
  context: FlowChatContext,
  event: {
    sessionId: string;
    turnId?: string;
    toolEvent: FlowToolEvent;
    subagentParentInfo?: SubagentParentInfo;
  },
  onTodoWriteResult: (sessionId: string, turnId: string, result: any) => void
): void {
  const { sessionId, turnId, toolEvent, subagentParentInfo } = event;
  if (!turnId) {
    log.debug('Tool event missing turnId', { sessionId, toolId: toolEvent.tool_id, eventType: toolEvent.event_type });
    return;
  }
  
  const parentSessionId = subagentParentInfo?.sessionId;
  const parentToolId = subagentParentInfo?.toolCallId;
  const parentTurnId = subagentParentInfo?.dialogTurnId;
  
  const targetSessionId = parentSessionId || sessionId;
  const targetTurnId = parentTurnId || turnId;
  
  if (!shouldProcessEvent(targetSessionId, targetTurnId, 'data', 'ToolEvent')) {
    return;
  }

  if (!subagentParentInfo) {
    touchPendingTurnCompletion(context, sessionId, turnId);
  }
  
  const eventData: ToolEventData = {
    sessionId,
    turnId,
    toolEvent,
    subagentParentInfo
  };
  
  const keyInfo = generateToolEventKey(eventData);
  
  if (keyInfo) {
    const { key, strategy } = keyInfo;
    
    if (strategy === 'accumulate') {
      context.eventBatcher.add(
        key,
        eventData,
        'accumulate',
        (existing, incoming) => ({
          ...existing,
          toolEvent: {
            ...(existing.toolEvent as ParamsPartialToolEvent),
            params:
              (existing.toolEvent as ParamsPartialToolEvent).params +
              (incoming.toolEvent as ParamsPartialToolEvent).params
          }
        })
      );
    } else {
      context.eventBatcher.add(key, eventData, 'replace');
    }
    return;
  }
  
  if (parentSessionId && parentToolId) {
    import('./SubagentModule').then(({ routeToolEventToToolCard }) => {
      routeToolEventToToolCard(context, parentSessionId, parentToolId, {
        sessionId,
        turnId,
        toolEvent
      }, onTodoWriteResult);
    }).catch(error => {
      log.error('Failed to load SubagentModule or route tool event', { sessionId, turnId, error });
    });
  } else {
    processToolEvent(context, sessionId, turnId, toolEvent, undefined, onTodoWriteResult);
  }
}

/**
 * Handle model round started event
 */
function handleModelRoundStart(context: FlowChatContext, event: any): void {
  const { sessionId, turnId, roundId, roundIndex, subagentParentInfo } = event;

  if (subagentParentInfo) {
    return;
  }
  
  if (!shouldProcessEvent(sessionId, turnId, 'data', 'ModelRoundStarted')) {
    return;
  }
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (model round start)', { sessionId });
    return;
  }

  const dialogTurn = session.dialogTurns.find((turn: DialogTurn) => turn.id === turnId);
  if (!dialogTurn) {
    log.debug('Dialog turn not found (model round start)', { turnId });
    return;
  }

  touchPendingTurnCompletion(context, sessionId, turnId);

  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (isStreamingExecutionState(currentState)) {
    stateMachineManager.transition(sessionId, SessionExecutionEvent.MODEL_ROUND_START, {
      modelRoundId: roundId,
    }).catch(error => {
      log.error('State machine transition failed on model round start', { sessionId, error });
    });
  }

  completeActiveTextItems(context, sessionId, turnId);

  const modelRound: ModelRound = {
    id: roundId,
    index: roundIndex || 0,
    items: [],
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: Date.now()
  };

  context.flowChatStore.addModelRound(sessionId, turnId, modelRound);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle token usage update event
 */
function handleTokenUsageUpdate(event: any): void {
  const { sessionId, inputTokens, outputTokens, totalTokens, maxContextTokens } = event;
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (token usage update)', { sessionId });
    return;
  }

  store.updateTokenUsage(sessionId, {
    inputTokens,
    outputTokens,
    totalTokens
  });

  if (maxContextTokens !== undefined && maxContextTokens !== null) {
    store.updateSessionMaxContextTokens(sessionId, maxContextTokens);
  }
}

/**
 * Handle context compression started event
 */
function handleCompressionStarted(_context: FlowChatContext, event: any): void {
  const { sessionId, turnId, compressionId, trigger, tokensBefore, contextWindow, threshold } = event;
  
  log.info('Context compression started', {
    sessionId, turnId, compressionId, trigger, tokensBefore, contextWindow, threshold
  });
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (compression started)', { sessionId });
    return;
  }
  
  const dialogTurn = session.dialogTurns.find(turn => turn.id === turnId);
  if (!dialogTurn) {
    log.debug('Dialog turn not found (compression started)', { turnId });
    return;
  }

  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (isStreamingExecutionState(currentState)) {
    void stateMachineManager
      .transition(sessionId, SessionExecutionEvent.COMPACTION_STARTED)
      .catch(error => {
        log.error('State machine transition failed on compression start', { sessionId, error });
      });
  }
  
  const compressionItem: FlowToolItem = {
    id: compressionId,
    type: 'tool',
    toolName: 'ContextCompression',
    toolCall: {
      input: {
        trigger,
        tokens_before: tokensBefore,
        context_window: contextWindow,
        threshold,
      },
      id: compressionId
    },
    timestamp: Date.now(),
    status: 'running',
    requiresConfirmation: false,
    startTime: Date.now()
  };
  
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
  
  store.addModelRoundItem(sessionId, turnId, compressionItem, lastModelRound.id);
}

/**
 * Handle context compression completed event
 */
function handleCompressionCompleted(context: FlowChatContext, event: any): void {
  const { 
    sessionId, turnId, compressionId, compressionCount, 
    tokensBefore, tokensAfter, compressionRatio, durationMs, hasSummary, summarySource
  } = event;
  
  log.info('Context compression completed', {
    sessionId, turnId, compressionId, compressionCount, 
    tokensBefore, tokensAfter, compressionRatio, durationMs
  });
  
  const store = FlowChatStore.getInstance();
  
  store.updateModelRoundItem(sessionId, turnId, compressionId, {
    toolResult: {
      result: {
        compression_count: compressionCount,
        tokens_before: tokensBefore,
        tokens_after: tokensAfter,
        compression_ratio: compressionRatio,
        duration: durationMs,
        has_summary: hasSummary,
        summary_source: summarySource,
      },
      success: true,
      duration_ms: durationMs || 0
    },
    status: 'completed',
    endTime: Date.now()
  } as any);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle context compression failed event
 */
function handleCompressionFailed(context: FlowChatContext, event: any): void {
  const { sessionId, turnId, compressionId, error } = event;
  
  log.error('Context compression failed', { sessionId, turnId, compressionId, error });
  
  const store = FlowChatStore.getInstance();
  
  store.updateModelRoundItem(sessionId, turnId, compressionId, {
    toolResult: {
      result: null,
      success: false,
      error,
      duration_ms: 0
    },
    status: 'error',
    endTime: Date.now()
  } as any);
  
  immediateSaveDialogTurn(context, sessionId, turnId);
}

/**
 * Handle dialog turn completed event
 */
function handleDialogTurnComplete(
  context: FlowChatContext,
  event: any,
  _onTodoWriteResult: (sessionId: string, turnId: string, result: any) => void
): void {
  const sessionId = event?.sessionId ?? event?.session_id;
  const turnId = event?.turnId ?? event?.turn_id;
  const subagentParentInfo = event?.subagentParentInfo ?? event?.subagent_parent_info;

  if (subagentParentInfo) {
    return;
  }

  if (!sessionId || !turnId) {
    log.warn('DialogTurnCompleted missing sessionId or turnId', { event });
    return;
  }

  const machine = stateMachineManager.get(sessionId);
  if (machine) {
    const ctx = machine.getContext();
    if (ctx.currentDialogTurnId !== turnId) {
      ctx.currentDialogTurnId = turnId;
    }
  }

  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (dialog turn complete)', { sessionId });
    return;
  }

  context.flowChatStore.updateDialogTurn(sessionId, turnId, turn => {
    return {
      ...turn,
      status: 'finishing' as const,
    };
  });

  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (currentState === SessionExecutionState.PROCESSING) {
    void stateMachineManager
      .transition(sessionId, SessionExecutionEvent.BACKEND_STREAM_COMPLETED)
      .catch(error => {
        log.error('State machine transition failed on backend stream completed', { sessionId, error });
      });
  } else {
    log.debug('Skipping BACKEND_STREAM_COMPLETED transition', { currentState, sessionId, turnId });
  }

  beginTurnCompletion(context, sessionId, turnId);
}

/**
 * Handle dialog turn failed event
 */
function handleDialogTurnFailed(context: FlowChatContext, event: any): void {
  const { sessionId, turnId, error, subagentParentInfo } = event;

  if (subagentParentInfo) {
    return;
  }
  
  log.error('Dialog turn failed', { sessionId, turnId, error });
  clearPendingTurnCompletion(context, sessionId, turnId);
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (dialog turn failed)', { sessionId });
    return;
  }
  
  const sessionActiveTextItems = context.activeTextItems.get(sessionId);
  if (sessionActiveTextItems) {
    sessionActiveTextItems.clear();
  }
  
  const sessionContentBuffer = context.contentBuffers.get(sessionId);
  if (sessionContentBuffer) {
    sessionContentBuffer.clear();
  }

  context.flowChatStore.markSessionFinished(sessionId);
  
  const dialogTurn = session.dialogTurns.find(turn => turn.id === turnId);
  const hasSuccessfulModelRounds = dialogTurn && dialogTurn.modelRounds.length > 0;
  
  if (hasSuccessfulModelRounds) {
    const settledAt = Date.now();
    context.flowChatStore.updateDialogTurn(sessionId, turnId, turn =>
      settleInterruptedDialogTurn(
        {
          ...turn,
          status: 'error',
          error: error || 'Execution failed',
          endTime: turn.endTime ?? settledAt,
        },
        settledAt
      )
    );
    
    saveDialogTurnToDisk(context, sessionId, turnId).catch(err => {
      log.warn('Failed to save failed dialog turn', { sessionId, turnId, error: err });
    });
  } else {
    if (dialogTurn?.userMessage?.content) {
      const machine = stateMachineManager.get(sessionId);
      if (machine) {
        machine.setQueuedInput(dialogTurn.userMessage.content);
      }
    }
    
    context.flowChatStore.deleteDialogTurn(sessionId, turnId);
    updateSessionMetadata(context, sessionId).catch(err => {
      log.warn('Failed to update failed session metadata', { sessionId, error: err });
    });
  }
  
  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (isStreamingExecutionState(currentState)) {
    stateMachineManager.transition(sessionId, SessionExecutionEvent.ERROR_OCCURRED, {
      error: error || 'Execution failed'
    }).catch(err => {
      log.error('State machine transition failed on error occurred', { sessionId, error: err });
    });
    stateMachineManager.transition(sessionId, SessionExecutionEvent.RESET).catch(err => {
      log.error('State machine transition failed on reset', { sessionId, error: err });
    });
  }
  
  notificationService.error(error || 'Execution failed', {
    title: 'Dialog execution failed',
    duration: 5000
  });
}

/**
 * Handle dialog turn cancelled event
 */
function handleDialogTurnCancelled(
  context: FlowChatContext,
  event: any,
  _onTodoWriteResult: (sessionId: string, turnId: string, result: any) => void
): void {
  const { sessionId, turnId, subagentParentInfo } = event;

  if (subagentParentInfo) {
    return;
  }
  
  log.info('Dialog turn cancelled', { sessionId, turnId });
  clearPendingTurnCompletion(context, sessionId, turnId);
  
  const store = FlowChatStore.getInstance();
  const session = store.getState().sessions.get(sessionId);
  
  if (!session) {
    log.debug('Session not found (dialog turn cancelled)', { sessionId });
    return;
  }
  
  const sessionActiveTextItems = context.activeTextItems.get(sessionId);
  if (sessionActiveTextItems) {
    sessionActiveTextItems.clear();
  }
  
  const sessionContentBuffer = context.contentBuffers.get(sessionId);
  if (sessionContentBuffer) {
    sessionContentBuffer.clear();
  }

  context.flowChatStore.markSessionFinished(sessionId);
  
  const settledAt = Date.now();
  context.flowChatStore.updateDialogTurn(sessionId, turnId, turn =>
    settleInterruptedDialogTurn(
      {
        ...turn,
        status: 'cancelled',
        endTime: turn.endTime ?? settledAt,
      },
      settledAt
    )
  );
  
  const dialogTurn = session.dialogTurns.find(t => t.id === turnId);
  if (dialogTurn) {
    appendPlanDisplayItemsIfNeeded(context, sessionId, turnId, dialogTurn);
  }
  
  saveDialogTurnToDisk(context, sessionId, turnId).catch(err => {
    log.warn('Failed to save cancelled dialog turn', { sessionId, turnId, error: err });
  });

  // Transition state machine to IDLE.  When the desktop's own stop button
  // is used, USER_CANCEL is dispatched before DialogTurnCancelled arrives,
  // so the machine is already IDLE.  When cancellation comes from an
  // external source (mobile remote), the machine is still PROCESSING.
  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (isStreamingExecutionState(currentState)) {
    void stateMachineManager
      .transition(sessionId, SessionExecutionEvent.FINISHING_SETTLED)
      .catch(error => {
        log.error('State machine transition failed on cancelled finishing settled', { sessionId, error });
      });
  }
}

/**
 * Detect .plan.md files modified by Edit/Write in dialog turn
 */
function detectModifiedPlanFiles(dialogTurn: DialogTurn): string[] {
  const planFiles: string[] = [];
  const createPlanFiles = new Set<string>();
  
  for (const round of dialogTurn.modelRounds) {
    for (const item of round.items) {
      if (item.type !== 'tool') continue;
      const toolItem = item as FlowToolItem;
      
      if (toolItem.toolName === 'CreatePlan' && toolItem.toolResult?.success) {
        const planPath = toolItem.toolResult.result?.plan_file_path;
        if (planPath) createPlanFiles.add(planPath);
      }
      
      if (['Edit', 'Write'].includes(toolItem.toolName) && toolItem.toolResult?.success) {
        const input = toolItem.toolCall?.input;
        const filePath = input?.file_path || input?.target_file || '';
        if (filePath.endsWith('.plan.md')) {
          planFiles.push(filePath);
        }
      }
    }
  }
  
  return [...new Set(planFiles)].filter(f => !createPlanFiles.has(f));
}

/**
 * Append PlanDisplay tool items if plan files were modified
 */
function appendPlanDisplayItemsIfNeeded(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  dialogTurn: DialogTurn
): void {
  const modifiedPlanFiles = detectModifiedPlanFiles(dialogTurn);
  if (modifiedPlanFiles.length === 0) return;
  
  const lastRound = dialogTurn.modelRounds[dialogTurn.modelRounds.length - 1];
  if (!lastRound) return;
  
  for (const planFilePath of modifiedPlanFiles) {
    const planToolItem: FlowToolItem = {
      id: `plan-display-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      type: 'tool',
      toolName: 'CreatePlan',
      toolCall: { input: {}, id: '' },
      toolResult: {
        result: { plan_file_path: planFilePath },
        success: true
      },
      timestamp: Date.now(),
      status: 'completed'
    };
    
    context.flowChatStore.addModelRoundItem(sessionId, turnId, planToolItem, lastRound.id);
  }
}
