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
import {
  saveAllInProgressTurns,
  immediateSaveDialogTurn,
  clearAllBuffers,
  createChatSession as createChatSessionModule,
  switchChatSession as switchChatSessionModule,
  deleteChatSession as deleteChatSessionModule,
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
  private initialized = false;

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
    const workspaceChanged = this.context.currentWorkspacePath && 
                            this.context.currentWorkspacePath !== workspacePath;
    
    if (workspaceChanged) {
      await this.cleanup();
      this.initialized = false;
    }
    
    if (this.initialized && !workspaceChanged) {
      return this.context.flowChatStore.getState().sessions.size > 0;
    }
    
    try {
      await this.initializeEventListeners();
      await this.context.flowChatStore.initializeFromDisk(workspacePath);
      
      const state = this.context.flowChatStore.getState();
      const hasHistoricalSessions = state.sessions.size > 0;
      
      if (hasHistoricalSessions && !state.activeSessionId) {
        const sessions = Array.from(state.sessions.values());
        const latestSession = (preferredMode
          ? sessions
              .filter(s => s.mode === preferredMode)
              .sort((a, b) => b.lastActiveAt - a.lastActiveAt)[0]
          : undefined) || sessions.sort((a, b) => b.lastActiveAt - a.lastActiveAt)[0];

        // If no session matches preferred mode, keep activeSessionId unset for caller to create one.
        if (preferredMode && latestSession.mode !== preferredMode) {
          this.initialized = true;
          this.context.currentWorkspacePath = workspacePath;
          return hasHistoricalSessions;
        }

        if (latestSession.isHistorical) {
          await this.context.flowChatStore.loadSessionHistory(latestSession.sessionId, workspacePath);
        }
        
        this.context.flowChatStore.switchSession(latestSession.sessionId);
      }
      
      this.initialized = true;
      this.context.currentWorkspacePath = workspacePath;
      
      return hasHistoricalSessions;
    } catch (error) {
      log.error('Initialization failed', error);
      return false;
    }
  }

  private async cleanup(): Promise<void> {
    try {
      clearAllBuffers(this.context);
      this.context.flowChatStore.setState(() => ({
        sessions: new Map(),
        activeSessionId: null
      }));
    } catch (error) {
      log.error('Cleanup failed', error);
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

  async sendMessage(
    message: string,
    sessionId?: string,
    displayMessage?: string,
    agentType?: string,
    switchToMode?: string
  ): Promise<void> {
    const targetSessionId = sessionId || this.context.flowChatStore.getState().activeSessionId;
    
    if (!targetSessionId) {
      throw new Error('No active session');
    }

    return sendMessageModule(this.context, message, targetSessionId, displayMessage, agentType, switchToMode);
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
