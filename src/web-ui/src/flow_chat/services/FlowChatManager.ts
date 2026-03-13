/**
 * Flow Chat unified manager
 * Integrates Agent management and Flow Chat UI state management
 * 
 * Refactoring note:
 * This file is the main entry point, responsible for singleton management, initialization, and module coordination
 * Specific functionality is split into modules under flow-chat-manager/
 */

import { processingStatusManager } from './ProcessingStatusManager';
import { FlowChatStore } from '../store/FlowChatStore';
import { AgentService } from '../../shared/services/agent-service';
import { stateMachineManager } from '../state-machine';
import { EventBatcher } from './EventBatcher';
import { createLogger } from '@/shared/utils/logger';

import type { FlowChatContext, SessionConfig, DialogTurn } from './flow-chat-manager/types';
import type { FlowToolItem, FlowTextItem, ModelRound } from '../types/flow-chat';
import {
  saveAllInProgressTurns,
  immediateSaveDialogTurn,
  createChatSession as createChatSessionModule,
  switchChatSession as switchChatSessionModule,
  deleteChatSession as deleteChatSessionModule,
  cleanupSaveState,
  cleanupSessionBuffers,
  sendMessage as sendMessageModule,
  cancelCurrentTask as cancelCurrentTaskModule,
  initializeEventListeners,
  processBatchedEvents,
  addDialogTurn as addDialogTurnModule,
  addImageAnalysisPhase as addImageAnalysisPhaseModule,
  updateImageAnalysisResults as updateImageAnalysisResultsModule,
  updateImageAnalysisItem as updateImageAnalysisItemModule
} from './flow-chat-manager';

const log = createLogger('FlowChatManager');

export class FlowChatManager {
  private static instance: FlowChatManager;
  private context: FlowChatContext;
  private agentService: AgentService;
  private eventListenerInitialized = false;

  private constructor() {
    this.context = {
      flowChatStore: FlowChatStore.getInstance(),
      processingManager: processingStatusManager,
      eventBatcher: new EventBatcher({
        onFlush: (events) => this.processBatchedEvents(events),
        debug: false
      }),
      contentBuffers: new Map(),
      activeTextItems: new Map(),
      saveDebouncers: new Map(),
      lastSaveTimestamps: new Map(),
      lastSaveHashes: new Map(),
      turnSaveInFlight: new Map(),
      turnSavePending: new Set(),
      currentWorkspacePath: null
    };
    
    this.agentService = AgentService.getInstance();
  }

  public static getInstance(): FlowChatManager {
    if (!FlowChatManager.instance) {
      FlowChatManager.instance = new FlowChatManager();
    }
    return FlowChatManager.instance;
  }

  async initialize(workspacePath: string, preferredMode?: string): Promise<boolean> {
    try {
      await this.initializeEventListeners();
      await this.context.flowChatStore.initializeFromDisk(workspacePath);

      const state = this.context.flowChatStore.getState();
      const workspaceSessions = Array.from(state.sessions.values())
        .filter(session => (session.workspacePath || workspacePath) === workspacePath);
      const hasHistoricalSessions = workspaceSessions.length > 0;
      const activeSession = state.activeSessionId
        ? state.sessions.get(state.activeSessionId) ?? null
        : null;
      const activeSessionBelongsToWorkspace = !!activeSession &&
        (activeSession.workspacePath || workspacePath) === workspacePath;

      if (hasHistoricalSessions && !activeSessionBelongsToWorkspace) {
        const sortedWorkspaceSessions = [...workspaceSessions].sort((a, b) => b.lastActiveAt - a.lastActiveAt);
        const latestSession = (preferredMode
          ? sortedWorkspaceSessions.find(session => session.mode === preferredMode)
          : undefined) || sortedWorkspaceSessions[0];

        if (!latestSession) {
          this.context.currentWorkspacePath = workspacePath;
          return hasHistoricalSessions;
        }

        // If no session matches preferred mode, keep activeSessionId unset for caller to create one.
        if (preferredMode && latestSession.mode !== preferredMode) {
          this.context.currentWorkspacePath = workspacePath;
          return hasHistoricalSessions;
        }

        if (latestSession.isHistorical) {
          await this.context.flowChatStore.loadSessionHistory(latestSession.sessionId, workspacePath);
        }

        this.context.flowChatStore.switchSession(latestSession.sessionId);
      }

      this.context.currentWorkspacePath = workspacePath;

      return hasHistoricalSessions;
    } catch (error) {
      log.error('Initialization failed', error);
      return false;
    }
  }

  private async initializeEventListeners(): Promise<void> {
    if (this.eventListenerInitialized) {
      return;
    }

    await initializeEventListeners(
      this.context,
      (sessionId, turnId, result) => this.handleTodoWriteResult(sessionId, turnId, result)
    );
    
    this.eventListenerInitialized = true;
  }

  private processBatchedEvents(events: Array<{ key: string; payload: any }>): void {
    processBatchedEvents(
      this.context,
      events,
      (sessionId, turnId, result) => this.handleTodoWriteResult(sessionId, turnId, result)
    );
  }

  async createChatSession(config: SessionConfig, mode?: string): Promise<string> {
    return createChatSessionModule(this.context, config, mode);
  }

  async switchChatSession(sessionId: string): Promise<void> {
    return switchChatSessionModule(this.context, sessionId);
  }

  async deleteChatSession(sessionId: string): Promise<void> {
    return deleteChatSessionModule(this.context, sessionId);
  }

  async resetWorkspaceSessions(
    workspacePath: string,
    options?: { reinitialize?: boolean; preferredMode?: string }
  ): Promise<void> {
    const removedSessionIds = this.context.flowChatStore.removeSessionsByWorkspace(workspacePath);

    removedSessionIds.forEach(sessionId => {
      stateMachineManager.delete(sessionId);
      this.context.processingManager.clearSessionStatus(sessionId);
      cleanupSaveState(this.context, sessionId);
      cleanupSessionBuffers(this.context, sessionId);
    });

    if (!options?.reinitialize) {
      return;
    }

    const hasHistoricalSessions = await this.initialize(workspacePath, options.preferredMode);
    const state = this.context.flowChatStore.getState();
    const activeSession = state.activeSessionId
      ? state.sessions.get(state.activeSessionId) ?? null
      : null;
    const hasActiveWorkspaceSession =
      !!activeSession &&
      (activeSession.workspacePath || workspacePath) === workspacePath;

    if (!hasHistoricalSessions || !hasActiveWorkspaceSession) {
      await this.createChatSession({}, options.preferredMode);
    }
  }

  async sendMessage(
    message: string,
    sessionId?: string,
    displayMessage?: string,
    agentType?: string,
    switchToMode?: string,
    options?: {
      imageContexts?: import('@/infrastructure/api/service-api/ImageAnalysisAPI').ImageContextData[];
      imageDisplayData?: Array<{ id: string; name: string; dataUrl?: string; imagePath?: string; mimeType?: string }>;
    }
  ): Promise<void> {
    const targetSessionId = sessionId || this.context.flowChatStore.getState().activeSessionId;
    
    if (!targetSessionId) {
      throw new Error('No active session');
    }

    return sendMessageModule(
      this.context,
      message,
      targetSessionId,
      displayMessage,
      agentType,
      switchToMode,
      options
    );
  }

  async cancelCurrentTask(): Promise<boolean> {
    return cancelCurrentTaskModule(this.context);
  }

  public async saveAllInProgressTurns(): Promise<void> {
    return saveAllInProgressTurns(this.context);
  }

  /**
   * Save a specific dialog turn to disk.
   * Used when tool call data is updated after the turn has completed (e.g. mermaid code fix).
   */
  public async saveDialogTurn(sessionId: string, turnId: string): Promise<void> {
    return immediateSaveDialogTurn(this.context, sessionId, turnId, true);
  }

  addDialogTurn(sessionId: string, dialogTurn: DialogTurn): void {
    addDialogTurnModule(this.context, sessionId, dialogTurn);
  }

  /**
   * Insert an in-stream /btw marker into the currently streaming turn, and split the streaming text item
   * so subsequent chunks continue after the marker.
   *
   * This is best-effort; if we cannot locate an active streaming turn/round, it becomes a no-op.
   */
  public insertBtwMarkerIntoActiveStream(params: {
    parentSessionId: string;
    requestId: string;
    childSessionId: string;
    title: string;
  }): void {
    const { parentSessionId, requestId, childSessionId, title } = params;

    const machine = stateMachineManager.get(parentSessionId);
    const ctx = machine?.getContext?.();
    const dialogTurnId = ctx?.currentDialogTurnId;
    if (!dialogTurnId) return;

    const session = this.context.flowChatStore.getState().sessions.get(parentSessionId);
    const turn = session?.dialogTurns.find(t => t.id === dialogTurnId);
    if (!turn) return;
    if (turn.status !== 'processing' && turn.status !== 'image_analyzing') {
      // Only inject into an actively streaming turn; otherwise we'd create dangling streaming items.
      return;
    }

    const lastRound: ModelRound | undefined = (() => {
      const streaming = [...turn.modelRounds].reverse().find(r => r.isStreaming);
      if (streaming) return streaming;
      return turn.modelRounds[turn.modelRounds.length - 1];
    })();
    if (!lastRound) return;

    const roundId = lastRound.id;

    if (!this.context.contentBuffers.has(parentSessionId)) {
      this.context.contentBuffers.set(parentSessionId, new Map());
    }
    if (!this.context.activeTextItems.has(parentSessionId)) {
      this.context.activeTextItems.set(parentSessionId, new Map());
    }
    const sessionBuffers = this.context.contentBuffers.get(parentSessionId)!;
    const sessionActiveItems = this.context.activeTextItems.get(parentSessionId)!;

    const existingTextItemId = sessionActiveItems.get(roundId);
    if (existingTextItemId) {
      // Freeze the existing streaming text item as "pre-marker".
      this.context.flowChatStore.updateModelRoundItem(parentSessionId, dialogTurnId, existingTextItemId, {
        isStreaming: false,
        status: 'completed',
      } as any);
    }

    // Reset buffer so the new tail text item starts fresh (no duplication).
    sessionBuffers.set(roundId, '');

    const markerId = `btw_marker_${requestId}`;
    const markerItem: FlowToolItem = {
      id: markerId,
      type: 'tool',
      timestamp: Date.now(),
      status: 'completed',
      toolName: 'BtwMarker',
      toolCall: {
        id: markerId,
        input: {
          requestId,
          childSessionId,
          title,
        },
      },
      requiresConfirmation: false,
    };
    this.context.flowChatStore.addModelRoundItem(parentSessionId, dialogTurnId, markerItem as any, roundId);

    const tailTextItemId = `btw_tail_${requestId}`;
    const tailTextItem: FlowTextItem = {
      id: tailTextItemId,
      type: 'text',
      content: '',
      isStreaming: true,
      isMarkdown: true,
      timestamp: Date.now(),
      status: 'streaming',
    };
    this.context.flowChatStore.addModelRoundItem(parentSessionId, dialogTurnId, tailTextItem as any, roundId);

    sessionActiveItems.set(roundId, tailTextItemId);
  }

  addImageAnalysisPhase(
    sessionId: string,
    dialogTurnId: string,
    imageContexts: import('@/shared/types/context').ImageContext[]
  ): void {
    addImageAnalysisPhaseModule(this.context, sessionId, dialogTurnId, imageContexts);
  }

  updateImageAnalysisResults(
    sessionId: string,
    dialogTurnId: string,
    results: import('../types/flow-chat').ImageAnalysisResult[]
  ): void {
    updateImageAnalysisResultsModule(this.context, sessionId, dialogTurnId, results);
  }

  updateImageAnalysisItem(
    sessionId: string,
    dialogTurnId: string,
    imageId: string,
    updates: { status?: 'analyzing' | 'completed' | 'error'; error?: string; result?: any }
  ): void {
    updateImageAnalysisItemModule(this.context, sessionId, dialogTurnId, imageId, updates);
  }

  async getAvailableAgents(): Promise<string[]> {
    return this.agentService.getAvailableAgents();
  }

  getCurrentSession() {
    return this.context.flowChatStore.getActiveSession();
  }

  getFlowChatState() {
    return this.context.flowChatStore.getState();
  }

  getAllProcessingStatuses() {
    return this.context.processingManager.getAllStatuses();
  }

  onFlowChatStateChange(callback: (state: any) => void) {
    return this.context.flowChatStore.subscribe(callback);
  }

  onProcessingStatusChange(callback: (statuses: any[]) => void) {
    return this.context.processingManager.addListener(callback);
  }

  getSessionIdByTaskId(taskId: string): string | undefined {
    return taskId;
  }

  private handleTodoWriteResult(sessionId: string, turnId: string, result: any): void {
    try {
      if (!result.todos || !Array.isArray(result.todos)) {
        log.debug('TodoWrite result missing todos array', { sessionId, turnId });
        return;
      }

      const incomingTodos: import('../types/flow-chat').TodoItem[] = result.todos.map((todo: any) => ({
        id: todo.id,
        content: todo.content,
        status: todo.status,
      }));

      if (result.merge) {
        const existingTodos = this.context.flowChatStore.getDialogTurnTodos(sessionId, turnId);
        const todoMap = new Map<string, import('../types/flow-chat').TodoItem>();
        
        existingTodos.forEach(todo => {
          todoMap.set(todo.id, todo);
        });
        
        incomingTodos.forEach(todo => {
          todoMap.set(todo.id, todo);
        });
        
        const mergedTodos = Array.from(todoMap.values());
        this.context.flowChatStore.setDialogTurnTodos(sessionId, turnId, mergedTodos);
      } else {
        this.context.flowChatStore.setDialogTurnTodos(sessionId, turnId, incomingTodos);
      }
      
      this.syncTodosToStateMachine(sessionId);
      
      window.dispatchEvent(new CustomEvent('bitfun:todowrite-update', {
        detail: {
          sessionId,
          turnId,
          todos: incomingTodos,
          merge: result.merge
        }
      }));
    } catch (error) {
      log.error('Failed to handle TodoWrite result', { sessionId, turnId, error });
    }
  }

  private syncTodosToStateMachine(sessionId: string): void {
    const machine = stateMachineManager.get(sessionId);
    if (!machine) return;
    
    const todos = this.context.flowChatStore.getTodos(sessionId);
    const context = machine.getContext();
    
    const plannerTodos = todos.map(todo => ({
      id: todo.id,
      content: todo.content,
      status: todo.status,
    }));
    
    if (context) {
      context.planner = {
        todos: plannerTodos,
        isActive: todos.length > 0
      };
    }
  }
}
export const flowChatManager = FlowChatManager.getInstance();
export default flowChatManager;
