/**
 * Flow Chat global state store
 * Prevents state loss when components remount
 */

import { FlowChatState, Session, DialogTurn, ModelRound, FlowItem, SessionConfig } from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('FlowChatStore');

export class FlowChatStore {
  private static instance: FlowChatStore;
  private state: FlowChatState;
  private listeners: Set<(state: FlowChatState) => void> = new Set();
  
  private silentMode = false;

  private constructor() {
    this.clearOldStorage();
    this.state = {
      sessions: new Map(),
      activeSessionId: null
    };
  }

  private clearOldStorage(): void {
    try {
      const keysToRemove = [
        'bitfun-flow-chat-state',
        'bitfun-flow-chat-global',
        'bitfun-session-ids'
      ];
      
      keysToRemove.forEach(key => {
        if (localStorage.getItem(key)) {
          localStorage.removeItem(key);
        }
      });

      Object.keys(localStorage).forEach(key => {
        if (key.startsWith('bitfun-session-')) {
          localStorage.removeItem(key);
        }
      });
    } catch (error) {
      log.warn('Failed to clear old storage data', error);
    }
  }


  public static getInstance(): FlowChatStore {
    if (!FlowChatStore.instance) {
      FlowChatStore.instance = new FlowChatStore();
    }
    return FlowChatStore.instance;
  }

  public getState(): FlowChatState {
    return this.state;
  }

  public setState(updater: (prevState: FlowChatState) => FlowChatState): void {
    const newState = updater(this.state);
    this.state = newState;
    
    if (!this.silentMode) {
      this.listeners.forEach(listener => listener(newState));
    }
  }
  
  /**
   * Silent state update (does not trigger listeners)
   * Used for batch updates, call notifyListeners() after completion
   */
  public setStateSilent(updater: (prevState: FlowChatState) => FlowChatState): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.setState(updater);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }
  
  /**
   * Manually notify all listeners (call after batch updates complete)
   */
  public notifyListeners(): void {
    this.listeners.forEach(listener => listener(this.state));
  }
  
  public beginSilentMode(): void {
    this.silentMode = true;
  }
  
  public endSilentMode(): void {
    this.silentMode = false;
    this.notifyListeners();
  }

  public subscribe(listener: (state: FlowChatState) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  public createSession(sessionId: string, config: SessionConfig, _unused?: undefined, title?: string, maxContextTokens?: number, mode?: string): void {
    import('../state-machine').then(({ stateMachineManager }) => {
      stateMachineManager.getOrCreate(sessionId);
    });
    
    this.setState(prev => {
      const session: Session = {
        sessionId,
        title: title || i18nService.t('flow-chat:session.new'),
        titleStatus: undefined,
        dialogTurns: [],
        status: 'idle',
        config,
        createdAt: Date.now(),
        lastActiveAt: Date.now(),
        error: null,
        maxContextTokens: maxContextTokens || 128128,
        mode: mode || 'agentic',
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, session);

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: sessionId
      };
    });
  }

  public switchSession(sessionId: string): void {
    let sessionMode: string | undefined;
    
    this.setState(prev => {
      if (!prev.sessions.has(sessionId)) return prev;
      
      const session = prev.sessions.get(sessionId)!;
      sessionMode = session.mode;
      
      const updatedSession = {
        ...session,
        lastActiveAt: Date.now()
      };
      
      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: sessionId
      };
    });
    
    window.dispatchEvent(new CustomEvent('bitfun:session-switched', {
      detail: { sessionId, mode: sessionMode || 'agentic' }
    }));
  }

  /**
   * Update session mode
   * @param sessionId Session ID
   * @param mode Mode ID (e.g., 'agentic', 'Plan')
   */
  public updateSessionMode(sessionId: string, mode: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.mode === mode) return prev;

      const updatedSession = {
        ...session,
        mode,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Move session to front by updating createdAt timestamp
   */
  public moveSessionToFront(sessionId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession = {
        ...session,
        createdAt: Date.now(),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public async deleteSession(sessionId: string): Promise<void> {
    const { stateMachineManager } = await import('../state-machine');
    stateMachineManager.delete(sessionId);
    
    try {
      const { agentAPI } = await import('@/infrastructure/api');
      await agentAPI.deleteSession(sessionId);
    } catch (error) {
      log.error('Failed to delete session on backend', { sessionId, error });
    }
    
    this.setState(prev => {
      const newSessions = new Map(prev.sessions);
      newSessions.delete(sessionId);

      let newActiveSessionId = prev.activeSessionId;
      if (prev.activeSessionId === sessionId) {
        const remainingSessions = Array.from(newSessions.keys());
        newActiveSessionId = remainingSessions.length > 0 ? remainingSessions[0] : null;
      }

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: newActiveSessionId
      };
    });
  }

  public clearSession(sessionId?: string): void {
    const targetSessionId = sessionId || this.state.activeSessionId;
    if (!targetSessionId) return;

    this.setState(prev => {
      const session = prev.sessions.get(targetSessionId);
      if (!session) return prev;

      const clearedSession = {
        ...session,
        dialogTurns: [],
        error: null,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(targetSessionId, clearedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public getActiveSession(): Session | null {
    if (!this.state.activeSessionId) {
      return null;
    }
    return this.state.sessions.get(this.state.activeSessionId) || null;
  }

  public addDialogTurn(sessionId: string, dialogTurn: DialogTurn): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.dialogTurns.some(turn => turn.id === dialogTurn.id)) {
        return prev;
      }

      const updatedSession = {
        ...session,
        dialogTurns: [...session.dialogTurns, dialogTurn],
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public deleteDialogTurn(sessionId: string, dialogTurnId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedDialogTurns = session.dialogTurns.filter(turn => turn.id !== dialogTurnId);

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Delete all dialog turns from turnIndex (inclusive)
   * Used for turn rollback: revert to before this turn and remove this turn and all subsequent history
   */
  public truncateDialogTurnsFrom(sessionId: string, turnIndex: number): void {

    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const clampedIndex = Math.max(0, Math.min(turnIndex, session.dialogTurns.length));
      const updatedSession = {
        ...session,
        dialogTurns: session.dialogTurns.slice(0, clampedIndex),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public updateDialogTurn(sessionId: string, dialogTurnId: string, updater: (turn: DialogTurn) => DialogTurn): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedDialogTurns = session.dialogTurns.map(turn => 
        turn.id === dialogTurnId ? updater(turn) : turn
      );

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Add image analysis phase to dialog turn
   */
  public addImageAnalysisPhase(
    sessionId: string, 
    dialogTurnId: string, 
    imageContexts: import('@/shared/types/context').ImageContext[]
  ): void {
    import('../types/flow-chat').then(({ FlowImageAnalysisItem }) => {
      this.updateDialogTurn(sessionId, dialogTurnId, turn => {
        const imageAnalysisItems: any[] = imageContexts.map((ctx, index) => ({
          id: `img-analysis-${ctx.id}`,
          type: 'image-analysis',
          imageContext: ctx,
          result: null,
          status: 'analyzing',
          timestamp: Date.now() + index,
        }));

        return {
          ...turn,
          imageAnalysisPhase: {
            items: imageAnalysisItems,
            status: 'analyzing',
            startTime: Date.now(),
          },
          status: 'image_analyzing',
        };
      });
    });
  }

  /**
   * Update image analysis results
   */
  public updateImageAnalysisResults(
    sessionId: string,
    dialogTurnId: string,
    results: import('../types/flow-chat').ImageAnalysisResult[]
  ): void {
      this.updateDialogTurn(sessionId, dialogTurnId, turn => {
        if (!turn.imageAnalysisPhase) {
          log.warn('Attempting to update non-existent image analysis phase', { sessionId, dialogTurnId });
          return turn;
        }

      const updatedItems = turn.imageAnalysisPhase.items.map(item => {
        const result = results.find(r => r.image_id === item.imageContext.id);
        if (result) {
          return {
            ...item,
            result,
            status: 'completed',
          };
        }
        return item;
      });

      const allCompleted = updatedItems.every(item => item.status === 'completed');

      return {
        ...turn,
        imageAnalysisPhase: {
          ...turn.imageAnalysisPhase,
          items: updatedItems,
          status: allCompleted ? 'completed' : 'analyzing',
          endTime: allCompleted ? Date.now() : undefined,
        },
        status: allCompleted ? 'pending' : 'image_analyzing',
      };
    });
  }

  /**
   * Update single image analysis item status (for error handling)
   */
  public updateImageAnalysisItem(
    sessionId: string,
    dialogTurnId: string,
    imageId: string,
    updates: { status?: 'analyzing' | 'completed' | 'error'; error?: string; result?: any }
  ): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      if (!turn.imageAnalysisPhase) return turn;

      const updatedItems = turn.imageAnalysisPhase.items.map(item => {
        if (item.imageContext.id === imageId) {
          return { ...item, ...updates };
        }
        return item;
      });

      return {
        ...turn,
        imageAnalysisPhase: {
          ...turn.imageAnalysisPhase,
          items: updatedItems,
        },
      };
    });
  }

  public addModelRound(sessionId: string, dialogTurnId: string, modelRound: ModelRound): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => ({
      ...turn,
      modelRounds: [...turn.modelRounds, modelRound],
      status: 'processing'
    }));
  }

  public updateModelRound(sessionId: string, dialogTurnId: string, modelRoundId: string, updater: (round: ModelRound) => ModelRound): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => ({
      ...turn,
      modelRounds: turn.modelRounds.map(round => 
        round.id === modelRoundId ? updater(round) : round
      )
    }));
  }

  /**
   * Batch update multiple model round items (reduces store update frequency)
   */
  public batchUpdateModelRoundItems(
    sessionId: string, 
    dialogTurnId: string, 
    updates: Array<{ itemId: string; changes: Partial<FlowItem> }>
  ): void {
    if (updates.length === 0) return;
    
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      const updatedModelRounds = turn.modelRounds.map(round => ({
        ...round,
        items: round.items.map(item => {
          const update = updates.find(u => u.itemId === item.id);
          return update ? { ...item, ...update.changes } : item;
        })
      }));
      
      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  public addModelRoundItem(sessionId: string, dialogTurnId: string, item: FlowItem, modelRoundId?: string): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let targetModelRoundIndex = turn.modelRounds.length - 1;
        if (modelRoundId) {
          targetModelRoundIndex = turn.modelRounds.findIndex(round => round.id === modelRoundId);
          if (targetModelRoundIndex === -1) {
            log.warn('Model round not found', { sessionId, dialogTurnId, modelRoundId });
            return turn;
          }
        }
        
        if (targetModelRoundIndex === -1) {
          log.warn('No available model rounds', { sessionId, dialogTurnId });
          return turn;
        }

      const targetModelRound = turn.modelRounds[targetModelRoundIndex];

      const existingItem = targetModelRound.items.find(existingItem => existingItem.id === item.id);
      if (existingItem) {
        return turn;
      }

      const updatedModelRounds = [...turn.modelRounds];
      
      updatedModelRounds[targetModelRoundIndex] = {
        ...targetModelRound,
        items: [...targetModelRound.items, item]
      };

      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  /**
   * Silent add ModelRound item (does not trigger listeners)
   * Used for batch update scenarios
   */
  public addModelRoundItemSilent(sessionId: string, dialogTurnId: string, item: FlowItem, modelRoundId?: string): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.addModelRoundItem(sessionId, dialogTurnId, item, modelRoundId);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }

  /**
   * Insert new FlowItem after specified tool item (for subagent content flattening)
   * @param sessionId Session ID
   * @param dialogTurnId Dialog turn ID
   * @param parentToolId Parent tool ID
   * @param newItem New item to insert
   */
  public insertModelRoundItemAfterTool(sessionId: string, dialogTurnId: string, parentToolId: string, newItem: FlowItem): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let parentRoundIndex = -1;
      let parentItemIndex = -1;
      
      for (let i = 0; i < turn.modelRounds.length; i++) {
        const itemIndex = turn.modelRounds[i].items.findIndex((item: any) => item.id === parentToolId);
        if (itemIndex !== -1) {
          parentRoundIndex = i;
          parentItemIndex = itemIndex;
          break;
        }
      }
      
      if (parentRoundIndex === -1 || parentItemIndex === -1) {
        log.warn('Parent tool item not found', { sessionId, dialogTurnId, parentToolId });
        return turn;
      }
      
      const targetModelRound = turn.modelRounds[parentRoundIndex];
      
      const existingItem = targetModelRound.items.find((item: any) => item.id === newItem.id);
      if (existingItem) {
        return turn;
      }
      
      let insertIndex = parentItemIndex + 1;
      
      while (insertIndex < targetModelRound.items.length) {
        const currentItem = targetModelRound.items[insertIndex] as any;
        if (currentItem.parentTaskToolId === parentToolId && currentItem.isSubagentItem) {
          insertIndex++;
        } else {
          break;
        }
      }
      
      const updatedItems = [
        ...targetModelRound.items.slice(0, insertIndex),
        newItem,
        ...targetModelRound.items.slice(insertIndex)
      ];
      
      const updatedModelRounds = [...turn.modelRounds];
      updatedModelRounds[parentRoundIndex] = {
        ...targetModelRound,
        items: updatedItems
      };
      
      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  public updateModelRoundItem(sessionId: string, dialogTurnId: string, itemId: string, updates: Partial<FlowItem>): void {
    let foundItemsCount = 0;

    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let updated = false;
      
      turn.modelRounds.forEach(modelRound => {
        const hasItem = modelRound.items.some((item: any) => item.id === itemId);
        if (hasItem) {
          foundItemsCount++;
        }
      });
      
      const updatedModelRounds = turn.modelRounds.map(modelRound => {
        if (updated) return modelRound;
        
        const updatedItems = modelRound.items.map((item: any) => {
          if (item.id === itemId) {
            const updatedItem = { ...item, ...updates };
            return updatedItem;
          }
          return item;
        });
        
        if (updatedItems.some((item: any) => item.id === itemId)) {
          updated = true;
          return { ...modelRound, items: updatedItems };
        }
        
        return modelRound;
      });
      
      if (!updated) {
        log.warn('Item not found for update', { sessionId, dialogTurnId, itemId });
        return turn;
      }

      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  /**
   * Silent update ModelRound item (does not trigger listeners)
   * Used for batch update scenarios
   */
  public updateModelRoundItemSilent(sessionId: string, dialogTurnId: string, itemId: string, updates: Partial<FlowItem>): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.updateModelRoundItem(sessionId, dialogTurnId, itemId, updates);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }

  /**
   * Find tool item (for early detection updates)
   */
  public findToolItem(sessionId: string, dialogTurnId: string, toolUseId: string): FlowItem | null {
    const session = this.state.sessions.get(sessionId);
    if (!session) return null;

    const dialogTurn = session.dialogTurns.find(turn => turn.id === dialogTurnId);
    if (!dialogTurn) return null;

    for (const modelRound of dialogTurn.modelRounds) {
      const item = modelRound.items.find((item: any) => item.id === toolUseId);
      if (item) {
        return item;
      }
    }

    return null;
  }

  public updateTokenUsage(
    sessionId: string, 
    tokenUsage: { inputTokens: number; outputTokens?: number; totalTokens: number }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession = {
        ...session,
        currentTokenUsage: {
          inputTokens: tokenUsage.inputTokens,
          outputTokens: tokenUsage.outputTokens,
          totalTokens: tokenUsage.totalTokens,
          timestamp: Date.now()
        }
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public rollbackTokenUsage(sessionId: string): void {
    void sessionId;
  }

  public updateSessionMaxContextTokens(sessionId: string, maxContextTokens: number): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.maxContextTokens === maxContextTokens) return prev;

      const updatedSession = {
        ...session,
        maxContextTokens
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public setError(sessionId: string, error: string | null): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        error,
        status: error ? 'error' as const : 'idle' as const,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public async updateSessionTitle(
    sessionId: string, 
    title: string, 
    status: 'generating' | 'generated' | 'failed'
  ): Promise<void> {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession = {
        ...session,
        title,
        titleStatus: status,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });

    if (status === 'generated') {
      try {
        const { conversationAPI } = await import('@/infrastructure/api');
        const { workspaceManager } = await import('@/infrastructure/services/business/workspaceManager');
        
        const workspacePath = workspaceManager.getState().currentWorkspace?.rootPath;
        if (!workspacePath) {
          log.warn('Workspace path not available, skipping title sync', { sessionId });
          return;
        }

        const metadata = await conversationAPI.loadSessionMetadata(sessionId, workspacePath);
        if (metadata) {
          metadata.sessionName = title;
          await conversationAPI.saveSessionMetadata(metadata, workspacePath);
        }
      } catch (error) {
        log.error('Failed to sync session title', { sessionId, error });
      }
    }
  }

  /**
   * Cancel current session task (UI state update)
   * Called by SessionStateMachine side effects, updates all related states to cancelled
   */
  public cancelSessionTask(sessionId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found', { sessionId });
        return prev;
      }

      const lastDialogTurn = session.dialogTurns[session.dialogTurns.length - 1];
      if (!lastDialogTurn) {
        log.warn('No dialog turns found', { sessionId });
        return prev;
      }

      if (lastDialogTurn.status === 'completed' || lastDialogTurn.status === 'cancelled') {
        return prev;
      }

      const updatedDialogTurns = session.dialogTurns.map((turn, index) => {
        if (index !== session.dialogTurns.length - 1) {
          return turn;
        }

        const updatedModelRounds = turn.modelRounds.map(round => {
          const updatedItems = round.items.map(item => {
            if (item.status === 'completed' || item.status === 'cancelled' || item.status === 'error') {
              return item;
            }
            
            return {
              ...item,
              status: 'cancelled' as const,
              ...(item.type === 'text' && { isStreaming: false }),
              ...(item.type === 'tool' && {
                isParamsStreaming: false,
                endTime: (item as any).endTime || Date.now()
              }),
              ...(item.type === 'thinking' && {
                isCollapsed: true,
                isStreaming: false
              })
            };
          });

          return {
            ...round,
            items: updatedItems,
            isStreaming: false,
            isComplete: true,
            endTime: round.endTime || Date.now()
          };
        });

        return {
          ...turn,
          status: 'cancelled' as const,
          modelRounds: updatedModelRounds,
          endTime: Date.now()
        };
      });

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        status: 'idle' as const,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      window.dispatchEvent(new CustomEvent('bitfun:dialog-cancelled', {
        detail: { sessionId }
      }));

      const lastTurn = updatedDialogTurns[updatedDialogTurns.length - 1];
      if (lastTurn && lastTurn.status === 'cancelled') {
        this.saveCancelledDialogTurn(sessionId, lastTurn.id).catch(error => {
          log.error('Failed to save cancelled dialog turn', { sessionId, turnId: lastTurn.id, error });
        });
      }

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Save cancelled dialog turn to disk
   */
  private async saveCancelledDialogTurn(sessionId: string, turnId: string): Promise<void> {
    try {
      const { globalAPI } = await import('@/infrastructure/api');
      const { conversationAPI } = await import('@/infrastructure/api');
      
      const workspacePath = await globalAPI.getCurrentWorkspacePath();
      if (!workspacePath) {
        log.warn('Workspace path not available, skipping save', { sessionId, turnId });
        return;
      }

      const session = this.state.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, skipping save', { sessionId, turnId });
        return;
      }

      const dialogTurn = session.dialogTurns.find(turn => turn.id === turnId);
      if (!dialogTurn) {
        log.warn('Dialog turn not found, skipping save', { sessionId, turnId });
        return;
      }

      const turnIndex = session.dialogTurns.findIndex(t => t.id === turnId);
      
      const turnData = {
        turnId,
        turnIndex,
        sessionId,
        timestamp: dialogTurn.startTime,
        userMessage: {
          id: dialogTurn.userMessage.id,
          content: dialogTurn.userMessage.content,
          timestamp: dialogTurn.userMessage.timestamp
        },
        modelRounds: dialogTurn.modelRounds.map((round, roundIndex) => {
          const textItems = round.items
            .filter(item => item.type === 'text')
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              timestamp: item.timestamp
            }));
          
          const toolItems = round.items
            .filter(item => item.type === 'tool')
            .map(item => ({
              id: item.id,
              toolName: (item as any).toolName || '',
              toolCall: (item as any).toolCall || { input: {}, id: item.id },
              toolResult: (item as any).toolResult,
              aiIntent: (item as any).aiIntent,
              startTime: (item as any).startTime || item.timestamp,
              endTime: (item as any).endTime,
              durationMs: (item as any).endTime 
                ? (item as any).endTime - (item as any).startTime 
                : undefined
            }));
          
          const thinkingItems = round.items
            .filter(item => item.type === 'thinking')
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              isCollapsed: (item as any).isCollapsed || false,
              timestamp: item.timestamp
            }));
          
          return {
            id: round.id,
            turnId,
            roundIndex,
            timestamp: round.startTime,
            textItems,
            toolItems,
            thinkingItems,
            startTime: round.startTime,
            endTime: round.endTime || Date.now(),
            status: round.status
          };
        }),
        startTime: dialogTurn.startTime,
        endTime: dialogTurn.endTime || Date.now(),
        durationMs: (dialogTurn.endTime || Date.now()) - dialogTurn.startTime,
        status: 'cancelled' as const
      };

      await conversationAPI.saveDialogTurn(turnData, workspacePath);
    } catch (error) {
      log.error('Failed to save cancelled dialog turn', { sessionId, turnId, error });
    }
  }


  /**
   * Initialize by loading historical session list from disk (metadata only)
   * Merges with existing sessions instead of replacing to preserve new sessions in memory
   */
  public async initializeFromDisk(workspacePath: string): Promise<void> {
    try {
      const { conversationAPI } = await import('@/infrastructure/api');
      const sessions = await conversationAPI.getConversationSessions(workspacePath);
      
      const { stateMachineManager } = await import('../state-machine');
      sessions.forEach(metadata => {
        stateMachineManager.getOrCreate(metadata.sessionId);
      });
      
      const processSession = async (metadata: any) => {
        const existingSession = this.state.sessions.get(metadata.sessionId);
        if (existingSession) {
          return;
        }
        
        let maxContextTokens = 128128;
        try {
          const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
          const models = await configManager.getConfig<any[]>('ai.models') || [];
          
          if (metadata.modelName) {
            const model = models.find((m: any) => m.name === metadata.modelName || m.id === metadata.modelName);
            if (model?.context_window) {
              maxContextTokens = model.context_window;
            }
          }
          
          if (maxContextTokens === 128128) {
            const defaultModels = await configManager.getConfig<Record<string, string>>('ai.default_models');
            const primaryModelId = defaultModels?.primary;
            
            if (primaryModelId) {
              const primaryModel = models.find((m: any) => m.id === primaryModelId);
              if (primaryModel?.context_window) {
                maxContextTokens = primaryModel.context_window;
              }
            }
          }
        } catch (error) {
          log.warn('Failed to get model context window size, using default', { sessionId: metadata.sessionId, error });
        }
        
        this.setState(prev => {
          if (prev.sessions.has(metadata.sessionId)) {
            return prev;
          }
          
          const VALID_AGENT_TYPES = ['agentic', 'debug', 'Plan', 'Cowork'];
          const rawAgentType = metadata.agentType || 'agentic';
          const validatedAgentType = VALID_AGENT_TYPES.includes(rawAgentType) ? rawAgentType : 'agentic';
          
          if (rawAgentType !== validatedAgentType) {
            log.warn('Invalid agentType, falling back to agentic', { sessionId: metadata.sessionId, rawAgentType, validatedAgentType });
          }
          
          const session: Session = {
            sessionId: metadata.sessionId,
            title: metadata.sessionName,
            titleStatus: 'generated',
            dialogTurns: [],
            status: 'idle',
            config: {
              agentType: validatedAgentType,
              modelName: metadata.modelName,
            },
            createdAt: metadata.createdAt,
            lastActiveAt: metadata.lastActiveAt,
            error: null,
            isHistorical: true,
            todos: (metadata as any).todos || [],
            maxContextTokens,
            mode: validatedAgentType,
          };
          
          const newSessions = new Map(prev.sessions);
          newSessions.set(metadata.sessionId, session);
          
          return {
            ...prev,
            sessions: newSessions,
          };
        });
      };
      
      await Promise.all(sessions.map(processSession));
    } catch (error) {
      log.error('Failed to load historical sessions', error);
    }
  }

  /**
   * Lazy load session history (convert historical data to FlowChat format)
   */
  public async loadSessionHistory(
    sessionId: string,
    workspacePath: string,
    limit?: number
  ): Promise<void> {
    try {
      const { stateMachineManager } = await import('../state-machine');
      stateMachineManager.getOrCreate(sessionId);
      
      try {
        const { agentAPI } = await import('@/infrastructure/api');
        await agentAPI.restoreSession(sessionId);
      } catch (error) {
        log.warn('Backend session restore failed (may be new session)', { sessionId, error });
      }
      
      const { conversationAPI } = await import('@/infrastructure/api');
      const turns = await conversationAPI.loadConversationHistory(
        sessionId,
        workspacePath,
        limit
      );
      
      const dialogTurns = this.convertToDialogTurns(turns);
      
      this.setState(prev => {
        const session = prev.sessions.get(sessionId);
        if (!session) return prev;
        
        const updatedSession = {
          ...session,
          dialogTurns,
          isHistorical: false,
        };
        
        const newSessions = new Map(prev.sessions);
        newSessions.set(sessionId, updatedSession);
        
        return {
          ...prev,
          sessions: newSessions,
        };
      });
    } catch (error) {
      log.error('Failed to load session history', { sessionId, error });
      throw error;
    }
  }

  /**
   * Convert DialogTurnData to FlowChat DialogTurn format
   */
  private convertToDialogTurns(turns: any[]): DialogTurn[] {
    return turns.map(turn => ({
      id: turn.turnId,
      sessionId: turn.sessionId,
      userMessage: {
        id: turn.userMessage.id,
        type: 'user' as const,
        content: turn.userMessage.content,
        timestamp: turn.userMessage.timestamp,
      },
      modelRounds: turn.modelRounds.map((round: any) => ({
        id: round.id,
        turnId: round.turnId,
        items: [
          ...round.textItems.map((text: any) => ({
            id: text.id,
            type: 'text' as const,
            content: text.content,
            isStreaming: text.isStreaming,
            isMarkdown: text.isMarkdown !== undefined ? text.isMarkdown : true,
            timestamp: text.timestamp,
            status: text.status || 'completed' as const,
            orderIndex: text.orderIndex,
            isSubagentItem: text.isSubagentItem,
            parentTaskToolId: text.parentTaskToolId,
            subagentSessionId: text.subagentSessionId,
          })),
          ...round.toolItems.map((tool: any) => {
            let inferredStatus = 'completed';
            if (tool.status) {
              inferredStatus = tool.status;
            } else if (tool.toolResult) {
              inferredStatus = tool.toolResult.success === false ? 'error' : 'completed';
            }
            
            return {
              id: tool.id,
              type: 'tool' as const,
              toolName: tool.toolName,
              toolCall: tool.toolCall,
              toolResult: tool.toolResult,
              aiIntent: tool.aiIntent,
              startTime: tool.startTime,
              endTime: tool.endTime,
              timestamp: tool.startTime,
              status: inferredStatus,
              orderIndex: tool.orderIndex,
              isSubagentItem: tool.isSubagentItem,
              parentTaskToolId: tool.parentTaskToolId,
              subagentSessionId: tool.subagentSessionId,
            };
          }),
          ...(round.thinkingItems || []).map((thinking: any) => ({
            id: thinking.id,
            type: 'thinking' as const,
            content: thinking.content,
            isStreaming: thinking.isStreaming || false,
            isCollapsed: thinking.isCollapsed || true,
            timestamp: thinking.timestamp,
            status: thinking.status || 'completed' as const,
            orderIndex: thinking.orderIndex,
            isSubagentItem: thinking.isSubagentItem,
            parentTaskToolId: thinking.parentTaskToolId,
            subagentSessionId: thinking.subagentSessionId,
          })),
        ].sort((a: any, b: any) => {
          const aIndex = a.orderIndex !== undefined ? a.orderIndex : a.timestamp || 0;
          const bIndex = b.orderIndex !== undefined ? b.orderIndex : b.timestamp || 0;
          
          return aIndex - bIndex;
        }),
        status: round.status,
        timestamp: round.timestamp,
      })),
      timestamp: turn.timestamp,
      status: turn.status,
      startTime: turn.startTime,
    }));
  }

  public setDialogTurnTodos(sessionId: string, turnId: string, todos: import('../types/flow-chat').TodoItem[]): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, cannot set turn todos', { sessionId, turnId });
        return prev;
      }

      const turnIndex = session.dialogTurns.findIndex(turn => turn.id === turnId);
      if (turnIndex === -1) {
        log.warn('Dialog turn not found, cannot set turn todos', { sessionId, turnId });
        return prev;
      }

      const updatedTurns = [...session.dialogTurns];
      updatedTurns[turnIndex] = {
        ...updatedTurns[turnIndex],
        todos: [...todos]
      };

      const updatedSession = {
        ...session,
        dialogTurns: updatedTurns,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public getDialogTurnTodos(sessionId: string, turnId: string): import('../types/flow-chat').TodoItem[] {
    const session = this.state.sessions.get(sessionId);
    if (!session) return [];

    const turn = session.dialogTurns.find(t => t.id === turnId);
    return turn?.todos || [];
  }
  
  public deleteTodo(sessionId: string, todoId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, cannot delete todo', { sessionId, todoId });
        return prev;
      }

      const todos = session.todos || [];
      const updatedTodos = todos.filter(t => t.id !== todoId);

      const updatedSession = {
        ...session,
        todos: updatedTodos,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Get all todo items for session (aggregates todos from all DialogTurns)
   * Mainly used by PlannerPanel to display overall progress
   */
  public getTodos(sessionId: string): import('../types/flow-chat').TodoItem[] {
    const session = this.state.sessions.get(sessionId);
    if (!session) return [];
    
    const allTodos: import('../types/flow-chat').TodoItem[] = [];
    session.dialogTurns.forEach(turn => {
      if (turn.todos && turn.todos.length > 0) {
        allTodos.push(...turn.todos);
      }
    });
    
    if (session.todos && session.todos.length > 0) {
      allTodos.push(...session.todos);
    }
    
    return allTodos;
  }

  public setTodos(sessionId: string, todos: import('../types/flow-chat').TodoItem[]): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        return prev;
      }

      const updatedSession = {
        ...session,
        todos: [...todos],
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }
}

export const flowChatStore = FlowChatStore.getInstance();
