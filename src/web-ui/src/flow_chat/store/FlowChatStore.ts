/**
 * Flow Chat global state store
 * Prevents state loss when components remount
 */

import {
  FlowChatState,
  Session,
  DialogTurn,
  ModelRound,
  FlowItem,
  FlowToolItem,
  FlowImageAnalysisItem,
  ImageAnalysisResult,
  AnyFlowItem,
  SessionConfig,
  SessionContextRestoreState,
  SessionHistoryState,
} from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import {
  isRemoteTraceContext,
  markPhaseAfterAnimationFrames,
  startupTrace,
} from '@/shared/utils/startupTrace';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type { DialogTurnData, LocalCommandMetadata, SessionKind } from '@/shared/types/session-history';
import {
  deriveLastFinishedAtFromMetadata,
  deriveSessionRelationshipFromMetadata,
  isLegacyPersistedBtwSession,
  normalizeSessionRelationship,
} from '../utils/sessionMetadata';
import type { SessionTitleDescriptor } from '../utils/sessionTitle';
import {
  deriveSessionTitleState,
  deriveSessionTitleStateFromMetadata,
  freezeSessionTitleState,
} from '../utils/sessionTitle';
import {
  isTransientToolStatus,
  normalizeRecoveredRoundStatus,
  normalizeRecoveredTextStatus,
  normalizeRecoveredThinkingStatus,
  normalizeRecoveredToolStatus,
  normalizeRecoveredTurnStatus,
  settleInterruptedDialogTurn,
} from '../utils/dialogTurnStability';
import type { WorkspaceInfo } from '@/shared/types';
import { sessionBelongsToWorkspaceNavRow } from '../utils/sessionOrdering';
import { sessionMatchesWorkspace } from '../utils/workspaceScope';

const log = createLogger('FlowChatStore');
const VALID_AGENT_TYPES = new Set(['agentic', 'debug', 'Plan', 'Cowork', 'Claw', 'Team', 'DeepResearch']);
const METADATA_LIST_RECENT_DEDUPE_TTL_MS = 1000;

interface MetadataListRequest {
  promise: Promise<void>;
  completedAtMs?: number;
  cleanupTimer?: ReturnType<typeof setTimeout>;
}

function isUnsupportedTauriCommandError(error: unknown, command: string): boolean {
  const anyError = error as any;
  const originalError = anyError?.context?.originalError;
  const messageParts = [
    anyError?.message,
    typeof originalError === 'string' ? originalError : originalError?.message,
  ].filter((part): part is string => typeof part === 'string');
  const normalizedMessage = messageParts.join(' ').toLowerCase();
  const normalizedCommand = command.toLowerCase();
  const contextCommand =
    typeof anyError?.context?.command === 'string'
      ? anyError.context.command.toLowerCase()
      : '';
  const mentionsCommand =
    contextCommand === normalizedCommand ||
    normalizedMessage.includes(normalizedCommand);

  if (!mentionsCommand) {
    return false;
  }

  return normalizedMessage.includes('unknown command') ||
    normalizedMessage.includes('command not found') ||
    (normalizedMessage.includes('command') && normalizedMessage.includes('not found')) ||
    normalizedMessage.includes('not registered') ||
    normalizedMessage.includes('is not a function');
}

function isValidPersistedAgentType(agentType: string): boolean {
  return VALID_AGENT_TYPES.has(agentType) || agentType.startsWith('acp:');
}

export class FlowChatStore {
  private static instance: FlowChatStore;
  private state: FlowChatState;
  private listeners: Set<(state: FlowChatState) => void> = new Set();
  private silentMode = false;
  private metadataListRequests = new Map<string, MetadataListRequest>();
  private unsupportedRestoreCommands = new Set<string>();
  private onPersistUnreadCompletion?: (sessionId: string, value: 'completed' | 'error' | 'interrupted' | undefined) => void;

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

  private getMetadataListRequestKey(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
  ): string {
    return JSON.stringify([
      workspacePath,
      remoteConnectionId || '',
      remoteSshHost || '',
    ]);
  }

  public setState(updater: (prevState: FlowChatState) => FlowChatState): void {
    const newState = updater(this.state);
    this.state = newState;
    
    if (!this.silentMode) {
      this.listeners.forEach(listener => {
        try {
          listener(newState);
        } catch (error) {
          console.error('[FlowChatStore] Listener threw an error, skipping:', error);
        }
      });
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
    this.listeners.forEach(listener => {
      try {
        listener(this.state);
      } catch (error) {
        console.error('[FlowChatStore] Listener threw an error during notifyListeners, skipping:', error);
      }
    });
  }
  
  public beginSilentMode(): void {
    this.silentMode = true;
  }
  
  public endSilentMode(): void {
    this.silentMode = false;
    this.notifyListeners();
  }

  private collectCascadeSessionIds(
    rootSessionId: string,
    sessions: Map<string, Session>
  ): string[] {
    if (!sessions.has(rootSessionId)) {
      return [];
    }

    const childSessionIdsByParent = new Map<string, string[]>();
    sessions.forEach(session => {
      const parentSessionId = session.parentSessionId;
      if (!parentSessionId) {
        return;
      }

      const existing = childSessionIdsByParent.get(parentSessionId) || [];
      existing.push(session.sessionId);
      childSessionIdsByParent.set(parentSessionId, existing);
    });

    const visited = new Set<string>();
    const orderedSessionIds: string[] = [];

    const visit = (sessionId: string): void => {
      if (visited.has(sessionId)) {
        return;
      }

      visited.add(sessionId);
      const childSessionIds = childSessionIdsByParent.get(sessionId) || [];
      childSessionIds.forEach(childSessionId => {
        visit(childSessionId);
      });
      orderedSessionIds.push(sessionId);
    };

    visit(rootSessionId);
    return orderedSessionIds;
  }

  public getCascadeSessionIds(sessionId: string): string[] {
    return this.collectCascadeSessionIds(sessionId, this.state.sessions);
  }

  public subscribe(listener: (state: FlowChatState) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Register a callback to persist unread completion changes.
   * Called by FlowChatManager during initialization.
   */
  public registerPersistUnreadCompletionCallback(
    callback: (sessionId: string, value: 'completed' | 'error' | 'interrupted' | undefined) => void
  ): void {
    this.onPersistUnreadCompletion = callback;
  }

  public createSession(
    sessionId: string,
    config: SessionConfig,
    _unused?: undefined,
    title?: string,
    maxContextTokens?: number,
    mode?: string,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    titleDescriptor?: SessionTitleDescriptor,
  ): void {
    import('../state-machine').then(({ stateMachineManager }) => {
      stateMachineManager.getOrCreate(sessionId);
    });
    
    this.setState(prev => {
      const relationship = normalizeSessionRelationship({ sessionKind: 'normal' });
      const titleState = deriveSessionTitleState(titleDescriptor);
      const session: Session = {
        sessionId,
        title:
          titleState.title ||
          title ||
          i18nService.t('flow-chat:session.new'),
        titleSource: titleState.titleSource,
        titleI18nKey: titleState.titleI18nKey,
        titleI18nParams: titleState.titleI18nParams,
        titleStatus: undefined,
        dialogTurns: [],
        status: 'idle',
        config,
        createdAt: Date.now(),
        lastActiveAt: Date.now(),
        lastFinishedAt: undefined,
        error: null,
        historyState: 'new',
        maxContextTokens: maxContextTokens || 128128,
        mode: mode || 'agentic',
        workspacePath,
        workspaceId: config.workspaceId,
        remoteConnectionId,
        remoteSshHost,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        btwThreads: [],
        btwOrigin: relationship.btwOrigin,
        isTransient: false,
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

  /**
   * Add a session created externally (e.g., from mobile remote) without switching the active session.
   * workspacePath is stored on the session so the sidebar can filter by current workspace.
   */
  public addExternalSession(
    sessionId: string,
    title: string,
    mode: string,
    workspacePath?: string,
    meta?: {
      parentSessionId?: string;
      sessionKind?: SessionKind;
      btwOrigin?: Session['btwOrigin'];
      isTransient?: boolean;
      agentBackedTransient?: boolean;
      deepReviewRunManifest?: Session['deepReviewRunManifest'];
    },
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): void {
    import('../state-machine').then(({ stateMachineManager }) => {
      stateMachineManager.getOrCreate(sessionId);
    });

    this.setState(prev => {
      if (prev.sessions.has(sessionId)) {
        return prev;
      }

      const relationship = normalizeSessionRelationship(meta);
      const session: Session = {
        sessionId,
        title: title || i18nService.t('flow-chat:session.new'),
        titleSource: 'text',
        titleI18nKey: undefined,
        titleI18nParams: undefined,
        titleStatus: 'generated',
        dialogTurns: [],
        status: 'idle',
        config: { maxContextTokens: 128128, autoCompact: true, enableTools: true } as any,
        createdAt: Date.now(),
        lastActiveAt: Date.now(),
        lastFinishedAt: undefined,
        error: null,
        maxContextTokens: 128128,
        mode: mode || 'agentic',
        isHistorical: false,
        historyState: 'new',
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        btwThreads: [],
        btwOrigin: relationship.btwOrigin,
        deepReviewRunManifest: meta?.deepReviewRunManifest,
        isTransient: meta?.isTransient ?? false,
        agentBackedTransient: meta?.agentBackedTransient ?? false,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, session);

      return {
        ...prev,
        sessions: newSessions,
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

  public updateSessionModelName(sessionId: string, modelName: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const normalizedModelName = modelName.trim() || 'auto';
      if ((session.config.modelName || 'auto') === normalizedModelName) {
        return prev;
      }

      const updatedSession = {
        ...session,
        config: {
          ...session.config,
          modelName: normalizedModelName,
        },
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  /**
   * Update session relationship metadata (parent/child grouping, kind, etc.).
   * This is UI-only and does not affect backend behavior directly.
   */
  public updateSessionRelationship(
    sessionId: string,
    updates: { parentSessionId?: string; sessionKind?: SessionKind }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const relationship = normalizeSessionRelationship({
        sessionKind: updates.sessionKind ?? session.sessionKind,
        parentSessionId: updates.parentSessionId ?? session.parentSessionId,
        btwOrigin: session.btwOrigin,
      });
      const next: Session = {
        ...session,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        btwOrigin: relationship.btwOrigin,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, next);

      return { ...prev, sessions: newSessions };
    });
  }

  public updateSessionBtwOrigin(
    sessionId: string,
    origin: Session['btwOrigin'],
    sessionKind: SessionKind = 'btw'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const relationship = normalizeSessionRelationship({
        sessionKind,
        parentSessionId: origin?.parentSessionId ?? session.parentSessionId,
        btwOrigin: { ...(session.btwOrigin || {}), ...(origin || {}) },
      });
      const next: Session = {
        ...session,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        btwOrigin: relationship.btwOrigin,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, next);
      return { ...prev, sessions: newSessions };
    });
  }

  public addBtwThreadMarker(
    parentSessionId: string,
    marker: {
      requestId: string;
      childSessionId: string;
      title: string;
      status: 'running' | 'done' | 'error';
      createdAt: number;
      parentDialogTurnId?: string;
      parentTurnIndex?: number;
      error?: string;
    }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;

      const existing = session.btwThreads || [];
      if (existing.some(t => t.requestId === marker.requestId)) {
        return prev;
      }

      const nextSession: Session = {
        ...session,
        btwThreads: [marker, ...existing].slice(0, 20),
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, nextSession);
      return { ...prev, sessions: newSessions };
    });
  }

  public updateBtwThreadMarker(
    parentSessionId: string,
    requestId: string,
    updates: Partial<{
      status: 'running' | 'done' | 'error';
      error?: string;
      title: string;
    }>
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;

      const existing = session.btwThreads || [];
      if (existing.length === 0) return prev;

      const nextThreads = existing.map(t => {
        if (t.requestId !== requestId) return t;
        return { ...t, ...updates };
      });

      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, { ...session, btwThreads: nextThreads });
      return { ...prev, sessions: newSessions };
    });
  }

  public removeBtwThreadMarker(parentSessionId: string, requestId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;
      const existing = session.btwThreads || [];
      const nextThreads = existing.filter(t => t.requestId !== requestId);
      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, { ...session, btwThreads: nextThreads });
      return { ...prev, sessions: newSessions };
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
    const sessionIdsToDelete = this.getCascadeSessionIds(sessionId);
    if (sessionIdsToDelete.length === 0) {
      return;
    }

    const { stateMachineManager } = await import('../state-machine');
    sessionIdsToDelete.forEach(id => {
      stateMachineManager.delete(id);
    });

    try {
      const { agentAPI } = await import('@/infrastructure/api');
      const deleteResults = await Promise.allSettled(
        sessionIdsToDelete.map(async id => {
          const sess = this.state.sessions.get(id);
          const workspacePath = sess?.workspacePath;
          if (!workspacePath) {
            throw new Error(`Workspace path not found for session ${id}`);
          }

          await agentAPI.deleteSession(
            id,
            workspacePath,
            sess?.remoteConnectionId,
            sess?.remoteSshHost
          );
        })
      );

      deleteResults.forEach((result, index) => {
        if (result.status === 'rejected') {
          log.error('Failed to delete session on backend', {
            sessionId: sessionIdsToDelete[index],
            error: result.reason,
          });
        }
      });
    } catch (error) {
      log.error('Failed to delete session on backend', { sessionId, error });
    }

    this.removeSession(sessionId);
  }

  public removeSession(sessionId: string): string[] {
    const removedSessionIds = this.getCascadeSessionIds(sessionId);
    if (removedSessionIds.length === 0) {
      return [];
    }

    this.setState(prev => {
      const removedSessionIdSet = new Set(removedSessionIds);
      const newSessions = new Map(prev.sessions);
      const removedSessions = removedSessionIds
        .map(id => prev.sessions.get(id))
        .filter((session): session is Session => Boolean(session));

      removedSessionIds.forEach(id => {
        newSessions.delete(id);
      });

      removedSessions.forEach(session => {
        const parentSessionId = session.btwOrigin?.parentSessionId ?? session.parentSessionId;
        if (!parentSessionId || removedSessionIdSet.has(parentSessionId)) {
          return;
        }

        const parentSession = newSessions.get(parentSessionId);
        if (!parentSession?.btwThreads?.length) {
          return;
        }

        const requestId = session.btwOrigin?.requestId;
        const nextThreads = parentSession.btwThreads.filter(thread => {
          if (thread.childSessionId === session.sessionId) {
            return false;
          }

          if (requestId && thread.requestId === requestId) {
            return false;
          }

          return true;
        });

        if (nextThreads.length !== parentSession.btwThreads.length) {
          newSessions.set(parentSessionId, {
            ...parentSession,
            btwThreads: nextThreads,
          });
        }
      });

      let newActiveSessionId = prev.activeSessionId;
      if (prev.activeSessionId && removedSessionIdSet.has(prev.activeSessionId)) {
        const remainingSessions = Array.from(newSessions.keys());
        newActiveSessionId = remainingSessions.length > 0 ? remainingSessions[0] : null;
      }

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: newActiveSessionId
      };
    });

    return removedSessionIds;
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

  /**
   * Remove sessions bound to a workspace using stable id + host/path scope (never path-only).
   */
  public removeSessionsForWorkspace(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>
  ): string[] {
    const removedSessionIds = Array.from(this.state.sessions.values())
      .filter(session => sessionMatchesWorkspace(session, workspace))
      .map(session => session.sessionId);

    return this.removeSessionsByIds(removedSessionIds);
  }

  public async cancelRunningSessionsForWorkspace(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>
  ): Promise<string[]> {
    const runningSessionIds = Array.from(this.state.sessions.values())
      .filter(session => sessionMatchesWorkspace(session, workspace))
      .filter(session => {
        const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
        return Boolean(
          lastTurn &&
          !['completed', 'cancelled', 'error'].includes(lastTurn.status)
        );
      })
      .map(session => session.sessionId);

    if (runningSessionIds.length === 0) {
      return [];
    }

    const { agentAPI } = await import('@/infrastructure/api');
    await Promise.allSettled(
      runningSessionIds.map(async sessionId => {
        try {
          await agentAPI.cancelSession(sessionId);
        } catch (error) {
          log.warn('Failed to cancel running session before closing workspace', {
            sessionId,
            workspaceId: workspace.id,
            error,
          });
        } finally {
          this.cancelSessionTask(sessionId);
        }
      })
    );

    return runningSessionIds;
  }

  /** @deprecated Prefer `removeSessionsForWorkspace` with full `WorkspaceInfo`. */
  public removeSessionsByWorkspace(
    workspacePath: string,
    remoteConnectionId?: string | null,
    remoteSshHost?: string | null
  ): string[] {
    const removedSessionIds = Array.from(this.state.sessions.values())
      .filter(session =>
        sessionBelongsToWorkspaceNavRow(session, workspacePath, remoteConnectionId, remoteSshHost)
      )
      .map(session => session.sessionId);

    return this.removeSessionsByIds(removedSessionIds);
  }

  private removeSessionsByIds(removedSessionIds: string[]): string[] {

    if (removedSessionIds.length === 0) {
      return [];
    }

    const removedSessionIdSet = new Set(removedSessionIds);

    this.setState(prev => {
      const newSessions = new Map(prev.sessions);
      removedSessionIdSet.forEach(sessionId => {
        newSessions.delete(sessionId);
      });

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId:
          prev.activeSessionId && removedSessionIdSet.has(prev.activeSessionId)
            ? null
            : prev.activeSessionId
      };
    });

    return removedSessionIds;
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

  public addLocalUsageReportTurn(params: {
    sessionId: string;
    markdown: string;
    reportId: string;
    schemaVersion: number;
    generatedAt: number;
    report?: Record<string, any>;
    status?: 'loading' | 'completed';
  }): DialogTurn | null {
    const session = this.state.sessions.get(params.sessionId);
    if (!session) {
      log.warn('Session not found, cannot add local usage report', { sessionId: params.sessionId });
      return null;
    }

    const metadata: LocalCommandMetadata = {
      localCommandKind: 'usage_report',
      reportId: params.reportId,
      schemaVersion: params.schemaVersion,
      generatedAt: params.generatedAt,
      modelVisible: false,
      usageReport: params.report,
      usageReportStatus: params.status ?? 'completed',
    };
    const turnIndex = session.dialogTurns.length;
    const dialogTurn: DialogTurn = {
      id: `local-usage-${params.reportId}`,
      sessionId: params.sessionId,
      kind: 'local_command',
      userMessage: {
        id: `local-usage-user-${params.reportId}`,
        content: params.markdown,
        timestamp: params.generatedAt,
        metadata,
      },
      modelRounds: [],
      status: params.status === 'loading' ? 'processing' : 'completed',
      startTime: params.generatedAt,
      endTime: params.generatedAt,
      backendTurnIndex: turnIndex,
    };

    this.setState(prev => {
      const currentSession = prev.sessions.get(params.sessionId);
      if (!currentSession) return prev;

      if (currentSession.dialogTurns.some(turn => turn.id === dialogTurn.id)) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(params.sessionId, {
        ...currentSession,
        dialogTurns: [...currentSession.dialogTurns, dialogTurn],
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
    return dialogTurn;
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

  public updateDialogTurn(
    sessionId: string,
    dialogTurnId: string,
    updater: (turn: DialogTurn) => DialogTurn,
    options?: { touchActivity?: boolean }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedDialogTurns = session.dialogTurns.map(turn => 
        turn.id === dialogTurnId ? updater(turn) : turn
      );

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastActiveAt: options?.touchActivity === false
          ? session.lastActiveAt
          : Date.now()
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
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      const imageAnalysisItems: FlowImageAnalysisItem[] = imageContexts.map((ctx, index) => ({
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
  }

  /**
   * Update image analysis results
   */
  public updateImageAnalysisResults(
    sessionId: string,
    dialogTurnId: string,
    results: ImageAnalysisResult[]
  ): void {
      this.updateDialogTurn(sessionId, dialogTurnId, turn => {
        if (!turn.imageAnalysisPhase) {
          log.warn('Attempting to update non-existent image analysis phase', { sessionId, dialogTurnId });
          return turn;
        }

      const updatedItems: FlowImageAnalysisItem[] = turn.imageAnalysisPhase.items.map(item => {
        const result = results.find(r => r.image_id === item.imageContext.id);
        if (result) {
          return {
            ...item,
            result,
            status: 'completed' as const,
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
          return update ? ({ ...item, ...update.changes } as AnyFlowItem) : item;
        })
      }));
      
      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  public addModelRoundItem(sessionId: string, dialogTurnId: string, item: AnyFlowItem, modelRoundId?: string): void {
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
  public addModelRoundItemSilent(sessionId: string, dialogTurnId: string, item: AnyFlowItem, modelRoundId?: string): void {
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
  public insertModelRoundItemAfterTool(sessionId: string, dialogTurnId: string, parentToolId: string, newItem: AnyFlowItem): void {
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
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let updated = false;
      
      const updatedModelRounds = turn.modelRounds.map(modelRound => {
        if (updated) return modelRound;
        
        const updatedItems = modelRound.items.map((item: any) => {
          const toolCallId =
            item.type === 'tool' ? ((item as FlowToolItem).toolCall?.id as string | undefined) : undefined;
          const idMatches = item.id === itemId || (toolCallId !== undefined && toolCallId === itemId);
          if (idMatches) {
            const updatedItem = { ...item, ...updates };
            return updatedItem;
          }
          return item;
        });
        
        if (updatedItems.some((item: any) => {
          const tc = item.type === 'tool' ? (item as FlowToolItem).toolCall?.id : undefined;
          return item.id === itemId || tc === itemId;
        })) {
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
      const item = modelRound.items.find((item: any) => {
        if (item.id === toolUseId) return true;
        if (item.type === 'tool') {
          const ti = item as FlowToolItem;
          return ti.toolCall?.id === toolUseId;
        }
        return false;
      });
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

  public rollbackTokenUsage(): void {
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

  public markSessionFinished(sessionId: string, timestamp: number = Date.now()): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        lastActiveAt: timestamp,
        lastFinishedAt: timestamp,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public markSessionUnreadCompletion(
    sessionId: string,
    completionKind: 'completed' | 'error' | 'interrupted'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        hasUnreadCompletion: completionKind,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return { ...prev, sessions: newSessions };
    });
    this.onPersistUnreadCompletion?.(sessionId, completionKind);
  }

  public clearSessionUnreadCompletion(sessionId: string): void {
    let didClear = false;
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || !session.hasUnreadCompletion) return prev;

      const updatedSession: Session = {
        ...session,
        hasUnreadCompletion: undefined,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      didClear = true;
      return { ...prev, sessions: newSessions };
    });
    if (didClear) {
      this.onPersistUnreadCompletion?.(sessionId, undefined);
    }
  }

  public setSessionNeedsAttention(
    sessionId: string,
    attentionKind: 'ask_user' | 'tool_confirm'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        needsUserAttention: attentionKind,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return { ...prev, sessions: newSessions };
    });
    this.onPersistUnreadCompletion?.(sessionId, undefined);
  }

  public clearSessionNeedsAttention(sessionId: string): void {
    let didClear = false;
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || !session.needsUserAttention) return prev;

      const updatedSession: Session = {
        ...session,
        needsUserAttention: undefined,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      didClear = true;
      return { ...prev, sessions: newSessions };
    });
    if (didClear) {
      this.onPersistUnreadCompletion?.(sessionId, undefined);
    }
  }

  public async updateSessionTitle(
    sessionId: string,
    title: string,
    status: 'generating' | 'generated' | 'failed'
  ): Promise<void> {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      // As soon as the user meaningfully interacts with the session we freeze the
      // title to text, so later locale changes do not rewrite real conversation names.
      const nextTitleState = freezeSessionTitleState(title);
      const updatedSession = {
        ...session,
        ...nextTitleState,
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

      const settledAt = Date.now();
      const updatedDialogTurns = session.dialogTurns.map((turn, index) =>
        index === session.dialogTurns.length - 1
          ? settleInterruptedDialogTurn(turn, settledAt)
          : turn
      );

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
      const { sessionAPI } = await import('@/infrastructure/api');
      const session = this.state.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, skipping save', { sessionId, turnId });
        return;
      }
      if (session.isTransient) {
        return;
      }

      const workspacePath = session.workspacePath;
      if (!workspacePath) {
        log.warn('Workspace path not available, skipping save', { sessionId, turnId });
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
        kind: dialogTurn.kind || 'user_dialog',
        userMessage: {
          id: dialogTurn.userMessage.id,
          content: dialogTurn.userMessage.content,
          timestamp: dialogTurn.userMessage.timestamp,
          metadata: dialogTurn.userMessage.metadata,
        },
        modelRounds: dialogTurn.modelRounds.map((round, roundIndex) => {
          const textItems = round.items
            .filter(item => item.type === 'text' && !(item as any).runtimeStatus)
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              timestamp: item.timestamp,
              status: item.status,
            }));
          
          const toolItems = round.items
            .filter(item => item.type === 'tool')
            .map(item => ({
              id: item.id,
              toolName: (item as any).toolName || '',
              interruptionReason: (item as any).interruptionReason,
              toolCall: (item as any).toolCall || { input: {}, id: item.id },
              toolResult: (item as any).toolResult,
              aiIntent: (item as any).aiIntent,
              startTime: (item as any).startTime || item.timestamp,
              endTime: (item as any).endTime,
              status: item.status,
              durationMs: (item as any).durationMs ?? ((item as any).endTime
                ? (item as any).endTime - (item as any).startTime
                : undefined),
              queueWaitMs: (item as any).queueWaitMs,
              preflightMs: (item as any).preflightMs,
              confirmationWaitMs: (item as any).confirmationWaitMs,
              executionMs: (item as any).executionMs,
            }));
          
          const thinkingItems = round.items
            .filter(item => item.type === 'thinking')
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              isCollapsed: (item as any).isCollapsed || false,
              timestamp: item.timestamp,
              status: item.status,
            }));
          
          return {
            id: round.id,
            turnId,
            roundIndex,
            timestamp: round.startTime,
            renderHints: round.renderHints,
            textItems,
            toolItems,
            thinkingItems,
            startTime: round.startTime,
            endTime: round.endTime || Date.now(),
            durationMs: round.durationMs,
            providerId: round.providerId,
            modelId: round.modelId,
            modelAlias: round.modelAlias,
            firstChunkMs: round.firstChunkMs,
            firstVisibleOutputMs: round.firstVisibleOutputMs,
            streamDurationMs: round.streamDurationMs,
            attemptCount: round.attemptCount,
            failureCategory: round.failureCategory,
            tokenDetails: round.tokenDetails,
            status: round.status
          };
        }),
        startTime: dialogTurn.startTime,
        endTime: dialogTurn.endTime || Date.now(),
        durationMs: (dialogTurn.endTime || Date.now()) - dialogTurn.startTime,
        status: 'cancelled' as const
      };

      await sessionAPI.saveSessionTurn(
        turnData,
        workspacePath,
        session.remoteConnectionId,
        session.remoteSshHost
      );
    } catch (error) {
      log.error('Failed to save cancelled dialog turn', { sessionId, turnId, error });
    }
  }


  /**
   * Initialize by loading persisted session metadata from disk
   * Clears sessions from other workspaces, then loads sessions for the target workspace.
   */
  public async initializeFromDisk(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<void> {
    const requestKey = this.getMetadataListRequestKey(workspacePath, remoteConnectionId, remoteSshHost);
    const existingRequest = this.metadataListRequests.get(requestKey);
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    if (existingRequest) {
      const completedAtMs = existingRequest.completedAtMs;
      const isRecentCompletedRequest =
        completedAtMs !== undefined &&
        elapsedMs(completedAtMs) <= METADATA_LIST_RECENT_DEDUPE_TTL_MS;

      if (completedAtMs === undefined || isRecentCompletedRequest) {
        startupTrace.markPhase('session_metadata_list_deduped', {
          remote,
          source: traceSource,
          dedupeState: completedAtMs === undefined ? 'in-flight' : 'recent',
        });
        return existingRequest.promise;
      }

      if (existingRequest.cleanupTimer) {
        clearTimeout(existingRequest.cleanupTimer);
      }
      this.metadataListRequests.delete(requestKey);
    }

    let succeeded = false;
    const loadPromise = this.initializeFromDiskUncached(
      workspacePath,
      remoteConnectionId,
      remoteSshHost,
      traceSource,
    ).then(result => {
      succeeded = result;
    });

    const request: MetadataListRequest = { promise: loadPromise };
    this.metadataListRequests.set(requestKey, request);

    loadPromise.finally(() => {
      const currentRequest = this.metadataListRequests.get(requestKey);
      if (currentRequest !== request) {
        return;
      }

      if (!succeeded) {
        this.metadataListRequests.delete(requestKey);
        return;
      }

      request.completedAtMs = nowMs();
      request.cleanupTimer = setTimeout(() => {
        if (this.metadataListRequests.get(requestKey) === request) {
          this.metadataListRequests.delete(requestKey);
        }
      }, METADATA_LIST_RECENT_DEDUPE_TTL_MS);
    });

    return loadPromise;
  }

  private async initializeFromDiskUncached(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<boolean> {
    const traceStartedAt = nowMs();
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    const metadataListTraceId = `metadata-${Math.random().toString(36).slice(2, 8)}`;
    let sessionCount = 0;
    startupTrace.markPhase('session_metadata_list_start', {
      remote,
      source: traceSource,
      metadataListTraceId,
    });
    try {
      const { sessionAPI } = await import('@/infrastructure/api');
      const sessions = await sessionAPI.listSessions(workspacePath, remoteConnectionId, remoteSshHost);
      sessionCount = sessions.length;
      startupTrace.markPhase('session_metadata_list_loaded', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
      });

      const { stateMachineManager } = await import('../state-machine');

      let models: any[] = [];
      let defaultModels: Record<string, string> = {};
      try {
        const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
        const [modelsResult, defaultModelsResult] = await Promise.allSettled([
          configManager.getConfig<any[]>('ai.models'),
          configManager.getConfig<Record<string, string>>('ai.default_models'),
        ]);

        if (modelsResult.status === 'fulfilled' && Array.isArray(modelsResult.value)) {
          models = modelsResult.value;
        }
        if (
          defaultModelsResult.status === 'fulfilled' &&
          defaultModelsResult.value &&
          typeof defaultModelsResult.value === 'object'
        ) {
          defaultModels = defaultModelsResult.value;
        }
      } catch (error) {
        log.warn('Failed to load model config for session metadata, using defaults', { error });
      }

      const processSession = async (metadata: any) => {
        try {
          const existingSession = this.state.sessions.get(metadata.sessionId);
          if (existingSession) {
            return;
          }
          if (isLegacyPersistedBtwSession(metadata)) {
            return;
          }

          stateMachineManager.getOrCreate(metadata.sessionId);

          let maxContextTokens = 128128;
          if (metadata.modelName) {
            const model = models.find((m: any) => m.name === metadata.modelName || m.id === metadata.modelName);
            if (model?.context_window) {
              maxContextTokens = model.context_window;
            }
          }

          if (maxContextTokens === 128128) {
            const primaryModelId = defaultModels?.primary;

            if (primaryModelId) {
              const primaryModel = models.find((m: any) => m.id === primaryModelId);
              if (primaryModel?.context_window) {
                maxContextTokens = primaryModel.context_window;
              }
            }
          }

          const relationship = deriveSessionRelationshipFromMetadata(metadata);
          const lastFinishedAt = deriveLastFinishedAtFromMetadata(metadata);
          const titleState = deriveSessionTitleStateFromMetadata(metadata);
          const hasDynamicDefaultTitle = titleState.titleSource === 'i18n';

          this.setState(prev => {
            if (prev.sessions.has(metadata.sessionId)) {
              return prev;
            }

            const rawAgentType = metadata.agentType || 'agentic';
            const validatedAgentType = isValidPersistedAgentType(rawAgentType) ? rawAgentType : 'agentic';

            if (rawAgentType !== validatedAgentType) {
              log.warn('Invalid agentType, falling back to agentic', { sessionId: metadata.sessionId, rawAgentType, validatedAgentType });
            }

            const session: Session = {
              sessionId: metadata.sessionId,
              title: titleState.title,
              titleSource: titleState.titleSource,
              titleI18nKey: titleState.titleI18nKey,
              titleI18nParams: titleState.titleI18nParams,
              titleStatus: hasDynamicDefaultTitle ? undefined : 'generated',
              dialogTurns: [],
              status: 'idle',
              config: {
                agentType: validatedAgentType,
                modelName: metadata.modelName,
              },
              createdAt: metadata.createdAt,
              lastActiveAt: metadata.lastActiveAt,
              lastFinishedAt,
              error: null,
              isHistorical: true,
              historyState: 'metadata-only',
              todos: (metadata as any).todos || [],
              maxContextTokens,
              mode: validatedAgentType,
              workspacePath: (metadata as any).workspacePath || workspacePath,
              remoteConnectionId: metadata.remoteConnectionId || remoteConnectionId,
              remoteSshHost:
                metadata.remoteSshHost || metadata.workspaceHostname || remoteSshHost,
              parentSessionId: relationship.parentSessionId,
              sessionKind: relationship.sessionKind,
              btwThreads: [],
              btwOrigin: relationship.btwOrigin,
              hasUnreadCompletion: metadata.unreadCompletion,
              needsUserAttention: metadata.needsUserAttention,
              deepReviewRunManifest: metadata.deepReviewRunManifest,
              isTransient: false,
            };

            const newSessions = new Map(prev.sessions);
            newSessions.set(metadata.sessionId, session);

            return {
              ...prev,
              sessions: newSessions,
            };
          });
        } catch (error) {
          log.warn('Failed to process persisted session metadata', {
            sessionId: metadata?.sessionId,
            error,
          });
        }
      };
      
      await Promise.all(sessions.map(processSession));
      startupTrace.markPhase('session_metadata_list_end', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
        durationMs: elapsedMs(traceStartedAt),
      });
      return true;
    } catch (error) {
      startupTrace.markPhase('session_metadata_list_failed', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
        durationMs: elapsedMs(traceStartedAt),
      });
      log.error('Failed to load persisted sessions', error);
      return false;
    }
  }

  public setSessionHistoryState(sessionId: string, historyState: SessionHistoryState): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || session.historyState === historyState) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        historyState,
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public setSessionContextRestoreState(
    sessionId: string,
    contextRestoreState: SessionContextRestoreState
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || session.contextRestoreState === contextRestoreState) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        contextRestoreState,
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  /**
   * Lazy load session history (convert historical data to FlowChat format)
   */
  public async loadSessionHistory(
    sessionId: string,
    workspacePath: string,
    limit?: number,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    const traceStartedAt = nowMs();
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    const sessionTraceId = `${sessionId.slice(0, 8)}-${Math.random().toString(36).slice(2, 8)}`;
    startupTrace.markPhase('historical_session_hydrate_start', { remote, sessionTraceId });
    this.setSessionHistoryState(sessionId, 'hydrating');
    try {
      const { stateMachineManager } = await import('../state-machine');
      stateMachineManager.getOrCreate(sessionId);
      
      const existingSession = this.state.sessions.get(sessionId);
      const isAcpSession = existingSession?.mode?.startsWith('acp:') ||
        existingSession?.config.agentType?.startsWith('acp:');
      let turns: DialogTurnData[] | undefined;
      let contextRestoreState: SessionContextRestoreState = 'ready';
      if (!isAcpSession) {
        const restoreStartedAt = nowMs();
        startupTrace.markPhase('historical_session_restore_start', { remote, sessionTraceId });
        try {
          const { agentAPI } = await import('@/infrastructure/api');
          const restoreWithTurnsOrSession = async (): Promise<void> => {
            if (
              typeof agentAPI.restoreSessionWithTurns === 'function' &&
              !this.unsupportedRestoreCommands.has('restore_session_with_turns')
            ) {
              try {
                const restored = await agentAPI.restoreSessionWithTurns(
                  sessionId,
                  workspacePath,
                  remoteConnectionId,
                  remoteSshHost,
                  sessionTraceId
                );
                turns = restored.turns;
                contextRestoreState = 'ready';
                return;
              } catch (error) {
                if (!isUnsupportedTauriCommandError(error, 'restore_session_with_turns')) {
                  throw error;
                }
                this.unsupportedRestoreCommands.add('restore_session_with_turns');
                startupTrace.markPhase('historical_session_restore_fallback', {
                  remote,
                  sessionTraceId,
                  from: 'restore_session_with_turns',
                  to: 'restore_session',
                  reason: 'unsupported-command',
                });
              }
            }

            await agentAPI.restoreSession(
              sessionId,
              workspacePath,
              remoteConnectionId,
              remoteSshHost,
              sessionTraceId
            );
            contextRestoreState = 'ready';
          };

          if (
            typeof agentAPI.restoreSessionView === 'function' &&
            !this.unsupportedRestoreCommands.has('restore_session_view')
          ) {
            try {
              const restored = await agentAPI.restoreSessionView(
                sessionId,
                workspacePath,
                remoteConnectionId,
                remoteSshHost,
                sessionTraceId
              );
              turns = restored.turns;
              contextRestoreState =
                restored.contextRestoreState === 'ready' ? 'ready' : 'pending';
            } catch (error) {
              if (!isUnsupportedTauriCommandError(error, 'restore_session_view')) {
                throw error;
              }
              this.unsupportedRestoreCommands.add('restore_session_view');
              startupTrace.markPhase('historical_session_restore_fallback', {
                remote,
                sessionTraceId,
                from: 'restore_session_view',
                to: 'restore_session_with_turns',
                reason: 'unsupported-command',
              });
              await restoreWithTurnsOrSession();
            }
          } else {
            await restoreWithTurnsOrSession();
          }
          startupTrace.markPhase('historical_session_restore_end', {
            remote,
            sessionTraceId,
            turnCount: Array.isArray(turns) ? turns.length : 0,
            contextRestoreState,
            durationMs: elapsedMs(restoreStartedAt),
          });
        } catch (error) {
          contextRestoreState = 'pending';
          startupTrace.markPhase('historical_session_restore_failed', {
            remote,
            sessionTraceId,
            durationMs: elapsedMs(restoreStartedAt),
          });
          log.warn('Backend session restore failed (may be new session)', { sessionId, error });
        }
      }
      
      if (!turns) {
        const turnsLoadStartedAt = nowMs();
        startupTrace.markPhase('historical_session_turns_load_start', { remote, sessionTraceId });
        const { sessionAPI } = await import('@/infrastructure/api');
        turns = await sessionAPI.loadSessionTurns(
          sessionId,
          workspacePath,
          limit,
          remoteConnectionId,
          remoteSshHost
        );
        startupTrace.markPhase('historical_session_turns_load_end', {
          remote,
          sessionTraceId,
          turnCount: Array.isArray(turns) ? turns.length : 0,
          durationMs: elapsedMs(turnsLoadStartedAt),
        });
      }
      startupTrace.markPhase('historical_session_turns_loaded', {
        remote,
        sessionTraceId,
        turnCount: Array.isArray(turns) ? turns.length : 0,
      });
      
      const convertStartedAt = nowMs();
      const dialogTurns = this.convertToDialogTurns(turns);
      startupTrace.markPhase('historical_session_convert_end', {
        remote,
        sessionTraceId,
        turnCount: dialogTurns.length,
        durationMs: elapsedMs(convertStartedAt),
      });
      
      const stateCommitStartedAt = nowMs();
      this.setState(prev => {
        const session = prev.sessions.get(sessionId);
        if (!session) return prev;
        
        const updatedSession = {
          ...session,
          dialogTurns,
          isHistorical: false,
          historyState: 'ready' as const,
          contextRestoreState,
          error: null,
        };
        
        const newSessions = new Map(prev.sessions);
        newSessions.set(sessionId, updatedSession);
        
        return {
          ...prev,
          sessions: newSessions,
        };
      });
      startupTrace.markPhase('historical_session_state_commit_end', {
        remote,
        sessionTraceId,
        turnCount: dialogTurns.length,
        durationMs: elapsedMs(stateCommitStartedAt),
      });
      markPhaseAfterAnimationFrames(startupTrace, 'historical_session_after_state_commit_frame', {
        remote,
        sessionTraceId,
        turnCount: dialogTurns.length,
      }, {
        frameCount: 2,
      });
      
      // Reset state machine to IDLE after loading history
      // This handles the case where restoreSession triggered events that left the state machine in PROCESSING
      stateMachineManager.reset(sessionId);
      startupTrace.markPhase('historical_session_hydrate_end', {
        remote,
        sessionTraceId,
        turnCount: dialogTurns.length,
        durationMs: elapsedMs(traceStartedAt),
      });
    } catch (error) {
      this.setState(prev => {
        const session = prev.sessions.get(sessionId);
        if (!session) return prev;

        const newSessions = new Map(prev.sessions);
        newSessions.set(sessionId, {
          ...session,
          isHistorical: true,
          historyState: 'failed',
        });

        return {
          ...prev,
          sessions: newSessions,
        };
      });
      startupTrace.markPhase('historical_session_hydrate_failed', {
        remote,
        sessionTraceId,
        durationMs: elapsedMs(traceStartedAt),
      });
      log.error('Failed to load session history', { sessionId, error });
      throw error;
    }
  }

  /**
   * Strip agent-internal XML wrapper tags from persisted user inputs.
   */
  private cleanRemoteUserInput(raw: string): string {
    const s = raw.trim();
    const userQueryMatch = s.match(/<user_query>\s*([\s\S]*?)\s*<\/user_query>/);
    if (userQueryMatch) {
      return userQueryMatch[1].trim();
    }

    return s
      .replace(/<system(?:_|-)reminder>[\s\S]*?<\/system(?:_|-)reminder>/g, '')
      .trim();
  }

  /**
   * Convert DialogTurnData to FlowChat DialogTurn format
   */
  private convertToDialogTurns(turns: any[]): DialogTurn[] {
    return turns.map(turn => {
      const metadata = turn.userMessage.metadata;
      const metaImages = metadata?.images;
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

      const displayContent =
        metadata?.localCommandKind === 'usage_report'
          ? turn.userMessage.content
          : metadata?.original_text || this.cleanRemoteUserInput(turn.userMessage.content);
      const normalizedTurnStatus = normalizeRecoveredTurnStatus(turn.status, { error: undefined });

      return {
      id: turn.turnId,
      sessionId: turn.sessionId,
      kind: turn.kind || 'user_dialog',
      userMessage: {
        id: turn.userMessage.id,
        type: 'user' as const,
        content: displayContent,
        timestamp: turn.userMessage.timestamp,
        hasImages,
        metadata,
        images,
      },
      modelRounds: turn.modelRounds.map((round: any) => {
        const normalizedRoundStatus = normalizeRecoveredRoundStatus(round.status, normalizedTurnStatus);

        return {
          id: round.id,
          turnId: round.turnId,
          index: round.roundIndex ?? 0,
          renderHints: round.renderHints,
          items: [
            ...round.textItems.map((text: any) => ({
              id: text.id,
              type: 'text' as const,
              content: text.content,
              isStreaming: false,
              isMarkdown: text.isMarkdown !== undefined ? text.isMarkdown : true,
              timestamp: text.timestamp,
              status: normalizeRecoveredTextStatus(text.status, normalizedTurnStatus),
              orderIndex: text.orderIndex,
              isSubagentItem: text.isSubagentItem,
              parentTaskToolId: text.parentTaskToolId,
              subagentSessionId: text.subagentSessionId,
            })),
            ...round.toolItems.map((tool: any) => ({
              id: tool.id,
              type: 'tool' as const,
              toolName: tool.toolName,
              interruptionReason:
                tool.interruptionReason === 'app_restart'
                  ? 'app_restart'
                  : isTransientToolStatus(tool.status)
                    ? 'app_restart'
                    : undefined,
              toolCall: tool.toolCall,
              toolResult: tool.toolResult,
              aiIntent: tool.aiIntent,
              requiresConfirmation: tool.requiresConfirmation,
              userConfirmed: tool.userConfirmed,
              acpPermission: tool.acpPermission,
              startTime: tool.startTime,
              endTime: tool.endTime,
              durationMs: tool.durationMs,
              queueWaitMs: tool.queueWaitMs,
              preflightMs: tool.preflightMs,
              confirmationWaitMs: tool.confirmationWaitMs,
              executionMs: tool.executionMs,
              timestamp: tool.startTime,
              status: normalizeRecoveredToolStatus(
                tool.status,
                normalizedTurnStatus,
                tool.toolResult,
                { preservePendingConfirmation: true },
              ),
              orderIndex: tool.orderIndex,
              isSubagentItem: tool.isSubagentItem,
              parentTaskToolId: tool.parentTaskToolId,
              subagentSessionId: tool.subagentSessionId,
              subagentModelId: tool.subagentModelId,
              subagentModelAlias: tool.subagentModelAlias,
            })),
            ...(round.thinkingItems || []).map((thinking: any) => ({
              id: thinking.id,
              type: 'thinking' as const,
              content: thinking.content,
              isStreaming: false,
              isCollapsed: thinking.isCollapsed ?? true,
              timestamp: thinking.timestamp,
              status: normalizeRecoveredThinkingStatus(thinking.status, normalizedTurnStatus),
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
          isStreaming: false,
          isComplete: normalizedRoundStatus !== 'pending' && normalizedRoundStatus !== 'streaming',
          status: normalizedRoundStatus,
          startTime: round.startTime ?? round.timestamp,
          endTime: round.endTime,
          durationMs: round.durationMs,
          providerId: round.providerId,
          modelId: round.modelId,
          modelAlias: round.modelAlias,
          firstChunkMs: round.firstChunkMs,
          firstVisibleOutputMs: round.firstVisibleOutputMs,
          streamDurationMs: round.streamDurationMs,
          attemptCount: round.attemptCount,
          failureCategory: round.failureCategory,
          tokenDetails: round.tokenDetails,
          timestamp: round.timestamp,
        };
      }),
      timestamp: turn.timestamp,
      status: normalizedTurnStatus,
      startTime: turn.startTime,
      endTime: turn.endTime,
      backendTurnIndex: turn.turnIndex,
    };
    });
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
