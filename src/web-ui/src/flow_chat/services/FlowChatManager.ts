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
import type { WorkspaceInfo } from '@/shared/types';
import {
  compareSessionsForDisplay,
  sessionBelongsToWorkspaceNavRow,
} from '../utils/sessionOrdering';
import { sessionMatchesWorkspace } from '../utils/workspaceScope';

import type { FlowChatContext, SessionConfig, DialogTurn } from './flow-chat-manager/types';
import type { FlowToolItem, FlowTextItem, ModelRound } from '../types/flow-chat';
import {
  saveAllInProgressTurns,
  immediateSaveDialogTurn,
  createChatSession as createChatSessionModule,
  switchChatSession as switchChatSessionModule,
  deleteChatSession as deleteChatSessionModule,
  renameChatSessionTitle as renameChatSessionTitleModule,
  cleanupSaveState,
  cleanupSessionBuffers,
  sendMessage as sendMessageModule,
  cancelCurrentTask as cancelCurrentTaskModule,
  cancelTaskForSession as cancelTaskForSessionModule,
  initializeEventListeners,
  processBatchedEvents,
  addDialogTurn as addDialogTurnModule,
  addImageAnalysisPhase as addImageAnalysisPhaseModule,
  updateImageAnalysisResults as updateImageAnalysisResultsModule,
  updateImageAnalysisItem as updateImageAnalysisItemModule
} from './flow-chat-manager';

const log = createLogger('FlowChatManager');
const RECENT_WORKSPACE_PRELOAD_LIMIT = 7;
const WARM_HISTORY_SESSION_LIMIT = 5;
const WARM_DISPATCHER_SESSION_LIMIT = 3;
const PRELOAD_WORKSPACE_CONCURRENCY = 2;

type PreloadWorkspaceScope = Pick<
  WorkspaceInfo,
  'id' | 'name' | 'rootPath' | 'connectionId' | 'sshHost'
>;

export class FlowChatManager {
  private static instance: FlowChatManager;
  private context: FlowChatContext;
  private agentService: AgentService;
  private eventListenerInitialized = false;
  private eventListenerCleanup: (() => void) | null = null;

  private constructor() {
    this.context = {
      flowChatStore: FlowChatStore.getInstance(),
      processingManager: processingStatusManager,
      eventBatcher: new EventBatcher({
        onFlush: (events) => this.processBatchedEvents(events)
      }),
      pendingTurnCompletions: new Map(),
      pendingHistoryLoads: new Map(),
      contentBuffers: new Map(),
      activeTextItems: new Map(),
      saveDebouncers: new Map(),
      lastSaveTimestamps: new Map(),
      lastSaveHashes: new Map(),
      turnSaveInFlight: new Map(),
      turnSavePending: new Set(),
      sessionActivityClearTimers: new Map(),
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

  async initialize(
    workspacePath: string,
    preferredMode?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    storageScope?: import('@/shared/types/session-history').SessionStorageScope,
    options?: {
      skipAutoSelectSession?: boolean;
    }
  ): Promise<boolean> {
    try {
      await this.initializeEventListeners();
      await this.context.flowChatStore.initializeFromDisk(
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
        storageScope
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

      const state = this.context.flowChatStore.getState();
      const workspaceSessions = Array.from(state.sessions.values()).filter(sessionMatchesWorkspace);
      const hasHistoricalSessions = workspaceSessions.length > 0;
      const activeSession = state.activeSessionId
        ? state.sessions.get(state.activeSessionId) ?? null
        : null;
      const activeSessionBelongsToWorkspace =
        !!activeSession && sessionMatchesWorkspace(activeSession);

      if (
        hasHistoricalSessions &&
        !activeSessionBelongsToWorkspace &&
        !options?.skipAutoSelectSession
      ) {
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
            latestSession.storageScope
          );
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

    this.eventListenerCleanup = await initializeEventListeners(
      this.context,
      (sessionId, turnId, result) => this.handleTodoWriteResult(sessionId, turnId, result)
    );
    
    this.eventListenerInitialized = true;
  }

  public async preloadRecentWorkspaceSessions(
    workspaces: PreloadWorkspaceScope[],
    options?: {
      metadataLimit?: number;
      warmHistoryCount?: number;
      warmDispatcherCount?: number;
      force?: boolean;
    }
  ): Promise<{
    attemptedWorkspaceCount: number;
    metadataLoadedCount: number;
    warmedSessionCount: number;
    warmedDispatcherCount: number;
    failedWorkspaces: string[];
  }> {
    const metadataLimit = options?.metadataLimit ?? RECENT_WORKSPACE_PRELOAD_LIMIT;
    const warmHistoryCount = options?.warmHistoryCount ?? WARM_HISTORY_SESSION_LIMIT;
    const warmDispatcherCount = options?.warmDispatcherCount ?? WARM_DISPATCHER_SESSION_LIMIT;
    const scopedWorkspaces = workspaces.slice(0, metadataLimit);
    const failedWorkspaces: string[] = [];
    let metadataLoadedCount = 0;

    const runPreload = async (workspace: PreloadWorkspaceScope) => {
      const remoteConnectionId = workspace.connectionId ?? undefined;
      const remoteSshHost = workspace.sshHost ?? undefined;
      if (
        !options?.force &&
        this.context.flowChatStore.hasWorkspaceMetadataPreloaded(
          workspace.rootPath,
          remoteConnectionId,
          remoteSshHost
        )
      ) {
        return;
      }

      try {
        const { sessionAPI } = await import('@/infrastructure/api');
        const metadata = await sessionAPI.listSessions(
          workspace.rootPath,
          remoteConnectionId,
          remoteSshHost,
          'workspace'
        );
        const inserted = await this.context.flowChatStore.hydrateWorkspaceSessionsMetadata(
          metadata,
          workspace.rootPath,
          remoteConnectionId,
          remoteSshHost,
          'workspace'
        );
        metadataLoadedCount += inserted;
      } catch (error) {
        failedWorkspaces.push(workspace.name || workspace.rootPath);
        log.warn('Failed to preload workspace sessions', {
          workspaceId: workspace.id,
          workspacePath: workspace.rootPath,
          error,
        });
      }
    };

    for (let index = 0; index < scopedWorkspaces.length; index += PRELOAD_WORKSPACE_CONCURRENCY) {
      const batch = scopedWorkspaces.slice(index, index + PRELOAD_WORKSPACE_CONCURRENCY);
      await Promise.all(batch.map(runPreload));
    }

    const warmedSessionCandidates = Array.from(this.context.flowChatStore.getState().sessions.values())
      .filter(session => {
        if (!session.isHistorical) return false;
        if (session.mode?.toLowerCase() === 'dispatcher') return false;
        if (this.context.flowChatStore.hasSessionHistoryWarmed(session.sessionId)) return false;
        return scopedWorkspaces.some(workspace => sessionMatchesWorkspace(session, workspace));
      })
      .sort(compareSessionsForDisplay)
      .slice(0, warmHistoryCount);

    const warmedDispatcherCandidates = Array.from(this.context.flowChatStore.getState().sessions.values())
      .filter(session => {
        if (!session.isHistorical) return false;
        if (session.mode?.toLowerCase() !== 'dispatcher') return false;
        if (this.context.flowChatStore.hasSessionHistoryWarmed(session.sessionId)) return false;
        return true;
      })
      .sort(compareSessionsForDisplay)
      .slice(0, warmDispatcherCount);

    let warmedSessionCount = 0;
    let warmedDispatcherCount = 0;
    await Promise.allSettled(
      warmedSessionCandidates.map(async session => {
        const workspacePath = session.workspacePath;
        if (!workspacePath) return;
        try {
          await this.context.flowChatStore.loadSessionHistory(
            session.sessionId,
            workspacePath,
            undefined,
            session.remoteConnectionId,
            session.remoteSshHost,
            session.storageScope
          );
          warmedSessionCount += 1;
        } catch (error) {
          log.warn('Failed to warm historical session', {
            sessionId: session.sessionId,
            workspacePath,
            error,
          });
        }
      })
    );

    await Promise.allSettled(
      warmedDispatcherCandidates.map(async session => {
        const workspacePath = session.workspacePath;
        if (!workspacePath) return;
        try {
          await this.context.flowChatStore.loadSessionHistory(
            session.sessionId,
            workspacePath,
            undefined,
            session.remoteConnectionId,
            session.remoteSshHost,
            session.storageScope
          );
          warmedDispatcherCount += 1;
        } catch (error) {
          log.warn('Failed to warm dispatcher session', {
            sessionId: session.sessionId,
            workspacePath,
            error,
          });
        }
      })
    );

    return {
      attemptedWorkspaceCount: scopedWorkspaces.length,
      metadataLoadedCount,
      warmedSessionCount,
      warmedDispatcherCount,
      failedWorkspaces,
    };
  }

  public async preloadAgenticOsSessions(options?: {
    warmDispatcherCount?: number;
  }): Promise<{ metadataLoadedCount: number; warmedDispatcherCount: number }> {
    const warmDispatcherCount = options?.warmDispatcherCount ?? WARM_DISPATCHER_SESSION_LIMIT;
    const { sessionAPI } = await import('@/infrastructure/api');
    const metadata = await sessionAPI.listSessions(undefined, undefined, undefined, 'agentic_os');
    const metadataLoadedCount = await this.context.flowChatStore.hydrateWorkspaceSessionsMetadata(
      metadata,
      '',
      undefined,
      undefined,
      'agentic_os'
    );
    const candidates = Array.from(this.context.flowChatStore.getState().sessions.values())
      .filter(session =>
        session.mode?.toLowerCase() === 'dispatcher' &&
        session.isHistorical &&
        !this.context.flowChatStore.hasSessionHistoryWarmed(session.sessionId)
      )
      .sort(compareSessionsForDisplay)
      .slice(0, warmDispatcherCount);
    let warmedDispatcherCount = 0;
    await Promise.allSettled(
      candidates.map(async session => {
        await this.context.flowChatStore.loadSessionHistory(
          session.sessionId,
          session.workspacePath || '',
          undefined,
          session.remoteConnectionId,
          session.remoteSshHost,
          'agentic_os'
        );
        warmedDispatcherCount += 1;
      })
    );
    return { metadataLoadedCount, warmedDispatcherCount };
  }

  public cleanupEventListeners(): void {
    if (this.eventListenerCleanup) {
      this.eventListenerCleanup();
      this.eventListenerCleanup = null;
      this.eventListenerInitialized = false;
    }
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

  async renameChatSessionTitle(sessionId: string, title: string): Promise<string> {
    return renameChatSessionTitleModule(this.context, sessionId, title);
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

  async cancelTaskForSession(sessionId: string): Promise<boolean> {
    return cancelTaskForSessionModule(this.context, sessionId);
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
    if (
      turn.status !== 'processing' &&
      turn.status !== 'finishing' &&
      turn.status !== 'image_analyzing'
    ) {
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
          parentSessionId,
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
