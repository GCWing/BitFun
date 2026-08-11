import {
  ChatMessage,
  RemoteModelCatalog,
  RemotePermissionMode,
  RemoteQuestionAnswerPayload,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary,
  WorkspaceInfo
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ClipboardService } from '../../services/ClipboardService';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { ImagePickerService } from '../../services/ImagePickerService';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore
} from '../../services/general-chat/GeneralChatConfigStore';
import { GeneralChatBootstrapController } from '../../services/general-chat/GeneralChatBootstrapController';
import { GeneralChatCommandController } from '../../services/general-chat/GeneralChatCommandController';
import { GeneralChatController } from '../../services/general-chat/GeneralChatController';
import { GeneralChatDraftController } from '../../services/general-chat/GeneralChatDraftController';
import { GeneralChatDraftLifecycleController } from '../../services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatStreamLifecycleController } from '../../services/general-chat/GeneralChatStreamLifecycleController';
import {
  GeneralChatSendResult,
  GeneralChatStreamCallbacks
} from '../../services/general-chat/GeneralChatPort';
import { MobileIdentityStore } from '../../services/MobileIdentityStore';
import { CloudAccountClient, CloudAccountDevice } from '../../services/CloudAccountClient';
import { CloudAccountSessionStore } from '../../services/CloudAccountSessionStore';
import { RemoteActivityLifecycleController } from '../../services/RemoteActivityLifecycleController';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import { RemoteChatPollingLifecycleController, RemoteChatPollingSnapshot } from '../../services/RemoteChatPollingLifecycleController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { FilePreviewController } from '../viewmodel/FilePreviewController';
import { SettingsController } from '../viewmodel/SettingsController';
import { ConversationController } from '../viewmodel/ConversationController';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteModelController } from '../../services/RemoteModelController';
import { RemotePairingPolicy } from '../../services/RemotePairingPolicy';
import { RemoteSessionController } from '../../services/RemoteSessionController';
import { RemoteSessionManager } from '../../services/RemoteSessionManager';
import { RemoteWorkspaceRepository } from '../../services/RemoteWorkspaceRepository';
import { RemoteWorkspaceCoordinator } from '../../services/RemoteWorkspaceCoordinator';
import { RemoteConnectionCoordinator } from '../../services/RemoteConnectionCoordinator';
import { RemoteToolActionController } from '../../services/RemoteToolActionController';
import { AsyncLifecycleGate } from '../../services/AsyncLifecycleGate';
import { QrScanService } from '../../services/QrScanService';
import { RemoteUiState } from '../../services/RemoteUiState';
import { VoiceInputLifecycleController } from '../../services/VoiceInputLifecycleController';
import { VoiceInputService } from '../../services/VoiceInputService';
import { ConversationIntent } from '../actions/ConversationIntent';
import { AppRootHostPort } from '../host/AppRootHostAdapter';
import { AppRootPresentationActions } from '../actions/AppRootPresentationActions';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract,
  ConversationSource
} from '../navigation/AppRouteContract';
import { AppShellState } from '../state/AppShellState';
import { AppShellViewModel } from '../viewmodel/AppShellViewModel';
import { RemoteActivityViewModel } from '../viewmodel/RemoteActivityViewModel';
import {
  RemoteConnectionController
} from '../viewmodel/RemoteConnectionController';
import { ConversationIntentDispatcher } from '../actions/ConversationIntentDispatcher';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { ConversationViewModel } from '../viewmodel/ConversationViewModel';
import { FilePreviewState } from '../state/FilePreviewState';
import { FilePreviewRequest } from '../../model/FilePreviewTarget';
import { RemoteWorkspaceViewModel } from '../viewmodel/RemoteWorkspaceViewModel';
import { RemoteSessionViewModel } from '../viewmodel/RemoteSessionViewModel';
import { GeneralChatConversationViewModel } from '../viewmodel/GeneralChatConversationViewModel';
import { ModelProviderGeneralChatAdapter } from '../../services/general-chat/ModelProviderGeneralChatAdapter';

export enum ConnectionState {
  Idle = 'idle',
  Parsing = 'parsing',
  Pairing = 'pairing',
  Connected = 'connected',
  Reconnecting = 'reconnecting',
  Failed = 'failed',
  Disconnected = 'disconnected'
}

const GENERAL_CHAT_HOME_DRAFT_ID: string = 'new-chat';
const GENERAL_CHAT_DRAFT_SAVE_DELAY_MS: number = 250;

export abstract class AppRootRuntimeComposition {
  readonly host: AppRootHostPort;

  constructor(host: AppRootHostPort) {
    this.host = host;
  }

  abstract applyDiscoveredWorkspaceSessions(all: RemoteSession[]): void;
  abstract closeAppSidebar(): void;
  abstract connect(autoReconnect?: boolean, accountPassword?: string): Promise<void>;
  abstract currentActiveTurnId(): string;
  abstract disconnect(clearPairing: boolean): Promise<void>;
  abstract enterCodeEntry(): void;
  abstract enterCompactLayout(): void;
  abstract exitActiveChat(): void;
  abstract failRemoteConnection(err: Object): void;
  abstract handleNavigationBack(route: AppRoute): boolean;
  abstract hasRemoteBindingForResume(): boolean;
  abstract isRemoteConversationContext(sessionId: string): boolean;
  abstract mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[];
  abstract openAddConnectionFromSettings(): void;
  abstract openAppSidebar(): void;
  abstract openRemoteControlSettings(): void;
  abstract pickImages(): Promise<void>;
  abstract reconnect(): Promise<void>;
  abstract reconnectActiveRemote(): Promise<void>;
  abstract selectAssistant(path: string): Promise<void>;
  abstract selectWorkspace(path: string): Promise<void>;
  abstract showVoiceInputError(message: string): void;
  abstract stopVoiceInput(showStatus: boolean): Promise<void>;
  abstract switchCompactConversationSource(source: ConversationSource): Promise<void>;
  abstract switchWideConversationSource(source: ConversationSource): Promise<void>;
  abstract toggleVoiceInput(): Promise<void>;

  readonly sessionManager: RemoteSessionManager = new RemoteSessionManager();
  readonly workspaceRepository: RemoteWorkspaceRepository =
    new RemoteWorkspaceRepository(this.sessionManager);
  readonly workspaceCoordinator: RemoteWorkspaceCoordinator =
    new RemoteWorkspaceCoordinator(this.workspaceRepository);
  readonly remoteResumeGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly remoteConnectionGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly filePreviewState: FilePreviewState = new FilePreviewState();
  readonly identityStore: MobileIdentityStore = new MobileIdentityStore();
  readonly clipboardService: ClipboardService = new ClipboardService();
  readonly qrScanService: QrScanService = new QrScanService();
  readonly imagePickerService: ImagePickerService = new ImagePickerService();
  readonly remotePairingPolicy: RemotePairingPolicy = new RemotePairingPolicy();
  readonly remoteConnectionCoordinator: RemoteConnectionCoordinator =
    new RemoteConnectionCoordinator(
      this.sessionManager,
      this.identityStore,
      this.remotePairingPolicy,
      this.remoteConnectionGate
    );
  readonly generalChatPageState: GeneralChatPageState = new GeneralChatPageState();
  readonly remotePageState: RemotePageState = new RemotePageState();
  readonly remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  readonly generalChatConfigStore: GeneralChatConfigStore = new GeneralChatConfigStore();
  readonly generalChatController: GeneralChatController =
    GeneralChatController.createDefault(this.generalChatConfigStore);
  readonly generalChatDraftController: GeneralChatDraftController =
    new GeneralChatDraftController(
      this.generalChatController,
      GENERAL_CHAT_DRAFT_SAVE_DELAY_MS,
      (err: Error) => {
        RemoteLogger.warn(`general chat draft operation failed: ${ConnectionErrorPolicy.errorText(err)}`);
      }
    );
  readonly generalChatDraftLifecycleController: GeneralChatDraftLifecycleController =
    new GeneralChatDraftLifecycleController(
      this.generalChatDraftController,
      GENERAL_CHAT_HOME_DRAFT_ID,
      (): string => this.conversationController.visibleGeneralChatDraftId()
    );
  readonly chatTimelineStore: ConversationViewModel = new ConversationViewModel();
  readonly generalChatCommandController: GeneralChatCommandController =
    new GeneralChatCommandController(
      this.generalChatController,
      {
        onSessions: (sessions: RemoteSession[]) => {
          this.generalChatPageState.setSessions(sessions);
        },
        onSessionPrepared: (sessionId: string) => {
          this.conversationController.resetGeneralTimeline(sessionId);
          this.remoteModelController.clearCatalog();
        },
        onActiveSession: (session: SessionSummary) => {
          this.generalChatPageState.setActiveSession(session);
        },
        onMessagesLoaded: (messages: ChatMessage[]) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.conversationController.syncGeneralTimeline();
        },
        onClearComposer: () => {
          this.generalChatPageState.clearComposer();
        },
        onChatInput: (text: string) => {
          this.generalChatPageState.setChatInput(text);
        },
        onStatusText: (statusText: string) => {
          this.generalChatPageState.setStatus(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.generalChatPageState.setBusy(isBusy);
        },
        onToast: (statusText: string) => {
          this.conversationController.showHomeToast(statusText);
        }
      }
    );
  readonly generalChatBootstrapController: GeneralChatBootstrapController =
    new GeneralChatBootstrapController(
      this.generalChatConfigStore,
      this.generalChatCommandController,
      this.generalChatDraftLifecycleController,
      {
        onConfigRestored: (snapshot: GeneralChatConfigSnapshot) => {
          this.settingsController.apply(snapshot);
        },
        onHomeDraftRestored: (text: string) => {
          this.generalChatPageState.setChatInput(text);
        },
        onStatusText: (statusText: string) => {
          this.generalChatPageState.setStatus(statusText);
        }
      }
    );
  readonly voiceInputService: VoiceInputService = new VoiceInputService();
  readonly remoteActivityLifecycleController: RemoteActivityLifecycleController =
    new RemoteActivityLifecycleController(() => {
      this.remoteActivityViewModel.checkConnectionHealth();
    });
  readonly remoteActivityViewModel: RemoteActivityViewModel =
    new RemoteActivityViewModel(
      this.remoteActivityLifecycleController,
      this.remoteConnectionCoordinator,
      this.remoteResumeGate,
      {
        isConnected: (): boolean => (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Connected,
        isBusy: (): boolean => this.remotePageState.isBusy,
        hasRemoteBinding: (): boolean => this.hasRemoteBindingForResume(),
        isRemoteChat: (): boolean => this.appShellViewModel.isRoute(AppRoute.RemoteChat),
        activeSession: (): SessionSummary => this.remotePageState.activeSession,
        onConnectionState: (state: string): void => this.remotePageState.setConnectionState(state as ConnectionState),
        onStatus: (status: string): void => this.remotePageState.setStatusText(status),
        onConnectionError: async (err: Object): Promise<boolean> => this.settingsController.handleRemoteConnectionError(err),
        onStopHeartbeat: (): void => this.remoteActivityViewModel.stopHeartbeat(),
        onStartPolling: (): void => this.conversationController.startRemotePolling(),
        onStopPolling: (): void => this.remoteChatPollingLifecycleController.stop(),
        onPoll: async (): Promise<void> => {
          await this.remoteChatPollingLifecycleController.pollNow();
        },
        onReconnect: async (): Promise<void> => {
          await this.reconnectActiveRemote();
        },
        onRestoreSession: async (session: SessionSummary): Promise<void> => {
          this.conversationController.applyRemoteActiveSession(session);
          await this.conversationController.loadRemoteMessages();
        }
      }
    );
  readonly generalChatStreamLifecycleController: GeneralChatStreamLifecycleController =
    new GeneralChatStreamLifecycleController();
  readonly remoteWorkspaceViewModel: RemoteWorkspaceViewModel =
    new RemoteWorkspaceViewModel(
      this.remotePageState,
      this.workspaceCoordinator,
      {
        isRemoteAvailable: (): boolean => this.remoteConnectionController.ensureAvailable(),
        isBusy: (): boolean => this.remotePageState.isBusy,
        onBusy: (isBusy: boolean): void => {
          this.remotePageState.setBusy(isBusy);
        },
        onStatus: (statusText: string): void => {
          this.remotePageState.setStatusText(statusText);
        },
        onWorkspaceSelected: (workspace: WorkspaceInfo): void => {
          this.remoteConnectionController.applyWorkspace(workspace);
          this.remoteSessionController.clearSessions();
        },
        onSessionsDiscovered: (sessions: RemoteSession[]): void => {
          this.applyDiscoveredWorkspaceSessions(sessions);
        },
        onRefreshSessions: async (): Promise<void> => {
          await this.remoteSessionViewModel.refreshSessions();
        },
        onConnectionFailure: (error: Object): void => {
          this.failRemoteConnection(error);
        }
      }
    );
  remoteWorkspaceSessions: RemoteSession[] = [];
  readonly appShellViewModel: AppShellViewModel = new AppShellViewModel();
  readonly appShellState: AppShellState = this.appShellViewModel.state;
  readonly voiceInputLifecycleController: VoiceInputLifecycleController =
    new VoiceInputLifecycleController(
      this.voiceInputService,
      {
        currentInputText: (): string => this.conversationController.visibleChatInput(),
        currentStatusText: (): string => this.conversationController.visibleStatusText(),
        onInputText: (routeId: string, text: string) => {
          this.conversationController.setChatInput(routeId as AppRoute, text);
        },
        onListening: (routeId: string, isListening: boolean) => {
          this.conversationController.setVoiceListening(routeId as AppRoute, isListening);
        },
        onStatusText: (statusText: string) => {
          this.conversationController.setVisibleStatusText(statusText);
        },
        onError: (message: string) => {
          this.showVoiceInputError(message);
        }
      }
    );
  readonly remoteSessionController: RemoteSessionController =
    new RemoteSessionController(
      this.sessionManager,
      8,
      {
        onSessions: (sessions: RemoteSession[], hasMore: boolean) => {
          const extras = this.remoteWorkspaceSessions.filter((item: RemoteSession) => {
            return item.workspacePath !== this.remotePageState.workspacePath;
          });
          this.remotePageState.setSessions(this.mergeSessions(sessions, extras), hasMore);
        },
        onActiveSession: (session: SessionSummary) => {
          this.conversationController.applyRemoteActiveSession(session);
        },
        onStatusText: (statusText: string) => {
          this.remotePageState.setStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.remotePageState.setBusy(isBusy);
        },
        onLoading: (isLoading: boolean) => {
          this.remotePageState.setLoading(isLoading);
        },
        onSessionError: (errorText: string) => {
          this.remotePageState.setError(errorText);
        },
        onReconnecting: () => {
          this.remotePageState.setConnectionState(ConnectionState.Reconnecting);
        },
        onConnected: () => {
          this.remotePageState.setConnectionState(ConnectionState.Connected);
        },
        onConnectionFailed: (err: Object) => {
          this.failRemoteConnection(err);
        },
        onStartHeartbeat: () => {
          this.remoteActivityViewModel.startHeartbeat();
        }
      }
    );
  readonly remoteChatCommandController: RemoteChatCommandController =
    new RemoteChatCommandController(
      this.sessionManager,
      {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.remotePageState.setHasMoreMessages(hasMoreMessages);
          this.conversationController.syncRemoteTimeline();
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          this.conversationController.updateKnownMessageCount(pollVersion, knownMessageCount);
        },
        onSendSucceeded: (turnId: string, pendingActiveId: string) => {
          if (turnId.length > 0) {
            this.chatTimelineStore.setLocalActiveTurn(turnId);
            this.conversationController.syncRemoteTimeline();
          } else if (pendingActiveId.length > 0) {
            this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
            this.conversationController.syncRemoteTimeline();
          }
          this.remoteChatPollingLifecycleController.nudge();
        },
        onSendFailed: (
          rawText: string,
          images: SelectedImageAttachment[],
          localMessageId: string,
          pendingActiveId: string
        ) => {
          this.remotePageState.setChatInput(rawText);
          this.remotePageState.setSelectedImages(images);
          this.chatTimelineStore.markOptimisticMessageFailed(localMessageId);
          if (pendingActiveId.length > 0) {
            this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
          }
          this.conversationController.syncRemoteTimeline();
        },
        onActiveSession: (session: SessionSummary) => {
          this.conversationController.applyRemoteActiveSession(session);
        },
        onSessionTitleChanged: (sessionId: string, title: string) => {
          this.remoteSessionController.updateSessionTitle(sessionId, title);
        },
        onStatusText: (statusText: string) => {
          this.remotePageState.setStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.remotePageState.setBusy(isBusy);
        },
        onPollRequested: () => {
          this.remoteChatPollingLifecycleController.pollNow();
        }
      }
    );
  readonly remoteFileDownloadController: RemoteFileDownloadController =
    new RemoteFileDownloadController(
      this.sessionManager,
      (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => {
        this.remotePageState.setDownloadStatus(downloadingFilePath, downloadedFilePath, fileDownloadStatus);
      },
      () => {
        this.remotePageState.clearDownloadingFilePath();
      },
      (statusText: string) => {
        this.remotePageState.setStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.remotePageState.setBusy(isBusy);
      }
    );
  readonly filePreviewController: FilePreviewController =
    new FilePreviewController(
      this.sessionManager,
      this.filePreviewState,
      {
        remoteAvailable: (): boolean => RemoteUiState.canUseRemote((this.remotePageState.connectionState as ConnectionState)),
        activeSession: (): SessionSummary => this.remotePageState.conversation.activeSession,
        workspacePath: (): string => this.remotePageState.workspacePath,
        openExternalLink: async (reference: string): Promise<boolean> =>
          this.host.openExternalLink ? await this.host.openExternalLink(reference) : false,
        onGeneralStatus: (statusText: string): void => this.generalChatPageState.setStatus(statusText),
        onRemoteStatus: (statusText: string): void => this.remotePageState.setStatusText(statusText)
      }
    );
  readonly remoteToolActionController: RemoteToolActionController =
    new RemoteToolActionController(
      this.sessionManager,
      (statusText: string) => {
        this.remotePageState.setStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.remotePageState.setBusy(isBusy);
      },
      () => {
        this.remoteChatPollingLifecycleController.pollNow();
      }
    );
  readonly remoteChatPollingLifecycleController: RemoteChatPollingLifecycleController =
    new RemoteChatPollingLifecycleController(
      this.sessionManager,
      {
        canPoll: (sessionId: string) => {
          return this.remotePageState.activeSession.sessionId === sessionId &&
            this.isRemoteConversationContext(sessionId) &&
            this.remoteConnectionController.ensureAvailable();
        },
        onSnapshot: (snapshot: RemoteChatPollingSnapshot) => {
          this.conversationController.applyRemoteSnapshot(snapshot);
        },
        onError: (error: Object) => {
          this.remotePageState.setStatusText(ConnectionErrorPolicy.errorText(error));
        }
      }
    );
  readonly remoteModelController: RemoteModelController =
    new RemoteModelController(
      this.sessionManager,
      this.identityStore,
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.conversationController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.conversationController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.chatTimelineStore.setModelCatalog(modelCatalog, selectedModelId);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (statusText: string) => {
        this.remotePageState.setStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.remotePageState.setBusy(isBusy);
      }
    );
  readonly remoteSessionViewModel: RemoteSessionViewModel =
    new RemoteSessionViewModel(
      this.remotePageState,
      this.remoteSessionController,
      this.remoteChatCommandController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      {
        remoteAvailable: (): boolean => this.remoteConnectionController.ensureAvailable(),
        isConnected: (): boolean => (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Connected,
        isBusy: (): boolean => this.remotePageState.isBusy,
        onBusy: (busy: boolean): void => this.remotePageState.setBusy(busy),
        onRouteChat: (sessionId: string): void => this.conversationController.routeCreatedRemoteSession(sessionId),
        onRouteHome: (): void => this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome),
        onStopPolling: (): void => this.remoteChatPollingLifecycleController.stop(),
        onStartPolling: (): void => this.conversationController.startRemotePolling(),
        onResetTimeline: (sessionId: string): void => this.conversationController.resetRemoteTimeline(sessionId),
        onClearRemoteFiles: (): void => this.remoteFileDownloadController.clear(),
        onKnownStateReset: (): void => this.conversationController.resetKnownRemoteState(),
        onLoadModelCatalog: async (sessionId: string): Promise<void> => {
          await this.conversationController.loadRemoteModelCatalog(sessionId);
        },
        onLoadActiveMessages: async (): Promise<void> => {
          await this.conversationController.loadRemoteMessages();
        },
        onRefreshSessions: async (): Promise<void> => {
          await this.remoteSessionController.refresh(
            this.remotePageState.sessionQuery,
            this.remotePageState.sessionFilter,
            this.remoteConnectionController.ensureAvailable(),
            (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Connected
          );
        },
        onSelectWorkspace: async (path: string): Promise<void> => {
          await this.selectWorkspace(path);
        }
      }
    );
  readonly generalChatConversationViewModel: GeneralChatConversationViewModel =
    new GeneralChatConversationViewModel(
      this.generalChatPageState,
      this.generalChatCommandController,
      this.generalChatDraftLifecycleController,
      this.generalChatStreamLifecycleController,
      this.chatTimelineStore,
      {
        isVisible: (sessionId: string): boolean => this.generalChatPageState.activeSession.sessionId === sessionId &&
          this.appShellViewModel.isGeneralChatVisible(),
        currentActiveTurnId: (): string => this.currentActiveTurnId(),
        latestUserMessageText: (): string => this.conversationController.latestUserMessageText(),
        syncTimeline: (): void => this.conversationController.syncGeneralTimeline(),
        refreshSessions: (): void => this.generalChatCommandController.refreshSessions()
      }
    );
  readonly remoteConnectionController: RemoteConnectionController =
    new RemoteConnectionController(
      this.remotePageState,
      this.identityStore,
      this.remotePairingPolicy,
      this.remoteConnectionCoordinator,
      this.remoteSessionController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      this.clipboardService,
      this.qrScanService,
      (sessionId: string): void => this.conversationController.resetRemoteTimeline(sessionId),
      (): void => this.conversationController.resetKnownRemoteState(),
      (): void => this.remoteActivityViewModel.startHeartbeat(),
      (): void => this.remoteActivityViewModel.stopHeartbeat(),
      (): void => this.remoteChatPollingLifecycleController.stop(),
      async (): Promise<void> => {
        await this.remoteWorkspaceViewModel.loadRecentWorkspacesInBackground();
      },
      (route: AppRoute): void => this.appShellViewModel.replaceRouteWithoutAnimation(route),
      (): void => this.appShellState.setConnectSheetVisible(false),
      (): void => this.appShellState.setConnectSheetVisible(true)
    );
  readonly settingsController: SettingsController =
    new SettingsController(
      this.generalChatConfigStore,
      this.generalChatPageState,
      {
        probeConfiguration: async (apiUrl: string, apiKey: string, modelName: string): Promise<void> => {
          await ModelProviderGeneralChatAdapter.probeConfiguration(apiUrl, apiKey, modelName);
        }
      },
      {
        client: new CloudAccountClient(),
        sessionStore: new CloudAccountSessionStore(),
        sessionManager: this.sessionManager,
        remoteState: this.remotePageState,
        hooks: {
          deviceId: (): string => this.remoteConnectionController.getDeviceId(),
          remoteAvailable: (): boolean => this.remoteConnectionController.ensureAvailable(),
          invalidatePreview: (): void => this.filePreviewController.invalidate(),
          invalidateRemoteActivity: (): void => this.remoteActivityViewModel.invalidate(),
          invalidateRemoteConnection: (): void => this.remoteConnectionCoordinator.invalidate(),
          stopPolling: (): void => this.remoteChatPollingLifecycleController.stop(),
          stopHeartbeat: (): void => this.remoteActivityViewModel.stopHeartbeat(),
          startHeartbeat: (): void => this.remoteActivityViewModel.startHeartbeat(),
          resetTimeline: (): void => this.conversationController.resetRemoteTimeline(''),
          resetKnownRemoteState: (): void => this.conversationController.resetKnownRemoteState(),
          closeSettings: (): void => this.appShellState.setSettingsVisible(false),
          closeConnectSheet: (): void => this.appShellState.setConnectSheetVisible(false),
          navigateRemoteHome: (): void => this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome),
          loadRecentWorkspaces: async (): Promise<void> => {
            await this.remoteWorkspaceViewModel.loadRecentWorkspacesInBackground();
          }
        }
      }
    );
  readonly conversationController: ConversationController =
    new ConversationController(
      this.generalChatPageState,
      this.remotePageState,
      this.remoteCreateState,
      { currentRoute: (): AppRoute => this.appShellViewModel.currentRoute() },
      {
        timeline: this.chatTimelineStore,
        chat: this.remoteChatCommandController,
        polling: this.remoteChatPollingLifecycleController,
        models: this.remoteModelController,
        files: this.remoteFileDownloadController,
        tools: this.remoteToolActionController,
        connection: this.remoteConnectionController,
        imagePicker: this.imagePickerService,
        clipboard: this.clipboardService,
        sessions: this.remoteSessionViewModel,
        sessionManager: this.sessionManager,
        workspace: this.workspaceCoordinator,
        settings: this.settingsController,
        appShell: this.appShellViewModel,
        filePreview: this.filePreviewController,
        generalCommands: this.generalChatCommandController,
        generalConversation: this.generalChatConversationViewModel,
        generalDrafts: this.generalChatDraftLifecycleController,
        hooks: {
          isConversationContext: (sessionId: string): boolean => this.isRemoteConversationContext(sessionId),
          isFilePreviewVisible: (): boolean => this.filePreviewState.visible,
          stopVoiceInput: async (): Promise<void> => this.stopVoiceInput(false),
          showToast: (message: string): boolean => this.host.showToast(message, 2600),
          selectAssistantWorkspace: async (path: string): Promise<void> => {
            await this.selectAssistant(path);
          }
        }
      }
    );
  readonly conversationIntentDispatcher: ConversationIntentDispatcher =
    new ConversationIntentDispatcher({
      openSidebar: (): void => this.openAppSidebar(),
      back: (): void => this.exitActiveChat(),
      newRemoteSession: (): void => { this.conversationController.createRemoteSession('code'); },
      newGeneralSession: (): void => this.conversationController.prepareNewGeneralChat(),
      activeGeneralSession: (): RemoteSession => this.conversationController.activeGeneralChatAsRemoteSession(),
      activeGeneralSessionId: (): string => this.generalChatPageState.activeSession.sessionId,
      isGeneralBusy: (): boolean => this.generalChatPageState.isBusy,
      isPinned: (sessionId: string): boolean => this.generalChatPageState.pinnedSessionId() === sessionId,
      pin: async (session: RemoteSession, pinned: boolean, busy: boolean): Promise<void> => {
        await this.generalChatCommandController.pinSession(session, pinned, busy);
      },
      archive: async (session: RemoteSession): Promise<void> => {
        await this.conversationController.archiveHomeSession(session, true);
      },
      delete: async (session: RemoteSession): Promise<void> => {
        await this.conversationController.deleteHomeSession(session);
        this.conversationController.prepareNewGeneralChat();
      },
      showToast: (text: string): void => this.conversationController.showHomeToast(text),
      uploadedFileCount: (): number => this.conversationController.activeGeneralUploadedFileCount(),
      stop: async (): Promise<void> => { await this.conversationController.stopVisibleTask(); },
      loadOlder: async (): Promise<void> => { await this.conversationController.loadOlderRemoteMessages(); },
      approve: async (id: string, input?: Object): Promise<void> => {
        await this.conversationController.approveRemoteTool(id, input);
      },
      reject: async (id: string): Promise<void> => { await this.conversationController.rejectRemoteTool(id); },
      cancel: async (id: string): Promise<void> => { await this.conversationController.cancelRemoteTool(id); },
      answer: async (id: string, answers: RemoteQuestionAnswerPayload): Promise<void> => {
        await this.conversationController.answerRemoteQuestion(id, answers);
      },
      rename: async (title: string): Promise<void> => {
        await this.conversationController.renameVisibleSession(title);
      },
      copy: async (text: string): Promise<void> => { await this.conversationController.copyRemoteMessage(text); },
      retry: async (text: string): Promise<void> => { await this.conversationController.retryVisibleMessage(text); },
      selectModel: async (id: string): Promise<void> => { await this.conversationController.selectVisibleModel(id); },
      pickImages: async (): Promise<void> => { await this.pickImages(); },
      removeImage: (id: string): void => this.conversationController.removeSelectedImage(this.appShellViewModel.currentRoute(), id),
      openFilePreview: (route: AppRoute, request: FilePreviewRequest): void =>
        this.filePreviewController.open(route, request),
      downloadFile: (path: string): void => this.conversationController.downloadVisibleFile(path),
      send: async (): Promise<void> => { await this.conversationController.sendVisibleMessage(); },
      voiceInput: async (): Promise<void> => { await this.toggleVoiceInput(); },
      inputChanged: (route: AppRoute, value: string): void =>
        this.conversationController.onVisibleChatInputChange(route, value)
    });
  readonly presentationActions: AppRootPresentationActions = {
    onNavigationBack: (route: AppRoute): boolean => this.handleNavigationBack(route),
    onConversationIntent: (route: AppRoute, intent: ConversationIntent): void =>
      this.conversationIntentDispatcher.dispatch(route, intent),
    onCloseSidebar: (): void => this.closeAppSidebar(),
    onWideConversationSource: (source: ConversationSource): void => {
      this.switchWideConversationSource(source);
    },
    onCompactConversationSource: (source: ConversationSource): void => {
      this.switchCompactConversationSource(source);
    },
    onCompactLayoutEntered: (): void => this.enterCompactLayout(),
    onLayoutModeChanged: (wideLayout: boolean): void => this.appShellState.setWideLayout(wideLayout),
    onRemoteHome: {
      openSidebar: (): void => this.openAppSidebar(),
      connectWorkspace: (): void => this.enterCodeEntry(),
      addConnection: (): void => this.appShellState.setConnectSheetVisible(true),
      openSettings: (): void => this.openRemoteControlSettings(),
      refresh: (): void => { this.remoteSessionViewModel.refreshSessions(); },
      showWorkspaces: (): void => { this.remoteWorkspaceViewModel.toggleRecentWorkspaces(); },
      showAssistants: (): void => { this.remoteWorkspaceViewModel.toggleAssistants(); },
      selectWorkspace: (path: string): void => { this.selectWorkspace(path); },
      selectAssistant: (path: string): void => { this.selectAssistant(path); },
      cancelWorkspace: (): void => this.remotePageState.setWorkspacePickerVisible(false),
      cancelAssistant: (): void => this.remotePageState.setAssistantPickerVisible(false),
      queryChanged: (query: string): void => this.remotePageState.setQuery(query),
      search: (): void => { this.remoteSessionViewModel.refreshSessions(); },
      loadMore: (): void => { this.remoteSessionViewModel.loadMoreSessions(); },
      reconnect: (): void => { this.reconnect(); },
      disconnect: (): void => { this.disconnect(false); },
      clearPairing: (): void => { this.disconnect(true); },
      create: (agentType: string): void => { this.conversationController.createRemoteSession(agentType); },
      createInPlace: (agentType: string): void => {
        this.conversationController.createRemoteSession(agentType, true);
      },
      createAssistant: (): void => { this.conversationController.openRemoteCreateSession(); },
      createInWorkspace: (path: string, agentType: string): void => {
        this.conversationController.createRemoteSessionInWorkspace(path, agentType);
      },
      createInWorkspaceInPlace: (path: string, agentType: string): void => {
        this.conversationController.createRemoteSessionInWorkspace(path, agentType, true);
      },
      openSession: (session: RemoteSession): void => this.conversationController.openHomeSession(session),
      openSessionInPlace: (session: RemoteSession): void => this.conversationController.openHomeSession(session, true),
      deleteSession: (session: RemoteSession): void => { this.conversationController.deleteHomeSession(session); }
    },
    onRemoteCreate: {
      back: (): void => this.conversationController.closeRemoteCreateSession(),
      toggleDevices: (): void => { this.conversationController.toggleRemoteCreateDevices(); },
      toggleWorkspaces: (): void => { this.conversationController.toggleRemoteCreateWorkspaces(); },
      selectDevice: (device: CloudAccountDevice): void => {
        this.conversationController.selectRemoteCreateDevice(device);
      },
      selectWorkspace: (path: string): void => this.conversationController.selectRemoteCreateWorkspace(path),
      draftChanged: (value: string): void => this.remoteCreateState.setDraft(value),
      voiceInput: async (): Promise<void> => { await this.toggleVoiceInput(); },
      selectModel: (modelId: string): void => this.remoteCreateState.setSelectedModelId(modelId),
      send: (): void => { this.conversationController.submitRemoteCreateSession(); }
    },
    onSidebar: {
      close: (): void => this.closeAppSidebar(),
      newChat: (): void => { this.closeAppSidebar(); this.conversationController.prepareNewGeneralChat(); },
      enterCode: (): void => { this.closeAppSidebar(); this.enterCodeEntry(); },
      settings: (): void => { this.closeAppSidebar(); this.appShellState.openSettings('general'); },
      openAccount: (): void => {
        this.closeAppSidebar();
        setTimeout(() => this.appShellState.openSettings('account'), 180);
      },
      openSession: (session: RemoteSession): void => {
        this.closeAppSidebar();
        this.conversationController.openHomeSession(session);
      },
      archive: (session: RemoteSession, archived: boolean): void => {
        this.conversationController.archiveHomeSession(session, archived);
      },
      exportSession: (session: RemoteSession): void => { this.conversationController.exportHomeSession(session); },
      deleteSession: (session: RemoteSession): void => { this.conversationController.deleteHomeSession(session); }
    },
    onSettings: {
      close: (): void => this.appShellState.leaveSettings(),
      addConnection: (): void => this.openAddConnectionFromSettings(),
      disconnect: (): void => { this.disconnect(false); },
      reconnect: (): void => { this.reconnect(); },
      openAccount: (): void => { this.appShellState.openSettings('account'); },
      cloudLogin: (relayUrl: string, username: string, password: string): Promise<string> =>
        this.settingsController.loginCloudAccount(relayUrl, username, password),
      cloudSync: (): Promise<string> => this.settingsController.syncCloudAccount(),
      cloudLogout: (): Promise<void> => this.settingsController.logoutCloudAccount(),
      cloudListDevices: (): Promise<CloudAccountDevice[]> => this.settingsController.listCloudAccountDevices(),
      getPermissionMode: (): Promise<RemotePermissionMode> => this.settingsController.getRemotePermissionMode(),
      setPermissionMode: (mode: RemotePermissionMode): Promise<RemotePermissionMode> =>
        this.settingsController.setRemotePermissionMode(mode),
      testGeneral: async (url: string, key: string, model: string, clear: boolean): Promise<string> =>
        this.settingsController.test(url, key, model, clear),
      saveGeneral: async (url: string, key: string, model: string, clear: boolean): Promise<string> =>
        this.settingsController.save(url, key, model, clear)
    },
    onConnect: {
      back: (): void => this.appShellState.setConnectSheetVisible(false),
      connect: (password?: string): void => {
        // Keep connection progress on the same RemoteHome surface as the connected state.
        this.appShellState.setConnectSheetVisible(false);
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
        this.connect(false, password || '');
      },
      clearPairing: (): void => { this.appShellState.setConnectSheetVisible(false); this.disconnect(true); },
      urlChanged: (url: string): void => { this.remotePageState.setRemoteUrl(url); this.remoteConnectionController.projectRemoteUrl(url); },
      userChanged: (user: string): void => this.remotePageState.setUserId(user),
      detected: (url: string): boolean => this.remoteConnectionController.handleDetectedUrl(url),
      inputVisible: (visible: boolean): void => this.remotePageState.setRemoteUrlInputVisible(visible),
      paste: (): void => { this.remoteConnectionController.paste(); },
      scan: (): void => { this.remoteConnectionController.scan(this.host.context()); },
      cloudListDevices: (): Promise<CloudAccountDevice[]> => this.settingsController.listCloudAccountDevices(),
      cloudSelectDevice: (device: CloudAccountDevice): Promise<void> =>
        this.settingsController.selectCloudAccountDevice(device)
    },
    onFilePreview: {
      close: (): void => this.filePreviewController.close(),
      refresh: (): void => this.filePreviewController.refresh(),
      download: (path: string): void => this.conversationController.downloadVisibleFile(path),
      openLink: (reference: string, label: string): void => this.filePreviewController.openLink(reference, label)
    },
    generalStatus: (): string => this.conversationController.generalChatHomeStatusText()
  };
  readonly navigationStack: NavPathStack = this.appShellViewModel.navigationStack;


}

