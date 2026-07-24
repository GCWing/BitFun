import {
  CreateSessionOptions,
  RemoteSession,
  SessionListResult,
  SessionSummary
} from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';
import { RemoteUiState } from './RemoteUiState';

export interface RemoteSessionClient {
  listSessions(limit: number, offset: number, query: string, agentType: string): Promise<SessionListResult>;
  createSession(options: CreateSessionOptions): Promise<SessionSummary>;
  deleteSession(sessionId: string): Promise<void>;
}

export interface RemoteSessionControllerCallbacks {
  onSessions: (sessions: RemoteSession[], hasMore: boolean) => void;
  onActiveSession: (session: SessionSummary) => void;
  onStatusText: (statusText: string) => void;
  onBusy: (isBusy: boolean) => void;
  onLoading: (isLoading: boolean) => void;
  onSessionError: (errorText: string) => void;
  onReconnecting: () => void;
  onConnected: () => void;
  onConnectionFailed: (err: Object) => void;
  onStartHeartbeat: () => void;
}

export class RemoteSessionController {
  private readonly client: RemoteSessionClient;
  private readonly pageSize: number;
  private readonly callbacks: RemoteSessionControllerCallbacks;
  private sessions: RemoteSession[] = [];
  private hasMore: boolean = false;
  private offset: number = 0;

  constructor(
    client: RemoteSessionClient,
    pageSize: number,
    callbacks: RemoteSessionControllerCallbacks
  ) {
    this.client = client;
    this.pageSize = pageSize;
    this.callbacks = callbacks;
  }

  setSessions(sessions: RemoteSession[], hasMore: boolean): void {
    this.sessions = sessions.slice();
    this.hasMore = hasMore;
    this.offset = this.sessions.length;
    this.callbacks.onSessions(this.sessions, this.hasMore);
  }

  clearSessions(): void {
    this.sessions = [];
    this.hasMore = false;
    this.offset = 0;
    this.callbacks.onSessions(this.sessions, this.hasMore);
  }

  updateSessionTitle(sessionId: string, title: string): void {
    this.sessions = RemoteUiState.updateSessionTitle(this.sessions, sessionId, title);
    this.offset = this.sessions.length;
    this.callbacks.onSessions(this.sessions, this.hasMore);
  }

  async refresh(
    query: string,
    filter: string,
    remoteAvailable: boolean,
    isConnected: boolean
  ): Promise<void> {
    if (!remoteAvailable) {
      return;
    }
    try {
      if (!isConnected) {
        this.callbacks.onReconnecting();
      }
      this.callbacks.onStatusText(RemoteI18n.t('status.refreshSessions'));
      this.callbacks.onLoading(true);
      this.offset = 0;
      const result = await this.client.listSessions(this.pageSize, this.offset, query, filter);
      this.sessions = result.sessions;
      this.hasMore = result.hasMore;
      this.offset = this.sessions.length;
      this.callbacks.onSessions(this.sessions, this.hasMore);
      this.callbacks.onConnected();
      this.callbacks.onStatusText(RemoteI18n.t('connection.connected'));
      this.callbacks.onStartHeartbeat();
    } catch (err) {
      this.callbacks.onSessionError(ConnectionErrorPolicy.errorText(err));
      this.callbacks.onConnectionFailed(err);
    } finally {
      this.callbacks.onLoading(false);
    }
  }

  async loadMore(
    query: string,
    filter: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    if (isBusy || !this.hasMore || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onLoading(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.loadMoreSessions'));
      const result = await this.client.listSessions(this.pageSize, this.offset, query, filter);
      this.sessions = RemoteUiState.mergeSessions(this.sessions, result.sessions);
      this.hasMore = result.hasMore;
      this.offset = this.sessions.length;
      this.callbacks.onSessions(this.sessions, this.hasMore);
      this.callbacks.onStatusText(RemoteI18n.t('status.sessionsUpdated'));
    } catch (err) {
      this.callbacks.onSessionError(ConnectionErrorPolicy.errorText(err));
      this.callbacks.onConnectionFailed(err);
    } finally {
      this.callbacks.onBusy(false);
      this.callbacks.onLoading(false);
    }
  }

  async create(
    agentType: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    onCreated: (session: SessionSummary) => Promise<void>
  ): Promise<void> {
    if (isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.createSession'));
      const session = await this.client.createSession({
        agentType,
        title: '',
        instruction: ''
      });
      this.callbacks.onActiveSession(session);
      this.callbacks.onStatusText(RemoteI18n.t('status.sessionCreated'));
      await onCreated(session);
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async open(
    item: RemoteSession,
    workspacePath: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    onOpened: (session: SessionSummary) => Promise<void>
  ): Promise<void> {
    if (isBusy || item.id.length === 0 || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.loadMessages'));
      const session: SessionSummary = {
        sessionId: item.id,
        title: item.title,
        workspacePath,
        agentType: item.agentType
      };
      this.callbacks.onActiveSession(session);
      await onOpened(session);
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async delete(
    item: RemoteSession,
    activeSessionId: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    query: string,
    filter: string,
    onDeletedActive: () => Promise<void>
  ): Promise<void> {
    if (isBusy || item.id.length === 0 || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.deletingSession'));
      await this.client.deleteSession(item.id);
      this.sessions = this.sessions.filter((session: RemoteSession) => session.id !== item.id);
      this.offset = this.sessions.length;
      this.callbacks.onSessions(this.sessions, this.hasMore);
      if (activeSessionId === item.id) {
        await onDeletedActive();
      }
      this.callbacks.onStatusText(RemoteI18n.t('status.sessionDeleted'));
      if (this.hasMore && this.sessions.length < this.pageSize) {
        this.callbacks.onBusy(false);
        await this.loadMore(query, filter, false, remoteAvailable);
      }
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }
}
