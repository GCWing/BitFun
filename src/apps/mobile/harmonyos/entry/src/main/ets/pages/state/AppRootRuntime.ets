import {
  ChatMessage,
  RecentWorkspaceEntry,
  RemoteModelCatalog,
  RemotePermissionMode,
  RemoteImageContext,
  RemoteQuestionAnswerPayload,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary,
  WorkspaceInfo
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ClipboardService } from '../../services/ClipboardService';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ChatTimelineState } from '../../services/ChatTimelineStore';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { ImagePickerService } from '../../services/ImagePickerService';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore,
  GeneralChatConfigUpdate,
  GeneralChatConfigValidator,
  GeneralChatModelSelectionPolicy
} from '../../services/general-chat/GeneralChatConfigStore';
import { GeneralChatBootstrapController } from '../../services/general-chat/GeneralChatBootstrapController';
import { GeneralChatCommandController } from '../../services/general-chat/GeneralChatCommandController';
import { GeneralChatController } from '../../services/general-chat/GeneralChatController';
import { GeneralChatDraftController } from '../../services/general-chat/GeneralChatDraftController';
import { GeneralChatDraftLifecycleController } from '../../services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatCloudConfigPolicy } from '../../services/general-chat/GeneralChatCloudConfigPolicy';
import { GeneralChatStreamLifecycleController } from '../../services/general-chat/GeneralChatStreamLifecycleController';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import {
  GeneralChatSendResult,
  GeneralChatStreamCallbacks
} from '../../services/general-chat/GeneralChatPort';
import { MobileIdentityStore } from '../../services/MobileIdentityStore';
import { CloudAccountClient, CloudAccountDevice, CloudAccountRequestError, CloudAccountSession } from '../../services/CloudAccountClient';
import { CloudAccountSessionStore } from '../../services/CloudAccountSessionStore';
import { Encoding } from '../../services/Encoding';
import { AppRootRouteState } from '../../services/AppRootRouteState';
import { RemoteActivityLifecycleController } from '../../services/RemoteActivityLifecycleController';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import {
  RemoteChatPollingCursor,
  RemoteChatPollingLifecycleController,
  RemoteChatPollingSnapshot
} from '../../services/RemoteChatPollingLifecycleController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { FileReferenceKind, FileTargetResolver } from '../../services/FileTargetResolver';
import { RemoteFilePreviewController } from '../../services/RemoteFilePreviewController';
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
import {
  VoiceInputLifecycleController,
  VoiceInputRouteSnapshot
} from '../../services/VoiceInputLifecycleController';
import { VoiceInputService } from '../../services/VoiceInputService';
import { ConversationIntent } from '../components/ConversationIntent';
import { AppRootHostPort } from '../host/AppRootHostAdapter';
import {
  AppRootPresentation,
  AppRootPresentationActions,
  ConnectPresentationActions,
  FilePreviewPresentationActions,
  RemoteCreatePresentationActions,
  RemoteHomePresentationActions,
  SettingsPresentationActions,
  SidebarPresentationActions
} from '../components/AppRootPresentation';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract,
  ConversationSource
} from '../navigation/AppRouteContract';
import { AppShellState } from './AppShellState';
import { AppShellViewModel } from './AppShellViewModel';
import {
  RemoteActivityViewModel,
  RemoteActivityViewModelHooks
} from './RemoteActivityViewModel';
import {
  RemoteConnectionViewModel
} from './RemoteConnectionViewModel';
import {
  ConversationIntentDispatcher,
  ConversationIntentDispatcherHooks
} from './ConversationIntentDispatcher';
import { GeneralChatPageState } from './GeneralChatPageState';
import { RemotePageState } from './RemotePageState';
import { RemoteCreateSessionState } from './RemoteCreateSessionState';
import { ConversationViewModel } from './ConversationViewModel';
import { FilePreviewState } from './FilePreviewState';
import { FilePreviewRequest, FilePreviewTargetContext } from './FilePreviewTarget';
import {
  RemoteWorkspaceViewModel,
  RemoteWorkspaceViewModelHooks
} from './RemoteWorkspaceViewModel';
import {
  RemoteSessionViewModel,
  RemoteSessionViewModelHooks
} from './RemoteSessionViewModel';
import {
  GeneralChatConversationViewModel,
  GeneralChatConversationViewModelHooks
} from './GeneralChatConversationViewModel';
import { ModelProviderGeneralChatAdapter } from '../../services/general-chat/ModelProviderGeneralChatAdapter';

enum ConnectionState {
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

export class AppRootRuntime {
  readonly host: AppRootHostPort;

  constructor(host: AppRootHostPort) {
    this.host = host;
  }

  readonly sessionManager: RemoteSessionManager = new RemoteSessionManager();
  readonly workspaceRepository: RemoteWorkspaceRepository =
    new RemoteWorkspaceRepository(this.sessionManager);
  readonly workspaceCoordinator: RemoteWorkspaceCoordinator =
    new RemoteWorkspaceCoordinator(this.workspaceRepository);
  readonly remoteResumeGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly remoteConnectionGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly filePreviewState: FilePreviewState = new FilePreviewState();
  private controlTargetEpoch: number = 1;
  private remoteCreateWorkspaceLoadVersion: number = 0;
  readonly identityStore: MobileIdentityStore = new MobileIdentityStore();
  readonly cloudAccountClient: CloudAccountClient = new CloudAccountClient();
  readonly cloudAccountSessionStore: CloudAccountSessionStore = new CloudAccountSessionStore();
  private cloudAccountSession?: CloudAccountSession;
  private cloudAccountRelayUrl: string = '';
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
      (): string => this.visibleGeneralChatDraftId()
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
          this.resetGeneralChatTimeline(sessionId);
          this.remoteModelController.clearCatalog();
        },
        onActiveSession: (session: SessionSummary) => {
          this.generalChatPageState.setActiveSession(session);
        },
        onMessagesLoaded: (messages: ChatMessage[]) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.syncGeneralChatTimelineFromStore();
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
          this.showHomeToast(statusText);
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
          this.applyGeneralChatConfig(snapshot);
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
      this.checkConnectionHealth();
    });
  readonly remoteActivityViewModel: RemoteActivityViewModel =
    new RemoteActivityViewModel(
      this.remoteActivityLifecycleController,
      this.remoteConnectionCoordinator,
      this.remoteResumeGate,
      new RemoteActivityViewModelHooks(
        (): boolean => this.connectionState === ConnectionState.Connected,
        (): boolean => this.isBusy,
        (): boolean => this.hasRemoteBindingForResume(),
        (): boolean => this.isRoute(AppRoute.RemoteChat),
        (): SessionSummary => this.activeSession,
        (state: string): void => this.setRemoteConnectionState(state as ConnectionState),
        (status: string): void => this.setRemoteStatusText(status),
        async (err: Object): Promise<boolean> => this.handleRemoteConnectionError(err),
        (): void => this.stopHeartbeat(),
        (): void => this.startPolling(),
        (): void => this.stopPolling(),
        async (): Promise<void> => {
          await this.pollActiveSession();
        },
        async (): Promise<void> => {
          await this.reconnectActiveRemote();
        },
        async (session: SessionSummary): Promise<void> => {
          this.applyRemoteActiveSession(session);
          await this.loadActiveMessages();
        }
      )
    );
  isSyncingAfterTurn: boolean = false;
  knownPollVersion: number = 0;
  knownModelCatalogVersion: number = 0;
  knownRemoteMessageCount: number = 0;
  readonly generalChatStreamLifecycleController: GeneralChatStreamLifecycleController =
    new GeneralChatStreamLifecycleController();
  readonly generalChatPageState: GeneralChatPageState = new GeneralChatPageState();
  readonly remotePageState: RemotePageState = new RemotePageState();
  readonly remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  readonly remoteWorkspaceViewModel: RemoteWorkspaceViewModel =
    new RemoteWorkspaceViewModel(
      this.remotePageState,
      this.workspaceCoordinator,
      new RemoteWorkspaceViewModelHooks(
        (): boolean => this.ensureRemoteAvailable(),
        (): boolean => this.isBusy,
        (isBusy: boolean): void => {
          this.setRemoteBusy(isBusy);
        },
        (statusText: string): void => {
          this.setRemoteStatusText(statusText);
        },
        (workspace: WorkspaceInfo): void => {
          this.applyWorkspace(workspace);
          this.remoteSessionController.clearSessions();
        },
        (sessions: RemoteSession[]): void => {
          this.applyDiscoveredWorkspaceSessions(sessions);
        },
        async (): Promise<void> => {
          await this.refreshSessions();
        },
        (error: Object): void => {
          this.failRemoteConnection(error);
        }
      )
    );
  remoteWorkspaceSessions: RemoteSession[] = [];
  readonly appShellViewModel: AppShellViewModel = new AppShellViewModel();
  readonly appShellState: AppShellState = this.appShellViewModel.state;
  readonly voiceInputLifecycleController: VoiceInputLifecycleController =
    new VoiceInputLifecycleController(
      this.voiceInputService,
      {
        currentInputText: (): string => this.visibleChatInput(),
        currentStatusText: (): string => this.visibleStatusText(),
        onInputText: (routeId: string, text: string) => {
          this.setChatInputForRoute(routeId as AppRoute, text);
        },
        onListening: (routeId: string, isListening: boolean) => {
          this.setVoiceListeningForRoute(routeId as AppRoute, isListening);
        },
        onStatusText: (statusText: string) => {
          this.setVisibleStatusText(statusText);
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
            return item.workspacePath !== this.workspacePath;
          });
          this.remotePageState.setSessions(this.mergeSessions(sessions, extras), hasMore);
        },
        onActiveSession: (session: SessionSummary) => {
          this.applyRemoteActiveSession(session);
        },
        onStatusText: (statusText: string) => {
          this.setRemoteStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.setRemoteBusy(isBusy);
        },
        onLoading: (isLoading: boolean) => {
          this.remotePageState.setLoading(isLoading);
        },
        onSessionError: (errorText: string) => {
          this.remotePageState.setError(errorText);
        },
        onReconnecting: () => {
          this.setRemoteConnectionState(ConnectionState.Reconnecting);
        },
        onConnected: () => {
          this.setRemoteConnectionState(ConnectionState.Connected);
        },
        onConnectionFailed: (err: Object) => {
          this.failRemoteConnection(err);
        },
        onStartHeartbeat: () => {
          this.startHeartbeat();
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
          this.syncChatTimelineFromStore();
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          this.knownRemoteMessageCount = knownMessageCount;
          this.updateChatPollingCursor(pollVersion, knownMessageCount);
        },
        onSendSucceeded: (turnId: string, pendingActiveId: string) => {
          if (turnId.length > 0) {
            this.chatTimelineStore.setLocalActiveTurn(turnId);
            this.syncChatTimelineFromStore();
          } else if (pendingActiveId.length > 0) {
            this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
            this.syncChatTimelineFromStore();
          }
          this.nudgeChatPolling();
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
          this.syncChatTimelineFromStore();
        },
        onActiveSession: (session: SessionSummary) => {
          this.applyRemoteActiveSession(session);
        },
        onSessionTitleChanged: (sessionId: string, title: string) => {
          this.remoteSessionController.updateSessionTitle(sessionId, title);
        },
        onStatusText: (statusText: string) => {
          this.setRemoteStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.setRemoteBusy(isBusy);
        },
        onPollRequested: () => {
          this.pollActiveSession();
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
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      }
    );
  readonly remoteFilePreviewController: RemoteFilePreviewController =
    new RemoteFilePreviewController(
      this.sessionManager,
      this.filePreviewState,
      (): boolean => RemoteUiState.canUseRemote(this.connectionState),
      (): number => this.controlTargetEpoch
    );
  readonly remoteToolActionController: RemoteToolActionController =
    new RemoteToolActionController(
      this.sessionManager,
      (statusText: string) => {
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      },
      () => {
        this.pollActiveSession();
      }
    );
  readonly remoteChatPollingLifecycleController: RemoteChatPollingLifecycleController =
    new RemoteChatPollingLifecycleController(
      this.sessionManager,
      {
        canPoll: (sessionId: string) => {
          return this.activeSession.sessionId === sessionId &&
            this.isRemoteConversationContext(sessionId) &&
            this.ensureRemoteAvailable();
        },
        onSnapshot: (snapshot: RemoteChatPollingSnapshot) => {
          this.applyChatSessionSnapshot(snapshot);
        },
        onError: (error: Object) => {
          this.setRemoteStatusText(ConnectionErrorPolicy.errorText(error));
        }
      }
    );
  readonly remoteModelController: RemoteModelController =
    new RemoteModelController(
      this.sessionManager,
      this.identityStore,
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.knownModelCatalogVersion = knownModelCatalogVersion;
        this.remoteChatPollingLifecycleController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.knownModelCatalogVersion = knownModelCatalogVersion;
        this.remoteChatPollingLifecycleController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.chatTimelineStore.setModelCatalog(modelCatalog, selectedModelId);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (statusText: string) => {
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      }
    );
  readonly remoteSessionViewModel: RemoteSessionViewModel =
    new RemoteSessionViewModel(
      this.remotePageState,
      this.remoteSessionController,
      this.remoteChatCommandController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      new RemoteSessionViewModelHooks(
        (): boolean => this.ensureRemoteAvailable(),
        (): boolean => this.connectionState === ConnectionState.Connected,
        (): boolean => this.isBusy,
        (busy: boolean): void => this.setRemoteBusy(busy),
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId),
        (): void => this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome),
        (): void => this.stopPolling(),
        (): void => this.startPolling(),
        (sessionId: string): void => this.resetChatTimeline(sessionId),
        (): void => this.remoteFileDownloadController.clear(),
        (): void => {
          this.knownPollVersion = 0;
          this.knownModelCatalogVersion = 0;
          this.knownRemoteMessageCount = 0;
        },
        async (sessionId: string): Promise<void> => {
          await this.remoteModelController.loadCatalog(
            sessionId,
            this.ensureRemoteAvailable(),
            (activeSessionId: string): boolean => {
              return this.isRemoteConversationContext(activeSessionId);
            }
          );
        },
        async (): Promise<void> => {
          const sessionId = this.activeSession.sessionId || '';
          await this.remoteChatCommandController.loadMessages(
            sessionId,
            (activeSessionId: string): boolean => {
              return this.isRemoteConversationContext(activeSessionId);
            }
          );
        },
        async (): Promise<void> => {
          await this.remoteSessionController.refresh(
            this.remotePageState.sessionQuery,
            this.remotePageState.sessionFilter,
            this.ensureRemoteAvailable(),
            this.connectionState === ConnectionState.Connected
          );
        },
        async (path: string): Promise<void> => {
          await this.selectWorkspace(path);
        }
      )
    );
  readonly generalChatConversationViewModel: GeneralChatConversationViewModel =
    new GeneralChatConversationViewModel(
      this.generalChatPageState,
      this.generalChatCommandController,
      this.generalChatDraftLifecycleController,
      this.generalChatStreamLifecycleController,
      this.chatTimelineStore,
      new GeneralChatConversationViewModelHooks(
        (sessionId: string): boolean => this.generalChatPageState.activeSession.sessionId === sessionId &&
          this.isGeneralChatVisible(),
        (): string => this.currentActiveTurnId(),
        (): string => this.latestUserMessageText(),
        (): void => this.syncGeneralChatTimelineFromStore(),
        (): void => this.generalChatCommandController.refreshSessions()
      )
    );
  readonly remoteConnectionViewModel: RemoteConnectionViewModel =
    new RemoteConnectionViewModel(
      this.remotePageState,
      this.identityStore,
      this.remotePairingPolicy,
      this.remoteConnectionCoordinator,
      this.remoteSessionController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      this.clipboardService,
      this.qrScanService,
      (sessionId: string): void => this.resetChatTimeline(sessionId),
      (): void => {
        this.knownPollVersion = 0;
        this.knownModelCatalogVersion = 0;
        this.knownRemoteMessageCount = 0;
      },
      (): void => this.startHeartbeat(),
      (): void => this.stopHeartbeat(),
      (): void => this.stopPolling(),
      async (): Promise<void> => {
        await this.loadRecentWorkspacesInBackground();
      },
      (route: AppRoute): void => this.appShellViewModel.replaceRouteWithoutAnimation(route),
      (): void => this.appShellState.setConnectSheetVisible(false),
      (): void => this.appShellState.setConnectSheetVisible(true)
    );
  readonly conversationIntentDispatcher: ConversationIntentDispatcher =
    new ConversationIntentDispatcher(new ConversationIntentDispatcherHooks(
      (): void => this.openAppSidebar(),
      (): void => this.closeActiveChat(),
      (): void => { this.createSession('code'); },
      (): void => this.prepareNewGeneralChat(),
      (): RemoteSession => this.activeGeneralChatAsRemoteSession(),
      (): string => this.generalChatPageState.activeSession.sessionId,
      (): boolean => this.generalChatPageState.isBusy,
      (sessionId: string): boolean => this.generalChatPageState.pinnedSessionId() === sessionId,
      async (session: RemoteSession, pinned: boolean, busy: boolean): Promise<void> => {
        await this.generalChatCommandController.pinSession(session, pinned, busy);
      },
      async (session: RemoteSession): Promise<void> => { await this.archiveHomeSession(session, true); },
      async (session: RemoteSession): Promise<void> => {
        await this.deleteHomeSession(session);
        this.prepareNewGeneralChat();
      },
      (text: string): void => this.showHomeToast(text),
      (): number => this.activeGeneralUploadedFileCount(),
      async (): Promise<void> => { await this.stopActiveChatTask(); },
      async (): Promise<void> => { await this.loadOlderMessages(); },
      async (id: string, input?: Object): Promise<void> => { await this.approveTool(id, input); },
      async (id: string): Promise<void> => { await this.rejectTool(id); },
      async (id: string): Promise<void> => { await this.cancelTool(id); },
      async (id: string, answers: RemoteQuestionAnswerPayload): Promise<void> => {
        await this.answerQuestion(id, answers);
      },
      async (title: string): Promise<void> => { await this.renameVisibleSession(title); },
      async (text: string): Promise<void> => { await this.copyMessage(text); },
      async (text: string): Promise<void> => { await this.retryVisibleMessage(text); },
      async (id: string): Promise<void> => { await this.selectModel(id); },
      async (): Promise<void> => { await this.pickImages(); },
      (id: string): void => this.removeSelectedImage(id),
      (route: AppRoute, request: FilePreviewRequest): void => this.openFilePreview(route, request),
      (path: string): void => this.downloadVisibleFile(path),
      async (): Promise<void> => { await this.sendVisibleChatMessage(); },
      async (): Promise<void> => { await this.toggleVoiceInput(); },
      (route: AppRoute, value: string): void => this.onVisibleChatInputChange(route, value)
    ));
  readonly presentationActions: AppRootPresentationActions = new AppRootPresentationActions(
    (route: AppRoute): boolean => this.handleNavigationBack(route),
    (route: AppRoute, intent: ConversationIntent): void => this.handleConversationIntent(route, intent),
    (): void => this.closeAppSidebar(),
    (source: ConversationSource): void => { this.switchWideConversationSource(source); },
    (source: ConversationSource): void => { this.switchCompactConversationSource(source); },
    (): void => this.enterCompactLayout(),
    new RemoteHomePresentationActions(
      (): void => this.openAppSidebar(), (): void => this.enterCodeEntry(), (): void => this.openAddConnection(),
      (): void => this.openRemoteControlSettings(), (): void => { this.refreshSessions(); },
      (): void => { this.showRecentWorkspaces(); }, (): void => { this.showAssistants(); },
      (path: string): void => { this.selectWorkspace(path); }, (path: string): void => { this.selectAssistant(path); },
      (): void => this.remotePageState.setWorkspacePickerVisible(false),
      (): void => this.remotePageState.setAssistantPickerVisible(false),
      (query: string): void => this.remotePageState.setQuery(query), (): void => { this.refreshSessions(); },
      (): void => { this.loadMoreSessions(); }, (): void => { this.reconnect(); },
      (): void => { this.disconnect(false); }, (): void => { this.disconnect(true); },
      (agentType: string): void => { this.createSession(agentType); },
      (agentType: string): void => { this.createSession(agentType, true); },
      (): void => { this.openRemoteCreateSession(); },
      (path: string, agentType: string): void => { this.createSessionInWorkspace(path, agentType); },
      (path: string, agentType: string): void => { this.createSessionInWorkspace(path, agentType, true); },
      (session: RemoteSession): void => this.openHomeSession(session),
      (session: RemoteSession): void => this.openHomeSessionInPlace(session),
      (session: RemoteSession): void => { this.deleteHomeSession(session); }
    ),
    new RemoteCreatePresentationActions(
      (): void => this.closeRemoteCreateSession(),
      (): void => { this.toggleRemoteCreateDevices(); },
      (): void => { this.toggleRemoteCreateWorkspaces(); },
      (device: CloudAccountDevice): void => { this.selectRemoteCreateDevice(device); },
      (path: string): void => this.selectRemoteCreateWorkspace(path),
      (value: string): void => this.remoteCreateState.setDraft(value),
      async (): Promise<void> => { await this.toggleVoiceInput(); },
      (modelId: string): void => this.selectRemoteCreateModel(modelId),
      (): void => { this.submitRemoteCreateSession(); }
    ),
    new SidebarPresentationActions(
      (): void => this.closeAppSidebar(),
      (): void => { this.closeAppSidebar(); this.prepareNewGeneralChat(); },
      (): void => { this.closeAppSidebar(); this.enterCodeEntry(); },
      (): void => { this.closeAppSidebar(); this.appShellState.openSettings('general'); },
      (): void => {
        this.closeAppSidebar();
        setTimeout(() => this.appShellState.openSettings('account'), 180);
      },
      (session: RemoteSession): void => { this.closeAppSidebar(); this.openHomeSession(session); },
      (session: RemoteSession, archived: boolean): void => { this.archiveHomeSession(session, archived); },
      (session: RemoteSession): void => { this.exportHomeSession(session); },
      (session: RemoteSession): void => { this.deleteHomeSession(session); }
    ),
    new SettingsPresentationActions(
      (): void => this.appShellState.leaveSettings(),
      (): void => this.openAddConnectionFromSettings(), (): void => { this.disconnect(false); },
      (): void => { this.reconnect(); },
      (): void => {
        this.appShellState.openSettings('account');
      },
      (relayUrl: string, username: string, password: string): Promise<string> =>
        this.loginCloudAccount(relayUrl, username, password),
      (): Promise<string> => this.syncCloudAccount(),
      (): Promise<void> => this.logoutCloudAccount(),
      (): Promise<CloudAccountDevice[]> => this.listCloudAccountDevices(),
      (): Promise<RemotePermissionMode> => this.getRemotePermissionMode(),
      (mode: RemotePermissionMode): Promise<RemotePermissionMode> => this.setRemotePermissionMode(mode),
      async (url: string, key: string, model: string, clear: boolean): Promise<string> =>
        this.testGeneralChatConfig(url, key, model, clear),
      async (url: string, key: string, model: string, clear: boolean): Promise<string> =>
        this.saveGeneralChatConfig(url, key, model, clear)
    ),
    new ConnectPresentationActions(
      (): void => this.appShellState.setConnectSheetVisible(false),
      (password?: string): void => {
        // Keep connection progress on the same RemoteHome surface as the
        // connected state instead of showing a separate loading sheet.
        this.appShellState.setConnectSheetVisible(false);
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
        this.connect(false, password || '');
      },
      (): void => { this.appShellState.setConnectSheetVisible(false); this.disconnect(true); },
      (url: string): void => { this.setRemoteUrl(url); this.applyRemotePairingProjection(url); },
      (user: string): void => this.setRemoteUserId(user),
      (url: string): boolean => this.handleDetectedRemoteUrl(url),
      (visible: boolean): void => this.setRemoteUrlInputVisible(visible),
      (): void => { this.pasteRemoteUrl(); }, (): void => { this.scanRemoteUrl(); },
      (): Promise<CloudAccountDevice[]> => this.listCloudAccountDevices(),
      (device: CloudAccountDevice): Promise<void> => this.selectCloudAccountDevice(device)
    ),
    new FilePreviewPresentationActions(
      (): void => this.closeFilePreview(),
      (): void => this.refreshFilePreview(),
      (path: string): void => this.downloadVisibleFile(path),
      (reference: string, label: string): void => this.openFilePreviewLink(reference, label)
    ),
    (): string => this.generalChatHomeStatusText()
  );
  readonly navigationStack: NavPathStack = this.appShellViewModel.navigationStack;


  get remoteUrl(): string { return this.remotePageState.remoteUrl; }
  get userId(): string { return this.remotePageState.userId; }
  get authenticatedUserId(): string { return this.remotePageState.authenticatedUserId; }
  get statusText(): string { return this.remotePageState.statusText; }
  get connectionState(): ConnectionState { return this.remotePageState.connectionState as ConnectionState; }
  get connectionFailureKind(): string { return this.remotePageState.connectionFailureKind; }
  get isBusy(): boolean { return this.remotePageState.isBusy; }
  get showRemoteUrlInput(): boolean { return this.remotePageState.showRemoteUrlInput; }
  get workspaceName(): string { return this.remotePageState.workspaceName; }
  get workspacePath(): string { return this.remotePageState.workspacePath; }
  get workspaceBranch(): string { return this.remotePageState.workspaceBranch; }
  get workspaceKind(): string { return this.remotePageState.workspaceKind; }
  get assistantId(): string { return this.remotePageState.assistantId; }
  get desktopName(): string { return this.remotePageState.desktopName; }
  get desktopId(): string { return this.remotePageState.desktopId; }
  get activeSession(): SessionSummary { return this.remotePageState.activeSession; }
  get messages(): ChatMessage[] { return this.remotePageState.persistedMessages; }
  get pendingMessages(): ChatMessage[] { return this.remotePageState.optimisticMessages; }
  get activeTurnMessage(): ChatMessage { return this.remotePageState.activeTurnMessage; }
  get timelineItems(): ChatTimelineItem[] { return this.remotePageState.timelineItems; }
  get hasMoreMessages(): boolean { return this.remotePageState.hasMoreMessages; }

  async aboutToAppear(): Promise<void> {
    this.syncRemotePageSummary();
    await this.generalChatBootstrapController.restore(this.host.context());
    await this.cloudAccountSessionStore.init(this.host.context());
    await this.restoreCloudAccountSession();
    await this.refreshGeneralChatModelCatalog();
    await this.restoreIdentity();
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${this.connectionState} route=${this.currentRoute()}`);
    this.resumeRemoteActivity();
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${this.connectionState} route=${this.currentRoute()}`);
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.setRemoteBusy(false);
    this.persistVisibleGeneralChatDraft();
  }

  aboutToDisappear(): void {
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.setRemoteBusy(false);
    this.persistVisibleGeneralChatDraft();
    this.stopGeneralChatStream(true, 'failed');
    this.generalChatDraftLifecycleController.cancel();
    this.remoteFileDownloadController.cancel();
    this.remoteFilePreviewController.close();
    this.voiceInputLifecycleController.cancel(`${this.currentRoute()}`, () => {
      this.setAllVoiceListening(false);
    });
  }

  currentRoute(): AppRoute {
    return this.appShellViewModel.currentRoute();
  }

  isGeneralComposerRoute(route: AppRoute): boolean {
    return AppRootRouteState.isGeneralComposerRoute(route);
  }

  visibleChatInput(): string {
    if (this.currentRoute() === AppRoute.RemoteCreate) {
      return this.remoteCreateState.draft;
    }
    return AppRootRouteState.chatInput(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  visibleSelectedImages(): SelectedImageAttachment[] {
    return AppRootRouteState.selectedImages(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  visibleVoiceListening(): boolean {
    if (this.currentRoute() === AppRoute.RemoteCreate) {
      return this.remoteCreateState.isVoiceListening;
    }
    return AppRootRouteState.voiceListening(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  setChatInputForRoute(route: AppRoute, value: string): void {
    if (route === AppRoute.RemoteCreate) {
      this.remoteCreateState.setDraft(value);
      return;
    }
    AppRootRouteState.setChatInput(route, value, this.generalChatPageState, this.remotePageState);
  }

  setSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.setSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  addSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.addSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  removeSelectedImageForRoute(route: AppRoute, imageId: string): void {
    AppRootRouteState.removeSelectedImage(route, imageId, this.generalChatPageState, this.remotePageState);
  }

  clearComposerForRoute(route: AppRoute): void {
    AppRootRouteState.clearComposer(route, this.generalChatPageState, this.remotePageState);
  }

  setVoiceListeningForRoute(route: AppRoute, isVoiceListening: boolean): void {
    if (route === AppRoute.RemoteCreate) {
      this.remoteCreateState.isVoiceListening = isVoiceListening;
      return;
    }
    AppRootRouteState.setVoiceListening(
      route,
      isVoiceListening,
      this.generalChatPageState,
      this.remotePageState
    );
  }

  setAllVoiceListening(isVoiceListening: boolean): void {
    this.generalChatPageState.setVoiceListening(isVoiceListening);
    this.remotePageState.setVoiceListening(isVoiceListening);
  }

  voiceInputSnapshot(route: AppRoute = this.currentRoute()): VoiceInputRouteSnapshot {
    if (route === AppRoute.RemoteCreate) {
      return {
        routeId: `${route}`,
        isListening: this.remoteCreateState.isVoiceListening,
        isBusy: this.remoteCreateState.isSubmitting,
        inputText: this.remoteCreateState.draft,
        selectedImageCount: 0
      };
    }
    return AppRootRouteState.snapshot(
      route,
      this.visibleChatBusy(),
      this.generalChatPageState,
      this.remotePageState
    );
  }

  isRoute(route: AppRoute): boolean {
    return this.appShellViewModel.isRoute(route);
  }

  isGeneralChatVisible(): boolean {
    return this.appShellViewModel.isGeneralChatVisible();
  }

  pushRoute(route: AppRoute, sessionId: string = ''): void {
    this.appShellViewModel.pushRoute(route, sessionId);
  }

  replaceRoute(route: AppRoute, sessionId: string = ''): void {
    this.appShellViewModel.replaceRoute(route, sessionId);
  }

  popRoute(fallback: AppRoute): void {
    this.appShellViewModel.popRoute(fallback);
  }

  private routeCreatedRemoteSession(sessionId: string): void {
    this.closeFilePreview();
    if (this.isRoute(AppRoute.RemoteCreate)) {
      this.appShellViewModel.replaceCurrentRoute(AppRoute.RemoteChat, sessionId);
      return;
    }
    this.pushRoute(AppRoute.RemoteChat, sessionId);
  }

  private routeRemoteSessionInPlace(_sessionId: string): void {
    this.closeFilePreview();
    if (this.isRoute(AppRoute.RemoteHome) || this.isRoute(AppRoute.RemoteChat)) {
      return;
    }
    this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
  }

  private isRemoteConversationContext(sessionId: string): boolean {
    if (sessionId.length === 0 || this.activeSession.sessionId !== sessionId) {
      return false;
    }
    return this.isRoute(AppRoute.RemoteChat) || this.isRoute(AppRoute.RemoteHome);
  }

  handleNavigationBack(route: AppRoute): boolean {
    if (this.filePreviewState.visible) {
      this.closeFilePreview();
      return true;
    }
    const action = this.appShellViewModel.backAction(route);
    if (action === AppNavigationBackAction.CloseSidebar) {
      this.closeAppSidebar();
      return true;
    }
    if (action === AppNavigationBackAction.CloseActiveChat) {
      this.closeActiveChat();
      return true;
    }
    if (action === AppNavigationBackAction.PopRemoteHome) {
      this.popRoute(AppRoute.ChatHome);
      return true;
    }
    return false;
  }

  handleRootBack(): boolean {
    if (!this.filePreviewState.visible) {
      return false;
    }
    this.closeFilePreview();
    return true;
  }


  handleConversationIntent(route: AppRoute, intent: ConversationIntent): void {
    this.conversationIntentDispatcher.dispatch(route, intent);
  }


  async saveGeneralChatConfig(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): Promise<string> {
    const update: GeneralChatConfigUpdate = {
      apiUrl,
      apiKey,
      modelName,
      clearApiKey
    };
    try {
      const validationError = await this.validateGeneralChatConfig(update);
      if (validationError.length > 0) {
        return validationError;
      }
      if (!update.clearApiKey) {
        const probeError = await this.probeGeneralChatConfig(update);
        if (probeError.length > 0) {
          return probeError;
        }
      }
      const catalogBeforeSave = await this.generalChatConfigStore.modelCatalog();
      const snapshot = await this.generalChatConfigStore.save(update);
      if (GeneralChatModelSelectionPolicy.shouldActivateSavedLocalModel(catalogBeforeSave)) {
        await this.generalChatConfigStore.selectLocalModel();
      }
      this.applyGeneralChatConfig(snapshot);
      await this.refreshGeneralChatModelCatalog();
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  async testGeneralChatConfig(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): Promise<string> {
    const update: GeneralChatConfigUpdate = {
      apiUrl,
      apiKey,
      modelName,
      clearApiKey
    };
    try {
      const validationError = await this.validateGeneralChatConfig(update);
      if (validationError.length > 0) {
        return validationError;
      }
      if (update.clearApiKey) {
        return RemoteI18n.t('settings.modelService.testNeedsKey');
      }
      return await this.probeGeneralChatConfig(update);
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  private async validateGeneralChatConfig(update: GeneralChatConfigUpdate): Promise<string> {
    const snapshot = await this.generalChatConfigStore.snapshot();
    return GeneralChatConfigValidator.validate(update, snapshot.hasApiKey);
  }

  private async probeGeneralChatConfig(update: GeneralChatConfigUpdate): Promise<string> {
    const apiKey = await this.effectiveGeneralChatApiKey(update);
    if (apiKey.length === 0) {
      return RemoteI18n.t('settings.modelService.apiKeyRequired');
    }
    try {
      await ModelProviderGeneralChatAdapter.probeConfiguration(update.apiUrl, apiKey, update.modelName);
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  private async effectiveGeneralChatApiKey(update: GeneralChatConfigUpdate): Promise<string> {
    const directKey = update.apiKey.trim();
    if (directKey.length > 0) {
      return directKey;
    }
    if (update.clearApiKey) {
      return '';
    }
    return (await this.generalChatConfigStore.accessToken()).trim();
  }

  applyGeneralChatConfig(snapshot: GeneralChatConfigSnapshot): void {
    this.generalChatPageState.setConfiguration(
      snapshot.apiUrl,
      snapshot.modelName,
      snapshot.hasApiKey,
      GeneralChatServiceStatus.fromConfiguration(snapshot.apiUrl, snapshot.modelName, snapshot.hasApiKey)
    );
  }

  private async refreshGeneralChatModelCatalog(): Promise<void> {
    const catalog = await this.generalChatConfigStore.modelCatalog();
    const selectedModelId = catalog.session_model_id || catalog.default_models.primary || '';
    this.generalChatPageState.setModelCatalog(catalog, selectedModelId);
    const active = await this.generalChatConfigStore.activeSnapshot();
    this.generalChatPageState.setServiceState(
      GeneralChatServiceStatus.fromConfiguration(active.apiUrl, active.modelName, active.hasApiKey)
    );
  }

  async restoreIdentity(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      return;
    }
    await this.remoteConnectionViewModel.restore(this.host.context());
  }

  async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    await this.remoteConnectionViewModel.connect(autoReconnect, accountPassword);
    await this.persistDelegatedAccountSession();
  }

  private async persistDelegatedAccountSession(): Promise<void> {
    if (this.cloudAccountSession) {
      return;
    }
    const delegated = this.sessionManager.delegatedAccountSession();
    if (!delegated) {
      return;
    }
    this.cloudAccountSession = delegated.session;
    this.cloudAccountRelayUrl = delegated.relayUrl;
    await this.cloudAccountSessionStore.save({
      relayUrl: delegated.relayUrl,
      username: delegated.session.userId,
      token: delegated.session.token,
      userId: delegated.session.userId,
      masterKey: Encoding.bytesToBase64(delegated.session.masterKey)
    });
    this.remotePageState.setAccountUserId(delegated.session.userId);
    this.remotePageState.setAccountUsername(delegated.session.userId);
    RemoteLogger.info('delegated account session persisted after room pairing');
  }

  async reconnect(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      await this.restoreCloudTarget(
        this.remotePageState.controlTargetDeviceId,
        this.remotePageState.controlTargetDeviceName
      );
      return;
    }
    await this.remoteConnectionViewModel.reconnect();
  }

  async disconnect(clearPairing: boolean): Promise<void> {
    this.invalidateFilePreviewTarget();
    await this.remoteConnectionViewModel.disconnect(clearPairing);
  }

  async pasteRemoteUrl(): Promise<void> {
    await this.remoteConnectionViewModel.paste();
  }

  async scanRemoteUrl(): Promise<void> {
    await this.remoteConnectionViewModel.scan(this.host.context());
  }

  handleDetectedRemoteUrl(remoteUrl: string): boolean {
    return this.remoteConnectionViewModel.handleDetectedUrl(remoteUrl);
  }

  applyWorkspace(workspace: WorkspaceInfo): void {
    this.remoteConnectionViewModel.applyWorkspace(workspace);
  }

  applyRemotePairingProjection(remoteUrl: string): void {
    this.remoteConnectionViewModel.projectRemoteUrl(remoteUrl);
  }

  ensureRemoteAvailable(): boolean {
    return this.remoteConnectionViewModel.ensureAvailable();
  }

  setRemoteConnectionState(connectionState: ConnectionState): void {
    this.remotePageState.setConnectionState(connectionState);
  }

  setRemoteUrl(remoteUrl: string): void {
    this.remotePageState.setRemoteUrl(remoteUrl);
  }

  setRemoteUserId(userId: string): void {
    this.remotePageState.setUserId(userId);
  }

  setRemoteAuthenticatedUserId(authenticatedUserId: string): void {
    this.remotePageState.setAuthenticatedUserId(authenticatedUserId);
  }

  setRemoteStatusText(statusText: string): void {
    this.remotePageState.setStatusText(statusText);
  }

  setRemoteConnectionFailureKind(connectionFailureKind: string): void {
    this.remotePageState.setConnectionFailureKind(connectionFailureKind);
  }

  setRemoteBusy(isBusy: boolean): void {
    this.remotePageState.setBusy(isBusy);
  }

  setRemoteUrlInputVisible(visible: boolean): void {
    this.remotePageState.setRemoteUrlInputVisible(visible);
  }

  syncRemotePageSummary(): void {
    if (this.remotePageState.statusText.length === 0) {
      this.remotePageState.setStatusText(RemoteI18n.t('status.waitingConnection'));
    }
    if (this.remotePageState.workspaceName.length === 0) {
      this.remotePageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    }
  }

  failRemoteConnection(err: Object): void {
    this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    this.setRemoteConnectionState(ConnectionState.Failed);
    this.stopHeartbeat();
  }

  async showRecentWorkspaces(): Promise<void> {
    await this.remoteWorkspaceViewModel.toggleRecentWorkspaces();
  }

  async showAssistants(): Promise<void> {
    await this.remoteWorkspaceViewModel.toggleAssistants();
  }

  async selectWorkspace(path: string): Promise<void> {
    this.closeFilePreview();
    await this.remoteWorkspaceViewModel.selectWorkspace(path);
  }

  async selectAssistant(path: string): Promise<void> {
    this.closeFilePreview();
    await this.remoteWorkspaceViewModel.selectAssistant(path);
  }

  async refreshSessions(): Promise<void> {
    await this.remoteSessionViewModel.refreshSessions();
  }

  async loadMoreSessions(): Promise<void> {
    await this.remoteSessionViewModel.loadMoreSessions();
  }

  setSessionFilter(filter: string): void {
    this.remoteSessionViewModel.setFilter(filter);
  }

  visibleChatBusy(): boolean {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.isBusy : this.remotePageState.isBusy;
  }

  visibleStatusText(): string {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.statusText : this.remotePageState.statusText;
  }

  setVisibleStatusText(statusText: string): void {
    if (this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat)) {
      this.generalChatPageState.setStatus(statusText);
      return;
    }
    this.setRemoteStatusText(statusText);
  }

  openAppSidebar(): void {
    this.host.animate(230, () => {
      this.appShellState.setSidebarVisible(true);
    });
  }

  closeAppSidebar(): void {
    this.host.animate(210, () => {
      this.appShellState.setSidebarVisible(false);
    });
  }

  enterCodeEntry(): void {
    if (this.cloudAccountSession && this.remotePageState.accountUserId.trim().length > 0) {
      this.appShellState.setConnectSheetVisible(true);
      return;
    }
    if (RemoteUiState.canUseRemote(this.connectionState)) {
      this.appShellState.setConnectSheetVisible(false);
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      return;
    }
    this.appShellState.setConnectSheetVisible(true);
  }

  async switchWideConversationSource(source: ConversationSource): Promise<void> {
    if (AppRouteContract.conversationSource(this.currentRoute()) === source) {
      return;
    }
    if (this.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    if (source === ConversationSource.General) {
      this.stopPolling();
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      return;
    }
    this.persistVisibleGeneralChatDraft();
    const activeRemoteSessionId = this.remotePageState.activeSession.sessionId || '';
    const target = AppRouteContract.routeForConversationSource(
      source,
      RemoteUiState.canUseRemote(this.connectionState),
      activeRemoteSessionId
    );
    const hasActiveRemoteConversation = target.name === AppRoute.RemoteChat;
    this.appShellViewModel.replaceRouteWithoutAnimation(
      hasActiveRemoteConversation ? AppRoute.RemoteHome : target.name
    );
    if (hasActiveRemoteConversation) {
      this.startPolling();
      await this.loadActiveMessages();
    }
  }

  /**
   * Compact counterpart of switchWideConversationSource. Switching source is a
   * change of context, not a command to start something: it resumes the session
   * the user was last in, and otherwise rests on the Remote landing surface
   * rather than opening the create composer for them.
   */
  async switchCompactConversationSource(source: ConversationSource): Promise<void> {
    this.closeAppSidebar();
    if (AppRouteContract.conversationSource(this.currentRoute()) === source) {
      return;
    }
    if (this.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    if (source === ConversationSource.General) {
      this.stopPolling();
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      return;
    }
    this.persistVisibleGeneralChatDraft();
    const activeRemoteSessionId = RemoteUiState.canUseRemote(this.connectionState) ?
      (this.remotePageState.activeSession.sessionId || '') : '';
    if (activeRemoteSessionId.length === 0) {
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      return;
    }
    this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteChat, activeRemoteSessionId);
    this.startPolling();
    await this.loadActiveMessages();
  }

  private enterCompactLayout(): void {
    const sessionId = this.remotePageState.activeSession.sessionId || '';
    if (this.isRoute(AppRoute.RemoteHome) && sessionId.length > 0) {
      this.appShellViewModel.pushRoute(AppRoute.RemoteChat, sessionId, false);
    }
  }

  openAddConnection(): void {
    this.appShellState.setConnectSheetVisible(true);
  }

  openRemoteControlSettings(): void {
    setTimeout(() => {
      this.appShellState.openSettings('remote');
    }, 180);
  }

  openAddConnectionFromSettings(): void {
    this.appShellState.setSettingsVisible(false);
    setTimeout(() => {
      this.openAddConnection();
    }, 220);
  }

  async loginCloudAccount(relayUrl: string, username: string, password: string): Promise<string> {
    RemoteLogger.info('cloud account UI login requested');
    const session = await this.cloudAccountClient.login(relayUrl, username, password, this.identityStoreSnapshotInstallId());
    this.applyCloudAccountSession(session, relayUrl, username);
    await this.cloudAccountSessionStore.save({
      relayUrl: relayUrl.trim(), username: username.trim(), token: session.token, userId: session.userId,
      masterKey: Encoding.bytesToBase64(session.masterKey)
    });
    await this.loadGeneralChatAccountModels(session, relayUrl);
    RemoteLogger.info('cloud account credentials persisted, refreshing account devices');
    RemoteLogger.info(`cloud account login success user=${session.userId}`);
    return session.userId;
  }

  private async restoreCloudAccountSession(): Promise<void> {
    try {
      const persisted = await this.cloudAccountSessionStore.load();
      if (!persisted) return;
      const session: CloudAccountSession = {
        token: persisted.token,
        userId: persisted.userId,
        masterKey: Encoding.base64ToBytes(persisted.masterKey)
      };
      this.applyCloudAccountSession(session, persisted.relayUrl, persisted.username || session.userId);
      await this.loadGeneralChatAccountModels(session, persisted.relayUrl);
    } catch (err) {
      RemoteLogger.warn(`cloud account restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
      await this.cloudAccountSessionStore.clear();
    }
  }

  private async loadGeneralChatAccountModels(session: CloudAccountSession, relayUrl: string): Promise<void> {
    this.generalChatConfigStore.replaceAccountModels([]);
    try {
      const blob = await this.cloudAccountClient.fetchSettings(relayUrl, session);
      if (!blob) {
        this.generalChatConfigStore.replaceAccountModels([]);
        await this.refreshGeneralChatModelCatalog();
        RemoteLogger.info('cloud model catalog is empty');
        return;
      }
      const models = GeneralChatCloudConfigPolicy.models(blob.plaintext);
      this.generalChatConfigStore.replaceAccountModels(models);
      await this.refreshGeneralChatModelCatalog();
      RemoteLogger.info(`cloud model catalog loaded count=${models.length} version=${blob.version}`);
    } catch (err) {
      await this.refreshGeneralChatModelCatalog();
      RemoteLogger.warn(`cloud model catalog load failed: ${err instanceof Error ? err.message : 'unknown error'}`);
    }
  }

  async syncCloudAccount(): Promise<string> {
    if (!this.cloudAccountSession || this.cloudAccountRelayUrl.length === 0) {
      throw new Error(RemoteI18n.t('remote.settings.accountNotSignedIn'));
    }
    let bundles: Object[];
    try {
      bundles = await this.cloudAccountClient.fetchSessions(this.cloudAccountRelayUrl, this.cloudAccountSession, 0);
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
        throw new Error(RemoteI18n.t('remote.settings.accountExpired'));
      }
      throw new Error(err instanceof Error ? err.message : RemoteI18n.t('remote.settings.accountSyncFailed'));
    }
    await this.loadGeneralChatAccountModels(this.cloudAccountSession, this.cloudAccountRelayUrl);
    RemoteLogger.info(`cloud account backup sync completed count=${bundles.length}`);
    return String(bundles.length);
  }

  protected applyCloudAccountSession(session: CloudAccountSession, relayUrl: string, username: string): void {
    this.cloudAccountSession = session;
    this.cloudAccountRelayUrl = relayUrl.trim();
    this.remotePageState.setAccountUserId(session.userId);
    this.remotePageState.setAccountUsername(username.trim());
  }

  async logoutCloudAccount(): Promise<void> {
    this.invalidateFilePreviewTarget();
    if (this.remotePageState.controlTargetType === 'account_device') {
      this.remoteActivityViewModel.invalidate();
      this.stopPolling();
      this.stopHeartbeat();
      this.sessionManager.reset();
      this.remotePageState.clearActiveSession();
      this.remotePageState.setSessions([], false);
      this.remotePageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
      this.remotePageState.setConnectionState(ConnectionState.Disconnected);
      this.remotePageState.setAuthenticatedUserId('');
    }
    this.cloudAccountSession = undefined;
    this.cloudAccountRelayUrl = '';
    this.generalChatConfigStore.replaceAccountModels([]);
    await this.refreshGeneralChatModelCatalog();
    await this.cloudAccountSessionStore.clear();
    this.remotePageState.setAccountUserId('');
    this.remotePageState.setAccountUsername('');
    this.remotePageState.clearControlTarget();
    RemoteLogger.info('cloud account logout success');
  }

  async listCloudAccountDevices(): Promise<CloudAccountDevice[]> {
    if (!this.cloudAccountSession || this.cloudAccountRelayUrl.length === 0) {
      return [];
    }
    try {
      return await this.cloudAccountClient.listDevices(this.cloudAccountRelayUrl, this.cloudAccountSession);
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
        throw new Error(RemoteI18n.t('remote.settings.accountExpired'));
      }
      if (err instanceof CloudAccountRequestError &&
        (err.statusCode === 404 || err.statusCode === 503 || err.statusCode === 504)) {
        throw new Error(RemoteI18n.t('remote.settings.deviceUnavailable'));
      }
      throw new Error(err instanceof Error ? err.message : RemoteI18n.t('remote.settings.deviceLoadFailed'));
    }
  }

  async getRemotePermissionMode(): Promise<RemotePermissionMode> {
    if (!this.ensureRemoteAvailable()) {
      throw new Error(RemoteI18n.t('remote.permissions.connectionRequired'));
    }
    return this.sessionManager.getPermissionMode();
  }

  async setRemotePermissionMode(mode: RemotePermissionMode): Promise<RemotePermissionMode> {
    if (!this.ensureRemoteAvailable()) {
      throw new Error(RemoteI18n.t('remote.permissions.connectionRequired'));
    }
    return this.sessionManager.setPermissionMode(mode);
  }

  private async restoreCloudTarget(targetDeviceId: string, targetDeviceName: string): Promise<void> {
    const targetId = targetDeviceId.trim();
    if (targetId.length === 0) {
      return;
    }
    try {
      const devices = await this.listCloudAccountDevices();
      const target = devices.find((device: CloudAccountDevice): boolean => device.deviceId === targetId);
      if (!target || !target.online) {
        const targetName = target?.deviceName || targetDeviceName || targetId;
        this.remotePageState.setControlTarget('account_device', targetId, targetName);
        this.remotePageState.setDesktopIdentity(targetName, targetId);
        this.remotePageState.setConnectionState(ConnectionState.Failed);
        this.remotePageState.setStatusText(RemoteI18n.t('remote.settings.deviceUnavailable'));
        return;
      }
      await this.selectCloudAccountDevice({
        deviceId: target.deviceId,
        deviceName: target.deviceName || targetDeviceName || target.deviceId,
        online: target.online,
        lastSeenAt: target.lastSeenAt
      });
    } catch (err) {
      RemoteLogger.warn(`cloud target restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
    }
  }

  private async expireCloudAccountSession(): Promise<void> {
    this.invalidateFilePreviewTarget();
    this.cloudAccountSession = undefined;
    this.cloudAccountRelayUrl = '';
    await this.cloudAccountSessionStore.clear();
    this.remotePageState.setAccountUserId('');
    this.remotePageState.setAccountUsername('');
    if (this.remotePageState.controlTargetType === 'account_device') {
      this.remoteActivityViewModel.invalidate();
      this.stopPolling();
      this.stopHeartbeat();
      this.sessionManager.reset();
      this.remotePageState.clearActiveSession();
      this.remotePageState.setSessions([], false);
      this.remotePageState.clearControlTarget();
      this.remotePageState.setConnectionState(ConnectionState.Disconnected);
    }
  }

  private async handleRemoteConnectionError(err: Object): Promise<boolean> {
    if (this.remotePageState.controlTargetType !== 'account_device' ||
      !(err instanceof CloudAccountRequestError) || err.statusCode !== 401) {
      return false;
    }
    await this.expireCloudAccountSession();
    this.remotePageState.setStatusText(RemoteI18n.t('remote.settings.accountExpired'));
    return true;
  }

  async selectCloudAccountDevice(device: CloudAccountDevice, navigateHome: boolean = true): Promise<void> {
    if (!device.online) {
      throw new Error(RemoteI18n.t('remote.settings.deviceOffline'));
    }
    if (!this.cloudAccountSession || this.cloudAccountRelayUrl.length === 0) {
      throw new Error(RemoteI18n.t('remote.settings.accountNotSignedIn'));
    }
    const deviceId = device.deviceId.trim();
    if (deviceId.length === 0 || deviceId === this.remoteConnectionViewModel.getDeviceId()) {
      return;
    }
    if (deviceId === this.remotePageState.controlTargetDeviceId &&
      this.connectionState === ConnectionState.Connected) {
      this.appShellState.setConnectSheetVisible(false);
      if (navigateHome) {
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      }
      return;
    }
    this.invalidateFilePreviewTarget();
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.stopPolling();
    this.stopHeartbeat();
    // Do not keep presenting the previous device while the new account device
    // is being handshaken. Clear its projection before the async connect.
    this.remotePageState.setConnectionState(ConnectionState.Reconnecting);
    this.remotePageState.setLoadingHome(true);
    this.remotePageState.clearControlTarget();
    this.remotePageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    this.remotePageState.setBusy(true);
    this.remotePageState.setStatusText(RemoteI18n.t('remote.settings.deviceConnecting'));
    this.remotePageState.clearActiveSession();
    this.resetChatTimeline('');
    this.knownPollVersion = 0;
    this.knownModelCatalogVersion = 0;
    this.knownRemoteMessageCount = 0;
    this.remotePageState.setSessions([], false);
    try {
      const initialSync = await this.sessionManager.connectAccountDevice(
        this.cloudAccountClient,
        this.cloudAccountRelayUrl,
        this.cloudAccountSession,
        deviceId
      );
      this.remotePageState.setControlTarget('account_device', deviceId, device.deviceName);
      this.remotePageState.setDesktopIdentity(device.deviceName, deviceId);
      this.remotePageState.setWorkspace(
        initialSync.workspace.name,
        initialSync.workspace.path,
        initialSync.workspace.assistantId || '',
        initialSync.workspace.gitBranch,
        initialSync.workspace.workspaceKind || 'normal'
      );
      this.remotePageState.setSessions(initialSync.sessions, initialSync.hasMoreSessions);
      this.remotePageState.setAuthenticatedUserId(initialSync.authenticatedUserId);
      this.remotePageState.setConnectionState(ConnectionState.Connected);
      this.remotePageState.setStatusText(RemoteI18n.t('connection.connected'));
      this.appShellState.setSettingsVisible(false);
      this.appShellState.setConnectSheetVisible(false);
      if (navigateHome) {
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      }
      await this.cloudAccountSessionStore.save({
        relayUrl: this.cloudAccountRelayUrl,
        username: this.remotePageState.accountUsername,
        token: this.cloudAccountSession.token,
        userId: this.cloudAccountSession.userId,
        masterKey: Encoding.bytesToBase64(this.cloudAccountSession.masterKey),
        targetDeviceId: deviceId,
        targetDeviceName: device.deviceName
      });
      this.startHeartbeat();
      await this.loadRecentWorkspacesInBackground();
      this.remotePageState.setLoadingHome(false);
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
      }
      this.remotePageState.clearControlTarget();
      this.remotePageState.setConnectionState(ConnectionState.Failed);
      const message = ConnectionErrorPolicy.errorText(err);
      this.remotePageState.setStatusText(message);
      this.sessionManager.reset();
      throw new Error(message);
    } finally {
      this.remotePageState.setLoadingHome(false);
      this.remotePageState.setBusy(false);
    }
  }

  private identityStoreSnapshotInstallId(): string {
    return this.remoteConnectionViewModel.getDeviceId();
  }

  openHomeSession(session: RemoteSession): void {
    this.closeFilePreview();
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openSession(session);
  }

  openHomeSessionInPlace(session: RemoteSession): void {
    this.closeFilePreview();
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openSession(session, true);
  }

  async deleteHomeSession(session: RemoteSession): Promise<void> {
    if (session.agentType !== 'chat') {
      await this.deleteSession(session);
      return;
    }
    await this.generalChatCommandController.deleteSession(session, this.generalChatPageState.isBusy);
  }

  activeGeneralChatAsRemoteSession(): RemoteSession {
    const active = this.generalChatPageState.activeSession;
    return {
      id: active.sessionId,
      title: active.title,
      agentType: 'chat',
      status: 'ready',
      updatedAt: '',
      createdAt: '',
      messageCount: this.generalChatPageState.timelineItems.length,
      workspacePath: active.workspacePath
    };
  }

  activeGeneralUploadedFileCount(): number {
    let count = 0;
    this.generalChatPageState.timelineItems.forEach((item: ChatTimelineItem) => {
      if (item.message && item.message.images) {
        count += item.message.images.length;
      }
    });
    return count;
  }

  async archiveHomeSession(session: RemoteSession, archived: boolean): Promise<void> {
    await this.generalChatCommandController.archiveSession(session, archived, this.generalChatPageState.isBusy);
  }

  async exportHomeSession(session: RemoteSession): Promise<void> {
    await this.generalChatCommandController.exportSession(
      session,
      this.generalChatPageState.isBusy,
      async (text: string): Promise<void> => {
        await this.clipboardService.writeText(text);
      }
    );
  }

  async openGeneralSession(item: RemoteSession): Promise<void> {
    if (this.generalChatPageState.isBusy) {
      return;
    }
    this.stopPolling();
    this.stopGeneralChatStream(false);
    await this.generalChatCommandController.openSession(
      item,
      this.generalChatPageState.isBusy,
      async (sessionId: string): Promise<string> => {
        return this.generalChatDraftLifecycleController.restore(sessionId);
      },
      (_sessionId: string) => {
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      }
    );
  }

  async startGeneralChat(text: string): Promise<void> {
    const trimmed = text.trim();
    if (trimmed.length === 0 || this.generalChatPageState.isBusy) {
      return;
    }
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.generalChatDraftLifecycleController.cancel();
    const created = await this.generalChatCommandController.createSession(
      trimmed,
      this.generalChatPageState.isBusy,
      async (): Promise<void> => {
        await this.generalChatDraftLifecycleController.clearHomeNow();
      },
      (_sessionId: string) => {
        this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      }
    );
    if (!created) {
      return;
    }
    await this.sendGeneralChatMessage();
  }

  async sendVisibleChatMessage(): Promise<void> {
    if (this.isGeneralChatVisible()) {
      if ((this.generalChatPageState.activeSession.sessionId || '').length === 0) {
        this.startVisibleGeneralChat();
        return;
      }
      await this.sendGeneralChatMessage();
      return;
    }
    await this.sendChatMessage();
  }

  async stopActiveChatTask(): Promise<void> {
    if (this.isGeneralChatVisible()) {
      this.stopGeneralChatStream(true);
      return;
    }
    await this.stopActiveTask();
  }

  closeActiveChat(): void {
    this.closeFilePreview();
    this.stopVoiceInput(false);
    if (this.isRoute(AppRoute.GeneralChat)) {
      this.persistVisibleGeneralChatDraft();
      this.stopGeneralChatStream(true);
      this.popRoute(AppRoute.ChatHome);
      this.restoreGeneralChatDraft(GENERAL_CHAT_HOME_DRAFT_ID);
      return;
    }
    this.stopPolling();
    this.popRoute(AppRoute.RemoteHome);
  }

  async renameVisibleSession(title: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      await this.generalChatCommandController.renameActiveSession(
        this.generalChatPageState.activeSession,
        title
      );
      return;
    }
    await this.renameActiveSession(title);
  }

  async retryVisibleMessage(text: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      const sessionId = this.generalChatPageState.activeSession.sessionId || '';
      const prepared = await this.generalChatCommandController.retryMessage(
        sessionId,
        text,
        this.generalChatPageState.isBusy
      );
      if (prepared) {
        await this.sendGeneralChatMessage();
      }
      return;
    }
    this.retryMessage(text);
  }

  downloadVisibleFile(path: string): void {
    if (this.isGeneralChatVisible()) {
      this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.fileDownloadMock'));
      return;
    }
    this.downloadFile(path);
  }

  openFilePreview(route: AppRoute, request: FilePreviewRequest): void {
    const context = new FilePreviewTargetContext(
      this.remotePageState.activeSession.sessionId,
      this.remotePageState.activeSession.workspacePath || this.remotePageState.workspacePath,
      this.controlTargetEpoch
    );
    const resolution = FileTargetResolver.resolve(request.reference, request.label, context);
    if (resolution.kind === FileReferenceKind.HttpUrl) {
      void this.openExternalLink(route, request.reference);
      return;
    }
    if (route !== AppRoute.RemoteChat) {
      this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.filePreviewUnavailable'));
      return;
    }
    if (resolution.kind !== FileReferenceKind.RemoteWorkspaceFile || !resolution.target) {
      return;
    }
    void this.remoteFilePreviewController.open(resolution.target);
  }

  private async openExternalLink(route: AppRoute, reference: string): Promise<void> {
    const opened = this.host.openExternalLink ? await this.host.openExternalLink(reference) : false;
    if (!opened) {
      if (AppRouteContract.isGeneralComposerRoute(route)) {
        this.generalChatPageState.setStatus(RemoteI18n.t('errors.operationFailed'));
      } else {
        this.setRemoteStatusText(RemoteI18n.t('errors.operationFailed'));
      }
    }
  }

  closeFilePreview(): void {
    this.remoteFilePreviewController.close();
  }

  refreshFilePreview(): void {
    void this.remoteFilePreviewController.refresh();
  }

  openFilePreviewLink(reference: string, label: string): void {
    this.openFilePreview(AppRoute.RemoteChat, new FilePreviewRequest(reference, label));
  }

  invalidateFilePreviewTarget(): void {
    this.controlTargetEpoch += 1;
    this.remoteFilePreviewController.close();
  }

  async createSession(agentType: string, inPlace: boolean = false): Promise<void> {
    this.closeFilePreview();
    await this.remoteSessionViewModel.createSession(
      agentType,
      '',
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  openRemoteCreateSession(): void {
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    const deviceId = this.remotePageState.controlTargetDeviceId || this.remotePageState.desktopId;
    const deviceName = this.remotePageState.controlTargetDeviceName || this.remotePageState.desktopName;
    this.remoteCreateState.prepare(deviceId, deviceName, this.remotePageState.selectedModelId);
    if (deviceId.length > 0) {
      this.remoteCreateState.setDevices([{
        deviceId,
        deviceName: deviceName || deviceId,
        online: true
      }]);
    }
    this.remoteCreateState.setWorkspaces(this.remotePageState.recentWorkspaces);
    this.pushRoute(AppRoute.RemoteCreate);
    this.loadRemoteCreateChoices();
    this.loadRemoteCreateModelCatalog();
  }

  closeRemoteCreateSession(): void {
    this.remoteCreateWorkspaceLoadVersion += 1;
    this.stopVoiceInput(false);
    this.remoteCreateState.closeMenu();
    this.popRoute(AppRoute.RemoteHome);
  }

  async loadRemoteCreateChoices(): Promise<void> {
    await Promise.all([
      this.loadRemoteCreateDevices(),
      this.loadRemoteCreateWorkspaces()
    ]);
  }

  async loadRemoteCreateModelCatalog(): Promise<void> {
    if (this.remotePageState.modelCatalog.models.length > 0) {
      return;
    }
    try {
      const catalog = await this.sessionManager.getModelCatalog();
      const selectedModelId = RemoteUiState.selectedModelIdForCatalog(
        catalog,
        this.remotePageState.selectedModelId
      );
      this.remotePageState.setModelCatalog(catalog, selectedModelId);
      this.remoteCreateState.setSelectedModelId(selectedModelId);
    } catch (_err) {
      // Model selection remains hidden when the remote does not expose a catalog.
    }
  }

  async loadRemoteCreateDevices(): Promise<void> {
    this.remoteCreateState.isLoadingDevices = this.remoteCreateState.devices.length === 0;
    try {
      const phoneDeviceId = this.remoteConnectionViewModel.getDeviceId();
      const accountDevices = await this.listCloudAccountDevices();
      const devices = accountDevices.filter((device: CloudAccountDevice): boolean =>
        device.online && device.deviceId !== phoneDeviceId
      );
      const currentId = this.remoteCreateState.selectedDeviceId;
      if (currentId.length > 0 && !devices.some((device: CloudAccountDevice): boolean => device.deviceId === currentId)) {
        devices.unshift({
          deviceId: currentId,
          deviceName: this.remoteCreateState.selectedDeviceName || currentId,
          online: true
        });
      }
      this.remoteCreateState.setDevices(devices);
    } catch (err) {
      const currentId = this.remoteCreateState.selectedDeviceId;
      if (currentId.length > 0) {
        this.remoteCreateState.setDevices([{
          deviceId: currentId,
          deviceName: this.remoteCreateState.selectedDeviceName || currentId,
          online: true
        }]);
      } else {
        this.remoteCreateState.setDevices([]);
      }
      this.remoteCreateState.errorText = RemoteI18n.t('remote.create.deviceLoadFailed');
    }
  }

  async loadRemoteCreateWorkspaces(): Promise<void> {
    const loadVersion = ++this.remoteCreateWorkspaceLoadVersion;
    const deviceId = this.remoteCreateState.selectedDeviceId;
    this.remoteCreateState.isLoadingWorkspaces = this.remoteCreateState.workspaces.length === 0;
    try {
      const workspaces = await this.workspaceCoordinator.recentWorkspaces();
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreateState.selectedDeviceId) {
        return;
      }
      this.remoteCreateState.setWorkspaces(workspaces);
    } catch (err) {
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreateState.selectedDeviceId) {
        return;
      }
      this.remoteCreateState.setWorkspaces([]);
      this.remoteCreateState.errorText = RemoteI18n.t('remote.create.workspaceLoadFailed');
    }
  }

  toggleRemoteCreateDevices(): void {
    this.remoteCreateState.toggleMenu('devices');
    if (this.remoteCreateState.openMenu === 'devices' && this.remoteCreateState.devices.length === 0) {
      this.loadRemoteCreateDevices();
    }
  }

  toggleRemoteCreateWorkspaces(): void {
    this.remoteCreateState.toggleMenu('workspaces');
    if (this.remoteCreateState.openMenu === 'workspaces' && this.remoteCreateState.workspaces.length === 0) {
      this.loadRemoteCreateWorkspaces();
    }
  }

  async selectRemoteCreateDevice(device: CloudAccountDevice): Promise<void> {
    if (device.deviceId === this.remoteCreateState.selectedDeviceId) {
      this.remoteCreateState.closeMenu();
      return;
    }
    const draft = this.remoteCreateState.draft;
    this.remoteCreateState.closeMenu();
    this.remoteCreateState.isLoadingWorkspaces = true;
    try {
      await this.selectCloudAccountDevice(device, false);
      this.remoteCreateState.selectDevice(device);
      this.remoteCreateState.setDraft(draft);
      await this.loadRemoteCreateWorkspaces();
    } catch (err) {
      this.remoteCreateState.isLoadingWorkspaces = false;
      this.remoteCreateState.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.settings.deviceSwitchFailed');
    }
  }

  selectRemoteCreateWorkspace(path: string): void {
    const workspace = this.remoteCreateState.workspaces.find((item: RecentWorkspaceEntry): boolean => item.path === path);
    this.remoteCreateState.selectWorkspace(workspace);
  }

  selectRemoteCreateModel(modelId: string): void {
    this.remoteCreateState.setSelectedModelId(modelId);
  }

  async submitRemoteCreateSession(): Promise<void> {
    const instruction = this.remoteCreateState.draft.trim();
    if (instruction.length === 0 || this.remoteCreateState.isSubmitting || !this.ensureRemoteAvailable()) {
      return;
    }
    const context = this.remoteCreateState.submissionContext();
    const activeDeviceId = this.remotePageState.controlTargetDeviceId || this.remotePageState.desktopId;
    if (context.deviceId.length === 0 || context.deviceId !== activeDeviceId) {
      this.remoteCreateState.errorText = RemoteI18n.t('remote.create.deviceMismatch');
      return;
    }
    this.remoteCreateState.isSubmitting = true;
    this.remoteCreateState.errorText = '';
    this.remoteCreateState.closeMenu();
    try {
      if (context.workspacePath.length > 0) {
        await this.remoteSessionViewModel.createSessionInWorkspace(
          context.workspacePath,
          this.workspacePath,
          instruction,
          context.agentType,
          undefined,
          this.remoteCreateState.selectedModelId
        );
      } else {
        await this.remoteSessionViewModel.createSession(
          context.agentType,
          instruction,
          undefined,
          this.remoteCreateState.selectedModelId
        );
      }
      if (this.isRoute(AppRoute.RemoteCreate)) {
        this.remoteCreateState.errorText = this.statusText || RemoteI18n.t('remote.create.submitFailed');
      }
    } catch (err) {
      this.remoteCreateState.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.create.submitFailed');
    } finally {
      this.remoteCreateState.isSubmitting = false;
    }
  }

  async createSessionInWorkspace(
    path: string,
    agentType: string = 'code',
    inPlace: boolean = false
  ): Promise<void> {
    this.closeFilePreview();
    await this.remoteSessionViewModel.createSessionInWorkspace(
      path,
      this.workspacePath,
      '',
      agentType,
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  applyDiscoveredWorkspaceSessions(all: RemoteSession[]): void {
    this.remoteWorkspaceSessions = all;
    const current = this.remotePageState.sessions;
    const extras = all.filter((item: RemoteSession) => item.workspacePath !== this.workspacePath);
    this.remotePageState.setSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  async loadRecentWorkspacesInBackground(): Promise<void> {
    await this.remoteWorkspaceViewModel.loadRecentWorkspacesInBackground();
  }

  mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[] {
    const merged = primary.slice();
    extras.forEach((item: RemoteSession) => {
      if (!merged.some((existing: RemoteSession) => existing.id === item.id)) {
        merged.push(item);
      }
    });
    return merged;
  }

  async openSession(item: RemoteSession, inPlace: boolean = false): Promise<void> {
    this.closeFilePreview();
    await this.remoteSessionViewModel.openSession(
      item,
      this.workspacePath,
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  applyRemoteActiveSession(session: SessionSummary): void {
    const current = this.remotePageState.activeSession;
    if (this.filePreviewState.visible &&
      (current.sessionId !== session.sessionId || current.workspacePath !== session.workspacePath)) {
      this.closeFilePreview();
    }
    this.remotePageState.setActiveSession(session);
  }

  async deleteSession(item: RemoteSession): Promise<void> {
    await this.remoteSessionViewModel.deleteSession(item, this.workspacePath);
  }

  async loadActiveMessages(): Promise<void> {
    await this.remoteSessionViewModel.loadActiveMessages((activeSessionId: string): boolean => {
      return this.isRemoteConversationContext(activeSessionId);
    });
  }

  async loadModelCatalog(sessionId: string): Promise<void> {
    await this.remoteSessionViewModel.loadModelCatalog(sessionId, (activeSessionId: string): boolean => {
      return this.isRemoteConversationContext(activeSessionId);
    });
  }

  async selectModel(modelId: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      if (await this.generalChatConfigStore.selectModel(modelId)) {
        await this.refreshGeneralChatModelCatalog();
      }
      return;
    }
    await this.remoteSessionViewModel.selectModel(modelId);
  }

  async loadOlderMessages(): Promise<void> {
    await this.remoteSessionViewModel.loadOlderMessages(this.knownPollVersion);
  }

  async sendGeneralChatMessage(): Promise<void> {
    await this.generalChatConversationViewModel.sendMessage();
    return;
  }

  stopGeneralChatStream(cancelled: boolean, finalStatus: string = 'cancelled'): void {
    this.generalChatConversationViewModel.stop(cancelled, finalStatus);
    return;
  }

  async sendChatMessage(): Promise<void> {
    if (this.remotePageState.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    const rawText = this.remotePageState.chatInput.trim();
    const images = this.remotePageState.selectedImages.slice();
    const text = rawText.length > 0 ? rawText : (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.remotePageState.clearComposer();
    const localMessage = RemoteUiState.localUserMessage(text, images);
    this.chatTimelineStore.appendOptimisticMessage(localMessage);
    const pendingActiveId = this.chatTimelineStore.setPendingActiveTurn(localMessage.id);
    this.syncChatTimelineFromStore();
    RemoteLogger.info(`chat send queued session=${this.shortSessionId(sessionId)} pending=${pendingActiveId}`);
    this.startPolling();
    this.nudgeChatPolling();
    const imageContexts: RemoteImageContext[] = images.length > 0 ? this.imagePickerService.toRemoteContexts(images) : [];
    await this.remoteChatCommandController.sendPreparedMessage(
      sessionId,
      text,
      this.activeSession.agentType,
      rawText,
      images,
      imageContexts,
      localMessage.id,
      pendingActiveId,
      this.isBusy,
      true
    );
  }

  async toggleVoiceInput(): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.toggle(this.host.context(), this.voiceInputSnapshot(route));
  }

  async stopVoiceInput(showStatus: boolean): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.stop(this.voiceInputSnapshot(route), showStatus);
  }

  showVoiceInputError(message: string): void {
    const text = message.length > 0 ? message : RemoteI18n.t('errors.voiceInputUnavailable');
    this.setVisibleStatusText(text);
    this.host.showToast(text, 2600);
  }

  async pickImages(): Promise<void> {
    if (this.visibleChatBusy()) {
      return;
    }
    const route = this.currentRoute();
    if (this.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    try {
      this.setVisibleStatusText(RemoteI18n.t('status.pickImage'));
      const picked = await this.imagePickerService.pickImages(3, this.visibleSelectedImages().length);
      if (picked.length === 0) {
        this.setVisibleStatusText(RemoteI18n.t('status.noImageSelected'));
        return;
      }
      this.addSelectedImagesForRoute(route, picked);
      this.setVisibleStatusText(RemoteI18n.f('status.imagesSelected', `${this.visibleSelectedImages().length}`));
    } catch (err) {
      this.setVisibleStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  removeSelectedImage(imageId: string): void {
    this.removeSelectedImageForRoute(this.currentRoute(), imageId);
  }

  startVisibleGeneralChat(): void {
    const rawText = this.generalChatPageState.chatInput.trim();
    const text = rawText.length > 0 ? rawText :
      (this.generalChatPageState.selectedImages.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    if (text.length === 0 || this.generalChatPageState.isBusy) {
      return;
    }
    if (this.generalChatPageState.serviceState === GeneralChatServiceState.Unconfigured) {
      const statusText = GeneralChatServiceStatus.userMessage(this.generalChatPageState.serviceState);
      this.generalChatPageState.setStatus(statusText);
      this.showHomeToast(statusText);
      return;
    }
    this.startGeneralChat(text);
  }

  generalChatHomeStatusText(): string {
    if (this.generalChatPageState.serviceState === GeneralChatServiceState.Ready ||
      this.generalChatPageState.serviceState === GeneralChatServiceState.Sending ||
      this.generalChatPageState.serviceState === GeneralChatServiceState.Streaming) {
      return '';
    }
    return GeneralChatServiceStatus.userMessage(
      this.generalChatPageState.serviceState,
      this.generalChatPageState.statusText
    );
  }

  prepareNewGeneralChat(): void {
    this.stopVoiceInput(false);
    this.stopGeneralChatStream(true);
    this.generalChatDraftLifecycleController.clearHome();
    this.generalChatPageState.clearComposer();
    this.generalChatPageState.clearActiveSession();
    this.resetGeneralChatTimeline('');
    this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
  }

  onVisibleChatInputChange(route: AppRoute, value: string): void {
    this.setChatInputForRoute(route, value);
    if (!this.isGeneralComposerRoute(route)) {
      return;
    }
    this.generalChatDraftLifecycleController.scheduleVisible(value);
  }

  visibleGeneralChatDraftId(): string {
    if (this.isGeneralChatVisible()) {
      return this.generalChatPageState.activeSession.sessionId || GENERAL_CHAT_HOME_DRAFT_ID;
    }
    return '';
  }

  persistVisibleGeneralChatDraft(): void {
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
  }

  async restoreGeneralChatDraft(draftId: string): Promise<void> {
    this.generalChatPageState.setChatInput(await this.generalChatDraftLifecycleController.restore(draftId));
  }

  latestUserMessageText(): string {
    if (this.isGeneralChatVisible()) {
      return this.generalChatPageState.latestUserMessageText();
    }
    const candidates = this.messages.concat(this.pendingMessages);
    for (let index = candidates.length - 1; index >= 0; index--) {
      if (candidates[index].role === 'user' && candidates[index].text.trim().length > 0) {
        return candidates[index].text;
      }
    }
    return '';
  }

  showHomeToast(message: string): void {
    if (!this.host.showToast(message, 2600)) {
      this.setVisibleStatusText(message);
    }
  }

  async stopActiveTask(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    await this.remoteChatCommandController.stopTask(
      sessionId,
      this.activeTurnMessage.id,
      this.currentActiveTurnId(),
      this.ensureRemoteAvailable()
    );
  }

  async renameActiveSession(title: string): Promise<void> {
    const nextTitle = title.trim();
    if (
      !this.activeSession.sessionId ||
        nextTitle.length === 0 ||
        nextTitle === this.activeSession.title ||
        this.isBusy
    ) {
      return;
    }
    await this.remoteChatCommandController.renameActiveSession(
      this.activeSession,
      nextTitle,
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  async copyMessage(text: string): Promise<void> {
    if (text.trim().length === 0) {
      return;
    }
    try {
      await this.clipboardService.writeText(text);
      this.setRemoteStatusText(RemoteI18n.t('status.messageCopied'));
    } catch (err) {
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async downloadFile(path: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    await this.remoteFileDownloadController.download(path, sessionId, this.isBusy, this.ensureRemoteAvailable());
  }

  retryMessage(text: string): void {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.remotePageState.setChatInput(text);
    this.sendChatMessage();
  }

  async approveTool(toolId: string, updatedInput?: Object): Promise<void> {
    await this.remoteToolActionController.approve(
      toolId,
      this.activeSession.sessionId || '',
      this.ensureRemoteAvailable(),
      updatedInput
    );
  }

  async rejectTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.reject(
      toolId,
      this.activeSession.sessionId || '',
      this.ensureRemoteAvailable()
    );
  }

  async cancelTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.cancel(
      toolId,
      this.activeSession.sessionId || '',
      this.ensureRemoteAvailable()
    );
  }

  async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    await this.remoteToolActionController.answer(
      toolId,
      this.activeSession.sessionId || '',
      this.ensureRemoteAvailable(),
      answers
    );
  }

  resetChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.knownPollVersion = 0;
    this.syncChatTimelineFromStore();
  }

  resetGeneralChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.syncGeneralChatTimelineFromStore();
  }

  syncChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshotState();
    const projectedItems: ChatTimelineItem[] = this.projectedTimelineItems();
    this.remotePageState.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      this.hasMoreMessages,
      projectedItems
    );
    this.remotePageState.setModelCatalog(state.modelCatalog, state.selectedModelId);
  }

  syncGeneralChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshotState();
    const projectedItems = this.chatTimelineStore.viewState(false);
    this.generalChatPageState.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      false,
      projectedItems
    );
    const itemSummary = projectedItems.map((item: ChatTimelineItem) => {
      const message = item.message;
      return `${item.type}:${item.id}:${message ? message.status : ''}:${message ? message.text.length : 0}`;
    }).join(',');
    RemoteLogger.info(`general chat projection revision=${this.generalChatPageState.timelineRevision} persisted=${state.persistedMessages.length} active=${state.activeTurn ? state.activeTurn.id : 'none'} items=${itemSummary}`);
  }

  startPolling(): void {
    this.remoteChatPollingLifecycleController.startActiveSession({
      sessionId: this.activeSession.sessionId || '',
      cursor: this.currentChatPollingCursor(),
      activeTurn: this.activeTurnMessage
    });
  }

  stopPolling(): void {
    this.remoteChatPollingLifecycleController.stop();
  }

  nudgeChatPolling(): void {
    this.remoteChatPollingLifecycleController.nudge();
  }

  async pollActiveSession(): Promise<void> {
    await this.remoteChatPollingLifecycleController.pollNow();
  }

  currentChatPollingCursor(): RemoteChatPollingCursor {
    return {
      pollVersion: this.knownPollVersion,
      knownMessageCount: this.knownRemoteMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    };
  }

  updateChatPollingCursor(pollVersion: number, knownMessageCount: number): void {
    this.knownPollVersion = pollVersion;
    this.knownRemoteMessageCount = knownMessageCount;
    this.remoteChatPollingLifecycleController.updateCursor({
      pollVersion,
      knownMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    });
  }

  applyChatSessionSnapshot(snapshot: RemoteChatPollingSnapshot): void {
    if (!this.isRemoteConversationContext(snapshot.sessionId)) {
      return;
    }
    this.chatTimelineStore.applySnapshot(snapshot);
    this.syncChatTimelineFromStore();
    this.knownPollVersion = snapshot.cursor.pollVersion;
    this.knownModelCatalogVersion = snapshot.cursor.knownModelCatalogVersion;
    this.knownRemoteMessageCount = snapshot.cursor.knownMessageCount;
    if (snapshot.title.length > 0) {
      this.remotePageState.setActiveSession({
        sessionId: this.activeSession.sessionId,
        title: snapshot.title,
        workspacePath: this.activeSession.workspacePath,
        agentType: this.activeSession.agentType
      });
    }
    if (snapshot.modelCatalog) {
      this.remoteModelController.applyCatalog(snapshot.modelCatalog);
    }
    this.setRemoteStatusText(this.hasRunningActiveTurn()
      ? RemoteI18n.t('status.desktopProcessing')
      : RemoteI18n.t('status.messagesSynced'));
    if (snapshot.shouldSyncAfterTurnEnded) {
      this.syncAfterTurnEnded();
    }
  }

  hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  currentActiveTurnId(): string {
    const activeTurnMessage = this.isGeneralChatVisible() ?
      this.generalChatPageState.activeTurnMessage : this.activeTurnMessage;
    if (activeTurnMessage.turnId && activeTurnMessage.turnId.length > 0) {
      return activeTurnMessage.turnId;
    }
    const activePrefix = 'active-';
    if (activeTurnMessage.id.indexOf(activePrefix) === 0) {
      return activeTurnMessage.id.slice(activePrefix.length);
    }
    return '';
  }

  projectedTimelineItems(): ChatTimelineItem[] {
    return this.chatTimelineStore.viewState(this.hasMoreMessages);
  }

  startHeartbeat(): void {
    this.remoteActivityViewModel.startHeartbeat();
  }

  stopHeartbeat(): void {
    this.remoteActivityViewModel.stopHeartbeat();
  }

  async checkConnectionHealth(): Promise<void> {
    await this.remoteActivityViewModel.checkConnectionHealth();
  }

  resumeRemoteActivity(): void {
    this.remoteActivityViewModel.resume();
  }

  hasRemoteBindingForResume(): boolean {
    if (this.remotePageState.controlTargetType === 'account_device') {
      return this.remotePageState.accountUserId.trim().length > 0 &&
        this.remotePageState.controlTargetDeviceId.trim().length > 0 &&
        this.connectionState !== ConnectionState.Idle &&
        this.connectionState !== ConnectionState.Disconnected;
    }
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      this.connectionState !== ConnectionState.Idle &&
      this.connectionState !== ConnectionState.Parsing &&
      this.connectionState !== ConnectionState.Pairing &&
      this.connectionState !== ConnectionState.Disconnected;
  }

  private async reconnectActiveRemote(): Promise<void> {
    if (this.remotePageState.controlTargetType !== 'account_device') {
      await this.connect(true);
      return;
    }
    const targetId = this.remotePageState.controlTargetDeviceId;
    const device = (await this.listCloudAccountDevices()).find((item: CloudAccountDevice): boolean => item.deviceId === targetId);
    if (!device) {
      throw new Error(RemoteI18n.t('remote.settings.deviceOffline'));
    }
    await this.selectCloudAccountDevice(device);
  }

  hasRemoteBindingForCodeHome(): boolean {
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      (this.connectionState === ConnectionState.Connected ||
        this.connectionState === ConnectionState.Reconnecting ||
        this.connectionState === ConnectionState.Pairing ||
        this.connectionState === ConnectionState.Parsing);
  }

  shortSessionId(sessionId: string): string {
    if (sessionId.length <= 8) {
      return sessionId;
    }
    return sessionId.slice(0, 4) + '...' + sessionId.slice(sessionId.length - 4);
  }

  async syncAfterTurnEnded(): Promise<void> {
    if (this.isSyncingAfterTurn) {
      return;
    }
    this.isSyncingAfterTurn = true;
    try {
      await this.loadActiveMessages();
    } finally {
      this.isSyncingAfterTurn = false;
    }
  }

}
