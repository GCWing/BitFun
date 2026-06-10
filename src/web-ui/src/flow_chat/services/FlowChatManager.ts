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
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { stateMachineManager } from '../state-machine';
import { EventBatcher } from './EventBatcher';
import { createLogger } from '@/shared/utils/logger';
import type { WorkspaceInfo } from '@/shared/types';
import {
  compareSessionsForDisplay,
  sessionBelongsToWorkspaceNavRow,
} from '../utils/sessionOrdering';

import type { FlowChatContext, SessionConfig, DialogTurn } from './flow-chat-manager/types';
import {
  saveAllInProgressTurns,
  immediateSaveDialogTurn,
  createChatSession as createChatSessionModule,
  switchChatSession as switchChatSessionModule,
  deleteChatSession as deleteChatSessionModule,
  renameChatSessionTitle as renameChatSessionTitleModule,
  forkChatSession as forkChatSessionModule,
  cleanupSaveState,
  cleanupSessionBuffers,
  sendMessage as sendMessageModule,
  cancelCurrentTask as cancelCurrentTaskModule,
  installPendingQueueDrainListener,
  drainPendingQueue,
  initializeEventListeners,
  processBatchedEvents,
  addDialogTurn as addDialogTurnModule,
  addImageAnalysisPhase as addImageAnalysisPhaseModule,
  updateImageAnalysisResults as updateImageAnalysisResultsModule,
  updateImageAnalysisItem as updateImageAnalysisItemModule,
  updateSessionMetadata,
} from './flow-chat-manager';

const log = createLogger('FlowChatManager');

export class FlowChatManager {
  private static instance: FlowChatManager;
  private context: FlowChatContext;
  private agentService: AgentService;
  private eventListenerInitialized = false;
  private eventListenerInitializationPromise: Promise<void> | null = null;
  private eventListenerCleanup: (() => void) | null = null;
  private initializationRequests = new Map<string, Promise<boolean>>();
  private latestInitializationRequestKey: string | null = null;

  private constructor() {
    this.context = {
      flowChatStore: FlowChatStore.getInstance(),
      processingManager: processingStatusManager,
      eventBatcher: new EventBatcher({
        onFlush: (events) => this.processBatchedEvents(events)
      }),
      pendingTurnCompletions: new Map(),
      pendingHistoryLoads: new Map(),
      pendingContextRestores: new Map(),
      contentBuffers: new Map(),
      activeTextItems: new Map(),
      saveDebouncers: new Map(),
      lastSaveTimestamps: new Map(),
      lastSaveHashes: new Map(),
      turnSaveInFlight: new Map(),
      turnSavePending: new Set(),
      runtimeStatusTimers: new Map(),
      userCancelledSessionIds: new Set(),
      handledTerminalTurnEvents: new Set(),
      currentWorkspacePath: null
    };
    
    this.agentService = AgentService.getInstance();
    installPendingQueueDrainListener(this.context);
  }

  /** Public hook used by the queue panel "send now" fallback to drain head item. */
  async drainPendingQueueForSession(sessionId: string): Promise<void> {
    return drainPendingQueue(this.context, sessionId);
  }

  public static getInstance(): FlowChatManager {
    if (!FlowChatManager.instance) {
      FlowChatManager.instance = new FlowChatManager();
    }
    return FlowChatManager.instance;
  }

  async initialize(
    workspacePath: string,
    preferredMode?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<boolean> {
    const requestKey = FlowChatManager.createInitializationRequestKey(
      workspacePath,
      preferredMode,
      remoteConnectionId,
      remoteSshHost,
    );
    const existingRequest = this.initializationRequests.get(requestKey);
    this.latestInitializationRequestKey = requestKey;
    if (existingRequest) {
      return existingRequest;
    }

    let request: Promise<boolean>;
    request = this.initializeWorkspace(
      requestKey,
      workspacePath,
      preferredMode,
      remoteConnectionId,
      remoteSshHost,
    ).finally(() => {
      if (this.initializationRequests.get(requestKey) === request) {
        this.initializationRequests.delete(requestKey);
      }
    });
    this.initializationRequests.set(requestKey, request);
    return request;
  }

  private static createInitializationRequestKey(
    workspacePath: string,
    preferredMode?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): string {
    return JSON.stringify([
      workspacePath,
      preferredMode ?? '',
      remoteConnectionId ?? '',
      remoteSshHost ?? '',
    ]);
  }

  private async initializeWorkspace(
    requestKey: string,
    workspacePath: string,
    preferredMode?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<boolean> {
    try {
      await this.initializeEventListeners();

      // Register callback to persist unread completion changes to backend
      this.context.flowChatStore.registerPersistUnreadCompletionCallback(
        (sessionId, value) => {
          updateSessionMetadata(this.context, sessionId).catch(err => {
            log.warn('Failed to persist unread completion change', { sessionId, value, err });
          });
        }
      );

      const initialMetadataPage = await this.context.flowChatStore.loadSessionMetadataPage(
        workspacePath,
        5,
        undefined,
        remoteConnectionId,
        remoteSshHost,
        'flow_chat_manager'
      );

      const sessionMatchesWorkspace = (session: {
        workspacePath?: string;
        remoteConnectionId?: string;
        remoteSshHost?: string;
      }) => {
        const sp = session.workspacePath || workspacePath;
        return sessionBelongsToWorkspaceNavRow(
          {
            workspacePath: sp,
            remoteConnectionId: session.remoteConnectionId,
            remoteSshHost: session.remoteSshHost,
          },
          workspacePath,
          remoteConnectionId,
          remoteSshHost
        );
      };

      let state = this.context.flowChatStore.getState();
      let workspaceSessions = Array.from(state.sessions.values()).filter(sessionMatchesWorkspace);
      if (
        preferredMode &&
        initialMetadataPage.hasMore &&
        !workspaceSessions.some(session => session.mode === preferredMode)
      ) {
        let nextCursor = initialMetadataPage.nextCursor;
        while (nextCursor) {
          const nextPage = await this.context.flowChatStore.loadSessionMetadataPage(
            workspacePath,
            5,
            nextCursor,
            remoteConnectionId,
            remoteSshHost,
            'flow_chat_manager_preferred_mode'
          );
          state = this.context.flowChatStore.getState();
          workspaceSessions = Array.from(state.sessions.values()).filter(sessionMatchesWorkspace);
          if (workspaceSessions.some(session => session.mode === preferredMode) || !nextPage.hasMore) {
            break;
          }
          nextCursor = nextPage.nextCursor;
        }
      }
      const hasHistoricalSessions =
        workspaceSessions.length > 0 ||
        initialMetadataPage.totalTopLevelCount > 0 ||
        initialMetadataPage.sessions.length > 0;
      const isCurrentInitializationRequest = () =>
        this.latestInitializationRequestKey === requestKey;
      const activeSession = state.activeSessionId
        ? state.sessions.get(state.activeSessionId) ?? null
        : null;
      const activeSessionBelongsToWorkspace =
        !!activeSession && sessionMatchesWorkspace(activeSession);
      const activeSessionIdAtAutoSelectStart = state.activeSessionId;

      if (hasHistoricalSessions && !activeSessionBelongsToWorkspace) {
        if (!isCurrentInitializationRequest()) {
          return hasHistoricalSessions;
        }
        const sortedWorkspaceSessions = [...workspaceSessions].sort(compareSessionsForDisplay);
        const latestSession = (preferredMode
          ? sortedWorkspaceSessions.find(session => session.mode === preferredMode)
          : undefined) || sortedWorkspaceSessions[0];

        if (!latestSession) {
          this.context.currentWorkspacePath = workspacePath;
          return hasHistoricalSessions;
        }

        if (latestSession.isHistorical) {
          await this.context.flowChatStore.loadSessionHistory(
            latestSession.sessionId,
            workspacePath,
            undefined,
            latestSession.remoteConnectionId,
            latestSession.remoteSshHost,
            { deferFullHistoryUntilActive: true },
          );
        }

        if (!isCurrentInitializationRequest()) {
          return hasHistoricalSessions;
        }

        const currentState = this.context.flowChatStore.getState();
        const currentActiveSession = currentState.activeSessionId
          ? currentState.sessions.get(currentState.activeSessionId) ?? null
          : null;
        const currentActiveSessionBelongsToWorkspace =
          !!currentActiveSession && sessionMatchesWorkspace(currentActiveSession);
        const activeSessionChangedDuringAutoSelect =
          currentState.activeSessionId !== activeSessionIdAtAutoSelectStart &&
          currentState.activeSessionId !== null;
        if (currentActiveSessionBelongsToWorkspace) {
          this.context.currentWorkspacePath = workspacePath;
          return hasHistoricalSessions;
        }
        if (activeSessionChangedDuringAutoSelect) {
          return hasHistoricalSessions;
        }

        this.context.flowChatStore.switchSession(latestSession.sessionId);
      }

      if (isCurrentInitializationRequest()) {
        this.context.currentWorkspacePath = workspacePath;
      }

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
    if (this.eventListenerInitializationPromise) {
      return this.eventListenerInitializationPromise;
    }

    this.eventListenerInitializationPromise = (async () => {
      this.eventListenerCleanup = await initializeEventListeners(
        this.context,
        (sessionId, turnId, result) => this.handleTodoWriteResult(sessionId, turnId, result)
      );

      this.eventListenerInitialized = true;
    })();

    try {
      await this.eventListenerInitializationPromise;
    } finally {
      this.eventListenerInitializationPromise = null;
    }
  }

  public cleanupEventListeners(): void {
    if (this.eventListenerCleanup) {
      this.eventListenerCleanup();
      this.eventListenerCleanup = null;
      this.eventListenerInitialized = false;
    }
    this.eventListenerInitializationPromise = null;
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

  async createAcpChatSession(clientId: string, config: SessionConfig = {}): Promise<string> {
    const workspacePath =
      config.workspacePath?.trim() ||
      this.context.currentWorkspacePath?.trim();
    if (!workspacePath) {
      throw new Error('Workspace path is required to create an ACP session');
    }

    window.dispatchEvent(new CustomEvent('bitfun:acp-session-creation', {
      detail: { phase: 'start', clientId, action: 'create' },
    }));

    try {
      const response = await ACPClientAPI.createFlowSession({
        clientId,
        workspacePath,
        remoteConnectionId: config.remoteConnectionId,
        remoteSshHost: config.remoteSshHost,
        sessionName: `${clientId} ACP`,
      });

      this.context.flowChatStore.createSession(
        response.sessionId,
        {
          ...config,
          workspacePath,
          agentType: response.agentType,
        },
        undefined,
        response.sessionName,
        128128,
        response.agentType,
        workspacePath,
        config.remoteConnectionId,
        config.remoteSshHost,
      );

      return response.sessionId;
    } finally {
      window.dispatchEvent(new CustomEvent('bitfun:acp-session-creation', {
        detail: { phase: 'finish', clientId, action: 'create' },
      }));
    }
  }

  async switchChatSession(sessionId: string): Promise<void> {
    return switchChatSessionModule(this.context, sessionId);
  }

  async deleteChatSession(sessionId: string): Promise<void> {
    return deleteChatSessionModule(this.context, sessionId);
  }

  public discardLocalSession(sessionId: string): string[] {
    const removedSessionIds = this.context.flowChatStore.removeSession(sessionId);
    removedSessionIds.forEach(id => {
      stateMachineManager.delete(id);
      this.context.processingManager.clearSessionStatus(id);
      cleanupSaveState(this.context, id);
      cleanupSessionBuffers(this.context, id);
    });
    return removedSessionIds;
  }

  public discardLocalSessionsForWorkspace(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>
  ): string[] {
    const removedSessionIds = this.context.flowChatStore.removeSessionsForWorkspace(workspace);
    removedSessionIds.forEach(id => {
      stateMachineManager.delete(id);
      this.context.processingManager.clearSessionStatus(id);
      cleanupSaveState(this.context, id);
      cleanupSessionBuffers(this.context, id);
    });
    return removedSessionIds;
  }

  async refreshWorkspaceSessions(
    workspace: Pick<WorkspaceInfo, 'rootPath' | 'connectionId' | 'sshHost'>
  ): Promise<void> {
    await this.context.flowChatStore.refreshWorkspaceFromDisk(
      workspace.rootPath,
      workspace.connectionId,
      workspace.sshHost
    );
  }

  async renameChatSessionTitle(sessionId: string, title: string): Promise<string> {
    return renameChatSessionTitleModule(this.context, sessionId, title);
  }

  async forkChatSession(sourceSessionId: string, sourceTurnId: string): Promise<string> {
    return forkChatSessionModule(this.context, sourceSessionId, sourceTurnId);
  }

  async resetWorkspaceSessions(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>,
    options?: {
      reinitialize?: boolean;
      preferredMode?: string;
      /** After reinit, ask core to run assistant bootstrap if BOOTSTRAP.md is present (e.g. workspace reset). */
      ensureAssistantBootstrap?: boolean;
    }
  ): Promise<void> {
    const workspacePath = workspace.rootPath;
    const remoteConnectionId = workspace.connectionId ?? null;
    const remoteSshHost = workspace.sshHost ?? null;
    const removedSessionIds = this.context.flowChatStore.removeSessionsForWorkspace(workspace);

    removedSessionIds.forEach(sessionId => {
      stateMachineManager.delete(sessionId);
      this.context.processingManager.clearSessionStatus(sessionId);
      cleanupSaveState(this.context, sessionId);
      cleanupSessionBuffers(this.context, sessionId);
    });

    if (!options?.reinitialize) {
      return;
    }

    const hasHistoricalSessions = await this.initialize(
      workspacePath,
      options.preferredMode,
      remoteConnectionId ?? undefined,
      remoteSshHost ?? undefined
    );
    const state = this.context.flowChatStore.getState();
    const activeSession = state.activeSessionId
      ? state.sessions.get(state.activeSessionId) ?? null
      : null;
    const hasActiveWorkspaceSession =
      !!activeSession &&
      sessionBelongsToWorkspaceNavRow(
        {
          workspacePath: activeSession.workspacePath || workspacePath,
          remoteConnectionId: activeSession.remoteConnectionId,
          remoteSshHost: activeSession.remoteSshHost,
        },
        workspacePath,
        remoteConnectionId,
        remoteSshHost
      );

    if (!hasHistoricalSessions || !hasActiveWorkspaceSession) {
      await this.createChatSession(
        {
          workspacePath,
          workspaceId: workspace.id,
          ...(remoteConnectionId ? { remoteConnectionId } : {}),
          ...(remoteSshHost ? { remoteSshHost } : {}),
        },
        options.preferredMode
      );
    }

    if (options?.ensureAssistantBootstrap) {
      const sid = this.context.flowChatStore.getState().activeSessionId;
      if (sid) {
        try {
          const { agentAPI } = await import('@/infrastructure/api/service-api/AgentAPI');
          await agentAPI.ensureAssistantBootstrap({
            sessionId: sid,
            workspacePath,
          });
        } catch (error) {
          log.warn('ensureAssistantBootstrap after resetWorkspaceSessions failed', {
            workspacePath,
            error,
          });
        }
      }
    }
  }

  async sendMessage(
    message: string,
    sessionId?: string,
    displayMessage?: string,
    agentType?: string,
    switchToMode?: string,
    options?: {
      imageContexts?: import('@/infrastructure/api/service-api/ImageContextTypes').ImageContextData[];
      imageDisplayData?: Array<{ id: string; name: string; dataUrl?: string; imagePath?: string; mimeType?: string }>;
      userMessageMetadata?: Record<string, unknown>;
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
