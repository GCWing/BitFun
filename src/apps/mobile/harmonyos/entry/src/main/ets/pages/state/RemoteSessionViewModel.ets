import { RemoteSession, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import { RemoteModelController } from '../../services/RemoteModelController';
import { RemoteSessionController } from '../../services/RemoteSessionController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { RemotePageState } from './RemotePageState';

export class RemoteSessionViewModelHooks {
  readonly remoteAvailable: () => boolean;
  readonly isConnected: () => boolean;
  readonly isBusy: () => boolean;
  readonly onBusy: (busy: boolean) => void;
  readonly onRouteChat: (sessionId: string) => void;
  readonly onRouteHome: () => void;
  readonly onStopPolling: () => void;
  readonly onStartPolling: () => void;
  readonly onResetTimeline: (sessionId: string) => void;
  readonly onClearRemoteFiles: () => void;
  readonly onKnownStateReset: () => void;
  readonly onLoadModelCatalog: (sessionId: string) => Promise<void>;
  readonly onLoadActiveMessages: () => Promise<void>;
  readonly onRefreshSessions: () => Promise<void>;
  readonly onSelectWorkspace: (path: string) => Promise<void>;

  constructor(
    remoteAvailable: () => boolean,
    isConnected: () => boolean,
    isBusy: () => boolean,
    onBusy: (busy: boolean) => void,
    onRouteChat: (sessionId: string) => void,
    onRouteHome: () => void,
    onStopPolling: () => void,
    onStartPolling: () => void,
    onResetTimeline: (sessionId: string) => void,
    onClearRemoteFiles: () => void,
    onKnownStateReset: () => void,
    onLoadModelCatalog: (sessionId: string) => Promise<void>,
    onLoadActiveMessages: () => Promise<void>,
    onRefreshSessions: () => Promise<void>,
    onSelectWorkspace: (path: string) => Promise<void>
  ) {
    this.remoteAvailable = remoteAvailable;
    this.isConnected = isConnected;
    this.isBusy = isBusy;
    this.onBusy = onBusy;
    this.onRouteChat = onRouteChat;
    this.onRouteHome = onRouteHome;
    this.onStopPolling = onStopPolling;
    this.onStartPolling = onStartPolling;
    this.onResetTimeline = onResetTimeline;
    this.onClearRemoteFiles = onClearRemoteFiles;
    this.onKnownStateReset = onKnownStateReset;
    this.onLoadModelCatalog = onLoadModelCatalog;
    this.onLoadActiveMessages = onLoadActiveMessages;
    this.onRefreshSessions = onRefreshSessions;
    this.onSelectWorkspace = onSelectWorkspace;
  }
}

/** Owns remote session commands and their page lifecycle effects. */
export class RemoteSessionViewModel {
  private readonly pageState: RemotePageState;
  private readonly sessions: RemoteSessionController;
  private readonly chat: RemoteChatCommandController;
  private readonly models: RemoteModelController;
  private readonly files: RemoteFileDownloadController;
  private readonly hooks: RemoteSessionViewModelHooks;

  constructor(
    pageState: RemotePageState,
    sessions: RemoteSessionController,
    chat: RemoteChatCommandController,
    models: RemoteModelController,
    files: RemoteFileDownloadController,
    hooks: RemoteSessionViewModelHooks
  ) {
    this.pageState = pageState;
    this.sessions = sessions;
    this.chat = chat;
    this.models = models;
    this.files = files;
    this.hooks = hooks;
  }

  async refreshSessions(): Promise<void> {
    await this.sessions.refresh(
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      this.hooks.remoteAvailable(),
      this.hooks.isConnected()
    );
  }

  async loadMoreSessions(): Promise<void> {
    await this.sessions.loadMore(
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable()
    );
  }

  async setFilter(filter: string): Promise<void> {
    if (this.pageState.sessionFilter === filter) {
      return;
    }
    this.pageState.setFilter(filter);
    await this.refreshSessions();
  }

  async createSession(
    agentType: string,
    instruction: string = '',
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat,
    modelId: string = ''
  ): Promise<void> {
    await this.sessions.create(
      agentType,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable(),
      async (session: SessionSummary): Promise<void> => {
        this.pageState.clearComposer();
        this.pageState.setHasMoreMessages(false);
        this.hooks.onKnownStateReset();
        this.hooks.onResetTimeline(session.sessionId);
        onRouteChat(session.sessionId);
        await this.hooks.onLoadModelCatalog(session.sessionId);
        await this.refreshSessions();
        await this.hooks.onLoadActiveMessages();
        this.hooks.onStartPolling();
      },
      instruction,
      modelId
    );
  }

  async createSessionInWorkspace(
    path: string,
    currentPath: string,
    instruction: string = '',
    agentType: string = 'code',
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat,
    modelId: string = ''
  ): Promise<void> {
    if (path.length > 0 && path !== currentPath) {
      await this.hooks.onSelectWorkspace(path);
      await this.createSession(agentType, instruction, onRouteChat, modelId);
      return;
    }
    if (path.length === 0 || path === currentPath) {
      await this.createSession(agentType, instruction, onRouteChat, modelId);
    }
  }

  async openSession(
    item: RemoteSession,
    currentWorkspacePath: string,
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat
  ): Promise<void> {
    await this.sessions.open(
      item,
      item.workspacePath || currentWorkspacePath,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable(),
      async (session: SessionSummary): Promise<void> => {
        this.hooks.onStopPolling();
        this.hooks.onResetTimeline(item.id);
        this.hooks.onKnownStateReset();
        this.pageState.setHasMoreMessages(false);
        this.files.clear();
        this.pageState.clearComposer();
        onRouteChat(item.id);
        await this.hooks.onLoadModelCatalog(item.id);
        await this.hooks.onLoadActiveMessages();
        if (this.pageState.activeSession.sessionId === session.sessionId) {
          this.hooks.onStartPolling();
        }
      }
    );
  }

  async deleteSession(item: RemoteSession, currentWorkspacePath: string): Promise<void> {
    await this.sessions.delete(
      item,
      this.pageState.activeSession.sessionId,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable(),
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      async (): Promise<void> => {
        this.hooks.onStopPolling();
        this.hooks.onResetTimeline('');
        this.pageState.setHasMoreMessages(false);
        this.pageState.clearComposer();
        this.models.clearCatalog();
        this.hooks.onKnownStateReset();
        this.pageState.setActiveSession({
          sessionId: '',
          title: '',
          workspacePath: currentWorkspacePath,
          agentType: 'code'
        });
        this.pageState.clearActiveSession();
        this.hooks.onRouteHome();
      }
    );
  }

  async loadActiveMessages(isChatRoute: (sessionId: string) => boolean): Promise<void> {
    const sessionId = this.pageState.activeSession.sessionId || '';
    await this.chat.loadMessages(sessionId, isChatRoute);
  }

  async loadOlderMessages(knownPollVersion: number): Promise<void> {
    await this.chat.loadOlderMessages(
      this.pageState.activeSession.sessionId || '',
      knownPollVersion,
      this.pageState.hasMoreMessages,
      this.hooks.isBusy()
    );
  }

  async loadModelCatalog(sessionId: string, isChatRoute: (sessionId: string) => boolean): Promise<void> {
    await this.models.loadCatalog(sessionId, this.hooks.remoteAvailable(), isChatRoute);
  }

  async selectModel(modelId: string): Promise<void> {
    await this.models.selectModel(
      modelId,
      this.pageState.activeSession.sessionId || '',
      this.hooks.isBusy(),
      this.hooks.remoteAvailable()
    );
  }
}
