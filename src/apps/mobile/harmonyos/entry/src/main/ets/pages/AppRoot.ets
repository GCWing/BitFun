import {
  ChatMessage,
  ChatMessageItemResponse,
  InitialSyncResult,
  RecentWorkspaceEntry,
  RemoteModelCatalog,
  RemoteImageContext,
  RemoteQuestionAnswerPayload,
  RemoteSession,
  RemoteToolStatusResponse,
  SelectedImageAttachment,
  SessionSummary,
  WorkspaceInfo
} from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ClipboardService } from '../services/ClipboardService';
import { ChatTimelineItem } from '../services/ChatTimelineProjector';
import { ChatTimelineState, ChatTimelineStore } from '../services/ChatTimelineStore';
import { ConnectionErrorPolicy, ConnectionErrorResult } from '../services/ConnectionErrorPolicy';
import { Encoding } from '../services/Encoding';
import { ImagePickerService } from '../services/ImagePickerService';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore,
  GeneralChatConfigUpdate
} from '../services/general-chat/GeneralChatConfigStore';
import { GeneralChatBootstrapController } from '../services/general-chat/GeneralChatBootstrapController';
import { GeneralChatCommandController } from '../services/general-chat/GeneralChatCommandController';
import { GeneralChatController } from '../services/general-chat/GeneralChatController';
import { GeneralChatDraftController } from '../services/general-chat/GeneralChatDraftController';
import { GeneralChatDraftLifecycleController } from '../services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatStreamLifecycleController } from '../services/general-chat/GeneralChatStreamLifecycleController';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../services/general-chat/GeneralChatServiceState';
import {
  GeneralChatSendResult,
  GeneralChatStreamCallbacks
} from '../services/general-chat/GeneralChatPort';
import { MobileIdentitySnapshot, MobileIdentityStore } from '../services/MobileIdentityStore';
import { RemoteDescriptorParser } from '../services/RemoteDescriptorParser';
import { AppRootRouteState } from '../services/AppRootRouteState';
import { RemoteActivityLifecycleController } from '../services/RemoteActivityLifecycleController';
import { RemoteChatCommandController } from '../services/RemoteChatCommandController';
import {
  RemoteChatPollingCursor,
  RemoteChatPollingLifecycleController,
  RemoteChatPollingSnapshot
} from '../services/RemoteChatPollingLifecycleController';
import { RemoteFileDownloadController } from '../services/RemoteFileDownloadController';
import { RemoteLogger } from '../services/RemoteLogger';
import { RemoteModelController } from '../services/RemoteModelController';
import { RemotePairingPolicy, RemotePairingProjection } from '../services/RemotePairingPolicy';
import { RemoteSessionController } from '../services/RemoteSessionController';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { RemoteToolActionController } from '../services/RemoteToolActionController';
import { QrScanService } from '../services/QrScanService';
import { RemoteUiState } from '../services/RemoteUiState';
import {
  VoiceInputLifecycleController,
  VoiceInputRouteSnapshot
} from '../services/VoiceInputLifecycleController';
import { VoiceInputService } from '../services/VoiceInputService';
import { AppShell } from './components/AppShell';
import { AppSidebar } from './components/AppSidebar';
import {
  GENERAL_CHAT_COMPOSER_CAPABILITIES,
  REMOTE_CHAT_COMPOSER_CAPABILITIES
} from './components/ChatComposerCapabilities';
import { ChatSurface } from './components/ChatSurface';
import { ConversationView } from './components/ConversationView';
import { ConnectView } from './components/ConnectView';
import { RemoteHomeView } from './components/RemoteHomeView';
import { RemoteControlSettingsSheet } from './components/RemoteControlSettingsSheet';
import { SettingsSheet } from './components/SettingsSheet';
import { PAGE_BG } from './components/Theme';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract
} from './navigation/AppRouteContract';
import { AppShellState } from './state/AppShellState';
import { GeneralChatPageState } from './state/GeneralChatPageState';
import { RemotePageState } from './state/RemotePageState';

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

@Entry
@Component
struct AppRoot {
  private readonly sessionManager: RemoteSessionManager = new RemoteSessionManager();
  private readonly identityStore: MobileIdentityStore = new MobileIdentityStore();
  private readonly clipboardService: ClipboardService = new ClipboardService();
  private readonly qrScanService: QrScanService = new QrScanService();
  private readonly imagePickerService: ImagePickerService = new ImagePickerService();
  private readonly remotePairingPolicy: RemotePairingPolicy = new RemotePairingPolicy();
  private readonly generalChatConfigStore: GeneralChatConfigStore = new GeneralChatConfigStore();
  private readonly generalChatController: GeneralChatController =
    GeneralChatController.createDefault(this.generalChatConfigStore);
  private readonly generalChatDraftController: GeneralChatDraftController =
    new GeneralChatDraftController(
      this.generalChatController,
      GENERAL_CHAT_DRAFT_SAVE_DELAY_MS,
      (err: Error) => {
        RemoteLogger.warn(`general chat draft operation failed: ${ConnectionErrorPolicy.errorText(err)}`);
      }
    );
  private readonly generalChatDraftLifecycleController: GeneralChatDraftLifecycleController =
    new GeneralChatDraftLifecycleController(
      this.generalChatDraftController,
      GENERAL_CHAT_HOME_DRAFT_ID,
      (): string => this.visibleGeneralChatDraftId()
    );
  private readonly chatTimelineStore: ChatTimelineStore = new ChatTimelineStore();
  private readonly generalChatCommandController: GeneralChatCommandController =
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
  private readonly generalChatBootstrapController: GeneralChatBootstrapController =
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
  private readonly voiceInputService: VoiceInputService = new VoiceInputService();
  private readonly remoteActivityLifecycleController: RemoteActivityLifecycleController =
    new RemoteActivityLifecycleController(() => {
      this.checkConnectionHealth();
    });
  private deviceId: string = Encoding.randomId('harmony');
  private autoReconnectAttempted: boolean = false;
  private isSyncingAfterTurn: boolean = false;
  private knownPollVersion: number = 0;
  private knownModelCatalogVersion: number = 0;
  private knownRemoteMessageCount: number = 0;
  private readonly generalChatStreamLifecycleController: GeneralChatStreamLifecycleController =
    new GeneralChatStreamLifecycleController();
  private readonly generalChatPageState: GeneralChatPageState = new GeneralChatPageState();
  private readonly remotePageState: RemotePageState = new RemotePageState();
  private remoteWorkspaceSessions: RemoteSession[] = [];
  private readonly appShellState: AppShellState = new AppShellState();
  private readonly voiceInputLifecycleController: VoiceInputLifecycleController =
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
  private readonly remoteSessionController: RemoteSessionController =
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
          this.activeSession = session;
          this.remotePageState.setActiveSession(session);
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
  private readonly remoteChatCommandController: RemoteChatCommandController =
    new RemoteChatCommandController(
      this.sessionManager,
      {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.hasMoreMessages = hasMoreMessages;
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
          this.activeSession = session;
          this.remotePageState.setActiveSession(session);
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
  private readonly remoteFileDownloadController: RemoteFileDownloadController =
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
  private readonly remoteToolActionController: RemoteToolActionController =
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
  private readonly remoteChatPollingLifecycleController: RemoteChatPollingLifecycleController =
    new RemoteChatPollingLifecycleController(
      this.sessionManager,
      {
        canPoll: (sessionId: string) => {
          return this.activeSession.sessionId === sessionId &&
            this.isRoute(AppRoute.RemoteChat) &&
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
  private readonly remoteModelController: RemoteModelController =
    new RemoteModelController(
      this.sessionManager,
      this.identityStore,
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.modelCatalog = modelCatalog;
        this.knownModelCatalogVersion = knownModelCatalogVersion;
        this.remoteChatPollingLifecycleController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.modelCatalog = modelCatalog;
        this.selectedModelId = selectedModelId;
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
  private readonly navigationStack: NavPathStack = new NavPathStack();
  private userIdFailureCount: number = 0;
  private userIdLockUntil: number = 0;

  private remoteUrl: string = '';
  private userId: string = '';
  private authenticatedUserId: string = '';
  private statusText: string = RemoteI18n.t('status.waitingConnection');
  private connectionState: ConnectionState = ConnectionState.Idle;
  private connectionFailureKind: string = '';
  private isBusy: boolean = false;
  private showRemoteUrlInput: boolean = false;
  private workspaceName: string = RemoteI18n.t('status.notConnected');
  private workspacePath: string = '';
  private workspaceBranch: string = '';
  private workspaceKind: string = 'normal';
  private assistantId: string = '';
  private desktopName: string = 'MacBook Pro';
  private desktopId: string = '';
  private sessionQuery: string = '';
  private sessionFilter: string = 'all';
  private activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  private messages: ChatMessage[] = [];
  private pendingMessages: ChatMessage[] = [];
  private hasMoreMessages: boolean = false;
  private activeTurnMessage: ChatMessage = RemoteUiState.emptyActiveTurn();
  private timelineItems: ChatTimelineItem[] = [];
  private modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  private selectedModelId: string = '';

  async aboutToAppear(): Promise<void> {
    this.syncRemotePageSummary();
    await this.generalChatBootstrapController.restore(getContext(this));
    await this.restoreIdentity();
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${this.connectionState} route=${this.currentRoute()}`);
    this.resumeRemoteActivity();
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${this.connectionState} route=${this.currentRoute()}`);
    this.stopPolling();
    this.persistVisibleGeneralChatDraft();
    this.stopGeneralChatStream(true, 'failed');
    this.stopHeartbeat();
  }

  aboutToDisappear(): void {
    this.stopPolling();
    this.persistVisibleGeneralChatDraft();
    this.stopGeneralChatStream(true, 'failed');
    this.generalChatDraftLifecycleController.cancel();
    this.remoteFileDownloadController.cancel();
    this.stopHeartbeat();
    this.voiceInputLifecycleController.cancel(`${this.currentRoute()}`, () => {
      this.setAllVoiceListening(false);
    });
  }

  private currentRoute(): AppRoute {
    return AppRouteContract.currentRoute(this.navigationStack.getAllPathName());
  }

  private isGeneralComposerRoute(route: AppRoute): boolean {
    return AppRootRouteState.isGeneralComposerRoute(route);
  }

  private visibleChatInput(): string {
    return AppRootRouteState.chatInput(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  private visibleSelectedImages(): SelectedImageAttachment[] {
    return AppRootRouteState.selectedImages(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  private visibleVoiceListening(): boolean {
    return AppRootRouteState.voiceListening(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  private setChatInputForRoute(route: AppRoute, value: string): void {
    AppRootRouteState.setChatInput(route, value, this.generalChatPageState, this.remotePageState);
  }

  private setSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.setSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  private addSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.addSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  private removeSelectedImageForRoute(route: AppRoute, imageId: string): void {
    AppRootRouteState.removeSelectedImage(route, imageId, this.generalChatPageState, this.remotePageState);
  }

  private clearComposerForRoute(route: AppRoute): void {
    AppRootRouteState.clearComposer(route, this.generalChatPageState, this.remotePageState);
  }

  private setVoiceListeningForRoute(route: AppRoute, isVoiceListening: boolean): void {
    AppRootRouteState.setVoiceListening(
      route,
      isVoiceListening,
      this.generalChatPageState,
      this.remotePageState
    );
  }

  private setAllVoiceListening(isVoiceListening: boolean): void {
    this.generalChatPageState.setVoiceListening(isVoiceListening);
    this.remotePageState.setVoiceListening(isVoiceListening);
  }

  private voiceInputSnapshot(route: AppRoute = this.currentRoute()): VoiceInputRouteSnapshot {
    return AppRootRouteState.snapshot(
      route,
      this.visibleChatBusy(),
      this.generalChatPageState,
      this.remotePageState
    );
  }

  private isRoute(route: AppRoute): boolean {
    return this.currentRoute() === route;
  }

  private isGeneralChatVisible(): boolean {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat);
  }

  private pushRoute(route: AppRoute, sessionId: string = ''): void {
    const spec = AppRouteContract.pathSpec(this.currentRoute(), route, sessionId);
    if (!spec) {
      return;
    }
    if (spec.hasSessionParam()) {
      this.navigationStack.pushPath({ name: spec.name, param: spec.routeParam() });
      return;
    }
    this.navigationStack.pushPath({ name: spec.name });
  }

  private replaceRoute(route: AppRoute, sessionId: string = ''): void {
    this.navigationStack.clear();
    if (route !== AppRoute.ChatHome) {
      this.pushRoute(route, sessionId);
    }
  }

  private popRoute(fallback: AppRoute): void {
    if (this.navigationStack.getAllPathName().length > 0) {
      this.navigationStack.pop();
      return;
    }
    this.replaceRoute(fallback);
  }

  private handleNavigationBack(route: AppRoute): boolean {
    const action = AppRouteContract.backAction(route, this.appShellState.showSidebar);
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

  build() {
    Stack() {
      AppShell({
        shellState: this.appShellState,
        content: () => {
          this.NavigationContent();
        },
        sidebar: () => {
          this.AppSidebarContent();
        },
        settings: () => {
          this.SettingsLayer();
        },
        connect: () => {
          this.ConnectSheet();
        },
        onCloseSidebar: () => {
          this.closeAppSidebar();
        }
      })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  NavigationContent() {
    Navigation(this.navigationStack) {
      this.RouteContent(AppRoute.ChatHome)
    }
    .width('100%')
    .height('100%')
    .hideTitleBar(true)
    .mode(NavigationMode.Stack)
    .navDestination(this.NavigationDestination)
  }

  @Builder
  NavigationDestination(name: string) {
    NavDestination() {
      this.RouteContent(name as AppRoute)
    }
    .hideTitleBar(true)
    .backgroundColor(PAGE_BG)
    .onBackPressed(() => {
      return this.handleNavigationBack(name as AppRoute);
    })
  }

  @Builder
  RouteContent(route: AppRoute) {
    Column() {
        if (route === AppRoute.RemoteHome) {
          RemoteHomeView({
            pageState: this.remotePageState,
            isBusy: this.remotePageState.isBusy,
            onOpenSidebar: () => {
              this.openAppSidebar();
            },
            onConnectWorkspace: () => {
              this.enterCodeEntry();
            },
            onAddConnection: () => {
              this.openAddConnection();
            },
            onOpenRemoteSettings: () => {
              this.openRemoteControlSettings();
            },
            onRefresh: () => {
              this.refreshSessions();
            },
            onShowWorkspaces: () => {
              this.showRecentWorkspaces();
            },
            onShowAssistants: () => {
              this.showAssistants();
            },
            onSelectWorkspace: (path: string) => {
              this.selectWorkspace(path);
            },
            onSelectAssistant: (path: string) => {
              this.selectAssistant(path);
            },
            onCancelWorkspacePicker: () => {
              this.remotePageState.setWorkspacePickerVisible(false);
            },
            onCancelAssistantPicker: () => {
              this.remotePageState.setAssistantPickerVisible(false);
            },
            onSessionQueryChange: (value: string) => {
              this.sessionQuery = value;
              this.remotePageState.setQuery(value);
            },
            onSearchSessions: () => {
              this.refreshSessions();
            },
            onLoadMoreSessions: () => {
              this.loadMoreSessions();
            },
            onReconnect: () => {
              this.reconnect();
            },
            onDisconnect: () => {
              this.disconnect(false);
            },
            onClearPairing: () => {
              this.disconnect(true);
            },
            onCreate: (agentType: string) => {
              this.createSession(agentType);
            },
            onCreateAssistantSession: () => {
              this.createSession('Claw');
            },
            onCreateInWorkspace: (path: string) => {
              this.createSessionInWorkspace(path);
            },
            onOpenSession: (session: RemoteSession) => {
              this.openHomeSession(session);
            },
            onDeleteSession: (session: RemoteSession) => {
              this.deleteHomeSession(session);
            }
          })
        } else {
          ConversationView({
            activeSession: route === AppRoute.RemoteChat ?
              this.remotePageState.activeSession : this.generalChatPageState.activeSession,
            surface: route === AppRoute.RemoteChat ? ChatSurface.Remote : ChatSurface.General,
            desktopName: route === AppRoute.RemoteChat ? this.remotePageState.desktopName : '',
            workspaceBranch: route === AppRoute.RemoteChat ? this.remotePageState.workspaceBranch : '',
            statusText: route === AppRoute.RemoteChat ?
              this.remotePageState.statusText : this.generalChatPageState.statusText,
            inlineStatusText: route === AppRoute.RemoteChat ? '' : this.generalChatHomeStatusText(),
            connectionState: route === AppRoute.RemoteChat ?
              this.remotePageState.connectionState :
              GeneralChatServiceStatus.connectionState(this.generalChatPageState.serviceState),
            composerCapabilities: route === AppRoute.RemoteChat ?
              REMOTE_CHAT_COMPOSER_CAPABILITIES : GENERAL_CHAT_COMPOSER_CAPABILITIES,
            isBusy: this.visibleChatBusy(),
            canStop: route === AppRoute.RemoteChat ?
              this.remotePageState.hasRunningActiveTurn() : this.generalChatPageState.hasRunningActiveTurn(),
            hasMoreMessages: route === AppRoute.RemoteChat ?
              this.remotePageState.hasMoreMessages : this.generalChatPageState.hasMoreMessages,
            timelineItems: route === AppRoute.RemoteChat ?
              this.remotePageState.timelineItems : this.generalChatPageState.timelineItems,
            timelineRevision: route === AppRoute.RemoteChat ?
              this.remotePageState.timelineRevision : this.generalChatPageState.timelineRevision,
            showSuggestionsWhenEmpty: route !== AppRoute.RemoteChat,
            supportsSearch: false,
            supportsImages: false,
            supportsFiles: false,
            modelCatalog: route === AppRoute.RemoteChat ?
              this.remotePageState.modelCatalog : RemoteUiState.emptyModelCatalog(),
            selectedModelId: route === AppRoute.RemoteChat ? this.remotePageState.selectedModelId : '',
            isSessionPinned: AppRouteContract.isGeneralComposerRoute(route) &&
              this.generalChatPageState.activeSession.sessionId.length > 0 &&
              this.generalChatPageState.pinnedSessionId() === this.generalChatPageState.activeSession.sessionId,
            downloadingFilePath: route === AppRoute.RemoteChat ? this.remotePageState.downloadingFilePath : '',
            downloadedFilePath: route === AppRoute.RemoteChat ? this.remotePageState.downloadedFilePath : '',
            fileDownloadStatus: route === AppRoute.RemoteChat ? this.remotePageState.fileDownloadStatus : '',
            selectedImages: route === AppRoute.RemoteChat ?
              this.remotePageState.selectedImages : this.generalChatPageState.selectedImages,
            isVoiceListening: route === AppRoute.RemoteChat ?
              this.remotePageState.isVoiceListening : this.generalChatPageState.isVoiceListening,
            chatInput: route === AppRoute.RemoteChat ?
              this.remotePageState.chatInput : this.generalChatPageState.chatInput,
            onOpenSidebar: () => {
              this.openAppSidebar();
            },
            onBack: () => {
              this.closeActiveChat();
            },
            onNewSession: () => {
              if (route === AppRoute.RemoteChat) {
                this.createSession('code');
              } else {
                this.prepareNewGeneralChat();
              }
            },
            onTogglePinSession: () => {
              if (AppRouteContract.isGeneralComposerRoute(route) &&
                this.generalChatPageState.activeSession.sessionId.length > 0) {
                const session = this.activeGeneralChatAsRemoteSession();
                this.generalChatCommandController.pinSession(
                  session,
                  this.generalChatPageState.pinnedSessionId() !== session.id,
                  this.generalChatPageState.isBusy
                );
              }
            },
            onArchiveSession: () => {
              if (AppRouteContract.isGeneralComposerRoute(route) &&
                this.generalChatPageState.activeSession.sessionId.length > 0) {
                this.archiveHomeSession(this.activeGeneralChatAsRemoteSession(), true);
              }
            },
            onDeleteSession: () => {
              if (AppRouteContract.isGeneralComposerRoute(route) &&
                this.generalChatPageState.activeSession.sessionId.length > 0) {
                const session = this.activeGeneralChatAsRemoteSession();
                this.deleteHomeSession(session).then(() => {
                  this.prepareNewGeneralChat();
                });
              }
            },
            onShowUploadedFiles: () => {
              const count = this.activeGeneralUploadedFileCount();
              this.showHomeToast(count > 0 ? `当前会话已上传 ${count} 个文件` : '当前会话暂无已上传文件');
            },
            onStop: () => {
              this.stopActiveChatTask();
            },
            onLoadOlder: () => {
              this.loadOlderMessages();
            },
            onApproveTool: (toolId: string, updatedInput?: Object) => {
              this.approveTool(toolId, updatedInput);
            },
            onRejectTool: (toolId: string) => {
              this.rejectTool(toolId);
            },
            onCancelTool: (toolId: string) => {
              this.cancelTool(toolId);
            },
            onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => {
              this.answerQuestion(toolId, answers);
            },
            onRenameSession: (title: string) => {
              this.renameVisibleSession(title);
            },
            onCopyMessage: (text: string) => {
              this.copyMessage(text);
            },
            onRetryMessage: (text: string) => {
              this.retryVisibleMessage(text);
            },
            onSelectModel: (modelId: string) => {
              this.selectModel(modelId);
            },
            onPickImages: () => {
              this.pickImages();
            },
            onRemoveImage: (imageId: string) => {
              this.removeSelectedImage(imageId);
            },
            onDownloadFile: (path: string) => {
              this.downloadVisibleFile(path);
            },
            onSend: () => {
              this.sendVisibleChatMessage();
            },
            onVoiceInput: () => {
              this.toggleVoiceInput();
            },
            onChatInputChange: (value: string) => {
              this.onVisibleChatInputChange(route, value);
            }
          })
        }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  AppSidebarContent() {
    AppSidebar({
      sessions: this.generalChatPageState.recentSessions(),
      pinnedSessionId: this.generalChatPageState.pinnedSessionId(),
      selectedSessionId: this.isGeneralChatVisible() ?
        this.generalChatPageState.activeSession.sessionId : '',
      connectionState: this.remotePageState.connectionState,
      activeSection: this.isRoute(AppRoute.RemoteHome) || this.isRoute(AppRoute.RemoteChat) ? 'remote' : 'chat',
      onClose: () => {
        this.closeAppSidebar();
      },
      onNewChat: () => {
        this.closeAppSidebar();
        this.prepareNewGeneralChat();
      },
      onEnterCode: () => {
        this.closeAppSidebar();
        this.enterCodeEntry();
      },
      onOpenSettings: () => {
        this.closeAppSidebar();
        this.appShellState.openSettings('general');
      },
      onOpenSession: (session: RemoteSession) => {
        this.closeAppSidebar();
        this.openHomeSession(session);
      },
      onArchiveSession: (session: RemoteSession, archived: boolean) => {
        this.archiveHomeSession(session, archived);
      },
      onExportSession: (session: RemoteSession) => {
        this.exportHomeSession(session);
      },
      onDeleteSession: (session: RemoteSession) => {
        this.deleteHomeSession(session);
      }
    })
  }

  @Builder
  SettingsLayer() {
    if (this.appShellState.settingsMode === 'remote') {
      RemoteControlSettingsSheet({
        desktopName: this.remotePageState.desktopName,
        desktopId: this.remotePageState.desktopId,
        userId: this.remotePageState.userId,
        accountUsername: this.remotePageState.accountUsername,
        authenticatedUserId: this.remotePageState.authenticatedUserId,
        deviceId: this.deviceId,
        connectionState: this.remotePageState.connectionState,
        statusText: this.remotePageState.statusText,
        isBusy: this.remotePageState.isBusy,
        onClose: () => {
          this.appShellState.setSettingsVisible(false);
        },
        onAddConnection: () => {
          this.openAddConnectionFromSettings();
        },
        onDisconnect: () => {
          this.disconnect(false);
        },
        onReconnect: () => {
          this.reconnect();
        }
      })
    } else {
      SettingsSheet({
        generalChatApiUrl: this.generalChatPageState.apiUrl,
        generalChatModelName: this.generalChatPageState.modelName,
        hasGeneralChatApiKey: this.generalChatPageState.hasApiKey,
        onSaveGeneralChatConfig: (
          apiUrl: string,
          apiKey: string,
          modelName: string,
          clearApiKey: boolean
        ) => {
          return this.saveGeneralChatConfig(apiUrl, apiKey, modelName, clearApiKey);
        },
        onClose: () => {
          this.appShellState.setSettingsVisible(false);
        }
      })
    }
  }

  @Builder
  ConnectSheet() {
    ConnectView({
      remoteUrl: this.remotePageState.remoteUrl,
      userId: this.remotePageState.userId,
      showRemoteUrlInput: this.remotePageState.showRemoteUrlInput,
      statusText: this.remotePageState.statusText,
      connectionState: this.remotePageState.connectionState,
      connectionFailureKind: this.remotePageState.connectionFailureKind,
      isBusy: this.remotePageState.isBusy,
      isConnected: this.remotePageState.connectionState === ConnectionState.Connected,
      desktopName: this.remotePageState.desktopName,
      desktopId: this.remotePageState.desktopId,
      deviceId: this.deviceId,
      requiresAccountAuth: this.remotePageState.requiresAccountAuth,
      accountUsername: this.remotePageState.accountUsername,
      onBack: () => {
        this.appShellState.setConnectSheetVisible(false);
      },
      onConnect: (password?: string) => {
        this.connect(false, password || '');
      },
      onClearPairing: () => {
        this.appShellState.setConnectSheetVisible(false);
        this.disconnect(true);
      },
      onRemoteUrlChange: (value: string) => {
        this.setRemoteUrl(value);
        this.applyRemotePairingProjection(value);
      },
      onUserIdChange: (value: string) => {
        this.setRemoteUserId(value);
      },
      onRemoteUrlDetected: (value: string): boolean => {
        return this.handleDetectedRemoteUrl(value);
      },
      onRemoteUrlInputVisibleChange: (visible: boolean) => {
        this.setRemoteUrlInputVisible(visible);
      },
      onPasteRemoteUrl: () => {
        this.pasteRemoteUrl();
      },
      onScanRemoteUrl: () => {
        this.scanRemoteUrl();
      }
    })
    .width('100%')
    .height('100%')
  }

  private async saveGeneralChatConfig(
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
      const snapshot = await this.generalChatConfigStore.save(update);
      this.applyGeneralChatConfig(snapshot);
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  private applyGeneralChatConfig(snapshot: GeneralChatConfigSnapshot): void {
    this.generalChatPageState.setConfiguration(
      snapshot.apiUrl,
      snapshot.modelName,
      snapshot.hasApiKey,
      GeneralChatServiceStatus.fromConfiguration(snapshot.apiUrl, snapshot.modelName, snapshot.hasApiKey)
    );
  }

  private async restoreIdentity(): Promise<void> {
    try {
      const snapshot: MobileIdentitySnapshot = await this.identityStore.init(getContext(this));
      this.deviceId = snapshot.installId;
      this.setRemoteUserId(snapshot.userId || snapshot.installId);
      this.setRemoteUrl(snapshot.remoteUrl);
      this.applyRemotePairingProjection(snapshot.remoteUrl);
      this.selectedModelId = await this.identityStore.getLastModelId();
      this.remoteModelController.setPreferredModelId(this.selectedModelId);
      this.userIdFailureCount = snapshot.userIdFailureCount;
      this.userIdLockUntil = snapshot.userIdLockUntil > Date.now() ? snapshot.userIdLockUntil : 0;
      if (snapshot.userIdLockUntil > 0 && this.userIdLockUntil === 0) {
        this.userIdFailureCount = 0;
        await this.identityStore.clearUserIdProtection();
      }
      this.setRemoteUrlInputVisible(snapshot.remoteUrl.length > 0);
      if (this.shouldAutoReconnect()) {
        this.autoReconnectAttempted = true;
        this.setRemoteConnectionState(ConnectionState.Reconnecting);
        this.setRemoteStatusText(RemoteI18n.t('status.restoringConnection'));
        this.connect(true);
      }
    } catch (err) {
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
      this.setRemoteConnectionState(ConnectionState.Failed);
    }
  }

  private shouldAutoReconnect(): boolean {
    return this.remotePairingPolicy.shouldAutoReconnect({
      autoReconnectAttempted: this.autoReconnectAttempted,
      remoteUrl: this.remoteUrl,
      userId: this.userId,
      requiresAccountAuth: this.remotePageState.requiresAccountAuth
    });
  }

  private async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    if (this.isBusy) {
      return;
    }
    try {
      this.setRemoteBusy(true);
      this.setRemoteConnectionFailureKind('');
      if (this.userIdLockUntil > 0 && !ConnectionErrorPolicy.isLocked(this.userIdLockUntil)) {
        this.userIdFailureCount = 0;
        this.userIdLockUntil = 0;
        await this.identityStore.clearUserIdProtection();
      }
      if (!autoReconnect && ConnectionErrorPolicy.isLocked(this.userIdLockUntil)) {
        throw new Error(RemoteI18n.f('errors.tooManyAttempts', `${ConnectionErrorPolicy.remainingLockSeconds(this.userIdLockUntil)}`));
      }
      this.setRemoteConnectionState(this.connectionState === ConnectionState.Reconnecting ?
        ConnectionState.Reconnecting : ConnectionState.Parsing);
      this.setRemoteStatusText(RemoteI18n.t('status.parsingUrl'));
      const descriptor = RemoteDescriptorParser.parse(this.remoteUrl);
      this.applyRemotePairingProjection(this.remoteUrl);
      const identity = this.remotePairingPolicy.identityForConnect(
        descriptor,
        this.userId,
        this.deviceId,
        accountPassword,
        autoReconnect
      );
      this.setRemoteUserId(identity.userId);

      this.setRemoteConnectionState(ConnectionState.Pairing);
      this.setRemoteStatusText(RemoteI18n.t('status.pairing'));
      const initialSync: InitialSyncResult = await this.sessionManager.connect(
        descriptor,
        identity,
        this.deviceId
      );
      await this.identityStore.savePairingInput(identity.userId, this.remoteUrl);
      await this.identityStore.clearUserIdProtection();
      this.userIdFailureCount = 0;
      this.userIdLockUntil = 0;
      this.applyWorkspace(initialSync.workspace);
      this.setRemoteAuthenticatedUserId(initialSync.authenticatedUserId);
      this.remoteSessionController.setSessions(initialSync.sessions, initialSync.hasMoreSessions);
      this.setRemoteConnectionState(ConnectionState.Connected);
      this.setRemoteConnectionFailureKind('');
      this.setRemoteStatusText(RemoteI18n.t('connection.connected'));
      this.appShellState.setConnectSheetVisible(false);
      if (!autoReconnect) {
        this.pushRoute(AppRoute.RemoteHome);
      }
      this.startHeartbeat();
      this.loadRecentWorkspacesInBackground();
    } catch (err) {
      const result: ConnectionErrorResult = await ConnectionErrorPolicy.connectionErrorResult(err, autoReconnect, {
        failureCount: this.userIdFailureCount,
        lockUntil: this.userIdLockUntil
      }, this.identityStore);
      this.setRemoteStatusText(result.message);
      this.setRemoteConnectionFailureKind(result.failureKind);
      this.userIdFailureCount = result.failureCount;
      this.userIdLockUntil = result.lockUntil;
      if (result.shouldShowRemoteUrlInput) {
        this.setRemoteUrlInputVisible(true);
      }
      this.setRemoteConnectionState(ConnectionState.Failed);
    } finally {
      this.setRemoteBusy(false);
    }
  }

  private async reconnect(): Promise<void> {
    if (this.isBusy) {
      return;
    }
    this.setRemoteConnectionState(ConnectionState.Reconnecting);
    this.setRemoteStatusText(RemoteI18n.t('status.reconnecting'));
    await this.connect();
  }

  private async disconnect(clearPairing: boolean): Promise<void> {
    this.stopPolling();
    this.stopHeartbeat();
    this.sessionManager.reset();
    this.remoteSessionController.clearSessions();
    this.remotePageState.clear();
    this.resetChatTimeline('');
    this.hasMoreMessages = false;
    this.remoteModelController.clearCatalog();
    this.remotePageState.clearActiveSession();
    this.knownPollVersion = 0;
    this.knownModelCatalogVersion = 0;
    this.knownRemoteMessageCount = 0;
    this.remoteFileDownloadController.clear();
    this.activeSession = {
      sessionId: '',
      title: '',
      workspacePath: '',
      agentType: 'code'
    };
    this.workspaceName = RemoteI18n.t('status.notConnected');
    this.workspacePath = '';
    this.workspaceBranch = '';
    this.workspaceKind = 'normal';
    this.assistantId = '';
    this.remotePageState.setWorkspace(
      this.workspaceName,
      this.workspacePath,
      this.assistantId,
      this.workspaceBranch,
      this.workspaceKind
    );
    this.setRemoteAuthenticatedUserId('');
    this.remotePageState.clearWorkspaceActions();
    this.setRemoteConnectionState(ConnectionState.Disconnected);
    this.setRemoteConnectionFailureKind('');
    this.setRemoteStatusText(clearPairing ? RemoteI18n.t('status.pairingCleared') : RemoteI18n.t('status.disconnected'));
    this.replaceRoute(AppRoute.RemoteHome);
    if (clearPairing) {
      this.setRemoteUrl('');
      this.setRemoteUserId(this.deviceId);
      this.remotePageState.setAccountPairing(false, '');
      this.setRemoteUrlInputVisible(false);
      await this.identityStore.clearPairingInput();
    } else {
      this.setRemoteUrlInputVisible(this.remoteUrl.trim().length > 0);
    }
  }

  private async pasteRemoteUrl(): Promise<void> {
    try {
      this.setRemoteStatusText(RemoteI18n.t('status.readClipboard'));
      const text = (await this.clipboardService.readText()).trim();
      if (text.length === 0) {
        this.setRemoteUrlInputVisible(true);
        this.setRemoteStatusText(RemoteI18n.t('status.clipboardEmpty'));
        return;
      }
      this.setRemoteUrl(text);
      this.applyRemotePairingProjection(text);
      this.setRemoteConnectionFailureKind('');
      this.setRemoteUrlInputVisible(true);
      try {
        RemoteDescriptorParser.parse(text);
        this.setRemoteStatusText(RemoteI18n.t('status.clipboardUrlFilled'));
      } catch (_err) {
        this.setRemoteStatusText(RemoteI18n.t('status.clipboardNeedsConfirm'));
      }
    } catch (err) {
      this.setRemoteUrlInputVisible(true);
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  private async scanRemoteUrl(): Promise<void> {
    try {
      this.setRemoteStatusText(RemoteI18n.t('status.openScanner'));
      const text = (await this.qrScanService.scanRemoteUrl(getContext(this))).trim();
      if (text.length === 0) {
        this.setRemoteStatusText(RemoteI18n.t('status.scanEmpty'));
        return;
      }
      this.handleDetectedRemoteUrl(text);
    } catch (err) {
      this.setRemoteUrlInputVisible(true);
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  private handleDetectedRemoteUrl(remoteUrl: string): boolean {
    try {
      this.setRemoteUrl(remoteUrl);
      this.applyRemotePairingProjection(remoteUrl);
      this.setRemoteConnectionFailureKind('');
      this.setRemoteUrlInputVisible(true);
      const descriptor = RemoteDescriptorParser.parse(remoteUrl);
      this.setRemoteStatusText(RemoteI18n.t('status.scannedUrl'));
      if (this.remotePairingPolicy.shouldPromptForAccount(descriptor)) {
        this.setRemoteStatusText(RemoteI18n.t('connect.enterAccountToPair'));
        this.appShellState.setConnectSheetVisible(true);
        return true;
      }
      this.connect();
      return false;
    } catch (err) {
      this.setRemoteUrlInputVisible(true);
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
      return true;
    }
  }

  private applyWorkspace(workspace: WorkspaceInfo): void {
    this.workspaceName = workspace.name;
    this.workspacePath = workspace.path;
    this.workspaceBranch = workspace.gitBranch;
    this.workspaceKind = workspace.workspaceKind || 'normal';
    this.assistantId = workspace.assistantId || '';
    this.remotePageState.setWorkspace(
      this.workspaceName,
      this.workspacePath,
      this.assistantId,
      this.workspaceBranch,
      this.workspaceKind
    );
  }

  private syncRemoteDesktopIdentity(remoteUrl: string): void {
    if (remoteUrl.trim().length === 0) {
      this.desktopId = '';
      this.remotePageState.setDesktopIdentity(this.desktopName, this.desktopId);
      return;
    }
    this.desktopName = RemoteUiState.desktopNameFromRemoteUrl(remoteUrl);
    this.desktopId = RemoteUiState.desktopIdFromRemoteUrl(remoteUrl);
    this.remotePageState.setDesktopIdentity(this.desktopName, this.desktopId);
  }

  private applyRemotePairingProjection(remoteUrl: string): void {
    this.syncRemoteDesktopIdentity(remoteUrl);
    const projection: RemotePairingProjection = this.remotePairingPolicy.projection(remoteUrl);
    this.remotePageState.setAccountPairing(projection.requiresAccountAuth, projection.accountUsername);
    const projectedUserId = this.remotePairingPolicy.userIdAfterProjection(this.userId, this.deviceId, projection);
    if (projectedUserId !== this.userId) {
      this.setRemoteUserId(projectedUserId);
    }
  }

  private setRemoteConnectionState(connectionState: ConnectionState): void {
    this.connectionState = connectionState;
    this.remotePageState.setConnectionState(connectionState);
  }

  private setRemoteUrl(remoteUrl: string): void {
    this.remoteUrl = remoteUrl;
    this.remotePageState.setRemoteUrl(remoteUrl);
  }

  private setRemoteUserId(userId: string): void {
    this.userId = userId;
    this.remotePageState.setUserId(userId);
  }

  private setRemoteAuthenticatedUserId(authenticatedUserId: string): void {
    this.authenticatedUserId = authenticatedUserId;
    this.remotePageState.setAuthenticatedUserId(authenticatedUserId);
  }

  private setRemoteStatusText(statusText: string): void {
    this.statusText = statusText;
    this.remotePageState.setStatusText(statusText);
  }

  private setRemoteConnectionFailureKind(connectionFailureKind: string): void {
    this.connectionFailureKind = connectionFailureKind;
    this.remotePageState.setConnectionFailureKind(connectionFailureKind);
  }

  private setRemoteBusy(isBusy: boolean): void {
    this.isBusy = isBusy;
    this.remotePageState.setBusy(isBusy);
  }

  private setRemoteUrlInputVisible(visible: boolean): void {
    this.showRemoteUrlInput = visible;
    this.remotePageState.setRemoteUrlInputVisible(visible);
  }

  private syncRemotePageSummary(): void {
    this.remotePageState.setDesktopIdentity(this.desktopName, this.desktopId);
    this.remotePageState.setPairingInput(this.remoteUrl, this.userId, this.showRemoteUrlInput);
    this.remotePageState.setAuthenticatedUserId(this.authenticatedUserId);
    this.remotePageState.setStatusText(this.statusText);
    this.remotePageState.setConnectionState(this.connectionState);
    this.remotePageState.setConnectionFailureKind(this.connectionFailureKind);
    this.remotePageState.setBusy(this.isBusy);
    this.remotePageState.setWorkspace(
      this.workspaceName,
      this.workspacePath,
      this.assistantId,
      this.workspaceBranch,
      this.workspaceKind
    );
  }

  private ensureRemoteAvailable(): boolean {
    if (RemoteUiState.canUseRemote(this.connectionState)) {
      return true;
    }
    this.setRemoteStatusText(RemoteI18n.t('status.remoteUnavailable'));
    return false;
  }

  private failRemoteConnection(err: Object): void {
    this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    this.setRemoteConnectionState(ConnectionState.Failed);
    this.stopHeartbeat();
  }

  private async showRecentWorkspaces(): Promise<void> {
    if (this.remotePageState.showWorkspacePicker) {
      this.remotePageState.setWorkspacePickerVisible(false);
      return;
    }
    this.remotePageState.setWorkspacePickerVisible(true);
    this.remotePageState.setAssistantPickerVisible(false);
    await this.loadRecentWorkspaces();
  }

  private async showAssistants(): Promise<void> {
    if (this.remotePageState.showAssistantPicker) {
      this.remotePageState.setAssistantPickerVisible(false);
      return;
    }
    this.remotePageState.setAssistantPickerVisible(true);
    this.remotePageState.setWorkspacePickerVisible(false);
    await this.loadAssistants();
  }

  private async loadRecentWorkspaces(): Promise<void> {
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.setRemoteBusy(true);
      this.setRemoteStatusText(RemoteI18n.t('status.loadingRecentWorkspaces'));
      const recentWorkspaces = await this.sessionManager.listRecentWorkspaces();
      this.remotePageState.setRecentWorkspaces(recentWorkspaces);
      this.setRemoteStatusText(recentWorkspaces.length > 0 ? RemoteI18n.t('status.chooseWorkspace') : RemoteI18n.t('status.noRecentWorkspaces'));
    } catch (err) {
      this.failRemoteConnection(err);
    } finally {
      this.setRemoteBusy(false);
    }
  }

  private async selectWorkspace(path: string): Promise<void> {
    if (this.isBusy || path.trim().length === 0) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.setRemoteBusy(true);
      this.setRemoteStatusText(RemoteI18n.t('status.switchingWorkspace'));
      await this.sessionManager.setWorkspace(path);
      const workspace = await this.sessionManager.getWorkspaceInfo();
      this.applyWorkspace(workspace);
      this.remotePageState.closePickers();
      this.remoteSessionController.clearSessions();
      this.remotePageState.clear();
      this.setRemoteStatusText(RemoteI18n.t('status.workspaceSwitched'));
      this.setRemoteBusy(false);
      await this.refreshSessions();
    } catch (err) {
      this.failRemoteConnection(err);
      this.setRemoteBusy(false);
    }
  }

  private async loadAssistants(): Promise<void> {
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.setRemoteBusy(true);
      this.setRemoteStatusText(RemoteI18n.t('status.loadingAssistants'));
      const assistants = await this.sessionManager.listAssistants();
      this.remotePageState.setAssistants(assistants);
      this.setRemoteStatusText(assistants.length > 0 ? RemoteI18n.t('status.chooseAssistant') : RemoteI18n.t('status.noAssistants'));
    } catch (err) {
      this.failRemoteConnection(err);
    } finally {
      this.setRemoteBusy(false);
    }
  }

  private async selectAssistant(path: string): Promise<void> {
    if (this.isBusy || path.trim().length === 0) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.setRemoteBusy(true);
      this.setRemoteStatusText(RemoteI18n.t('status.switchingAssistant'));
      await this.sessionManager.setAssistant(path);
      const workspace = await this.sessionManager.getWorkspaceInfo();
      this.applyWorkspace(workspace);
      this.remotePageState.closePickers();
      this.remoteSessionController.clearSessions();
      this.remotePageState.clear();
      this.setRemoteStatusText(RemoteI18n.t('status.assistantSwitched'));
      this.setRemoteBusy(false);
      await this.refreshSessions();
    } catch (err) {
      this.failRemoteConnection(err);
      this.setRemoteBusy(false);
    }
  }

  private async refreshSessions(): Promise<void> {
    await this.remoteSessionController.refresh(
      this.sessionQuery,
      this.sessionFilter,
      this.ensureRemoteAvailable(),
      this.connectionState === ConnectionState.Connected
    );
  }

  private async loadMoreSessions(): Promise<void> {
    await this.remoteSessionController.loadMore(
      this.sessionQuery,
      this.sessionFilter,
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  private setSessionFilter(filter: string): void {
    if (this.sessionFilter === filter) {
      return;
    }
    this.sessionFilter = filter;
    this.refreshSessions();
  }

  private visibleChatBusy(): boolean {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.isBusy : this.remotePageState.isBusy;
  }

  private visibleStatusText(): string {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.statusText : this.remotePageState.statusText;
  }

  private setVisibleStatusText(statusText: string): void {
    if (this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat)) {
      this.generalChatPageState.setStatus(statusText);
      return;
    }
    this.setRemoteStatusText(statusText);
  }

  private openAppSidebar(): void {
    this.getUIContext().animateTo({ duration: 230, curve: Curve.EaseOut }, () => {
      this.appShellState.setSidebarVisible(true);
    });
  }

  private closeAppSidebar(): void {
    this.getUIContext().animateTo({ duration: 210, curve: Curve.EaseOut }, () => {
      this.appShellState.setSidebarVisible(false);
    });
  }

  private enterCodeEntry(): void {
    if (RemoteUiState.canUseRemote(this.connectionState)) {
      this.appShellState.setConnectSheetVisible(false);
      this.pushRoute(AppRoute.RemoteHome);
      return;
    }
    this.appShellState.setConnectSheetVisible(true);
  }

  private openAddConnection(): void {
    this.appShellState.setConnectSheetVisible(true);
  }

  private openRemoteControlSettings(): void {
    setTimeout(() => {
      this.appShellState.openSettings('remote');
    }, 180);
  }

  private openAddConnectionFromSettings(): void {
    this.appShellState.setSettingsVisible(false);
    setTimeout(() => {
      this.openAddConnection();
    }, 220);
  }

  private openHomeSession(session: RemoteSession): void {
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openSession(session);
  }

  private async deleteHomeSession(session: RemoteSession): Promise<void> {
    if (session.agentType !== 'chat') {
      await this.deleteSession(session);
      return;
    }
    await this.generalChatCommandController.deleteSession(session, this.generalChatPageState.isBusy);
  }

  private activeGeneralChatAsRemoteSession(): RemoteSession {
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

  private activeGeneralUploadedFileCount(): number {
    let count = 0;
    this.generalChatPageState.timelineItems.forEach((item: ChatTimelineItem) => {
      if (item.message && item.message.images) {
        count += item.message.images.length;
      }
    });
    return count;
  }

  private async archiveHomeSession(session: RemoteSession, archived: boolean): Promise<void> {
    await this.generalChatCommandController.archiveSession(session, archived, this.generalChatPageState.isBusy);
  }

  private async exportHomeSession(session: RemoteSession): Promise<void> {
    await this.generalChatCommandController.exportSession(
      session,
      this.generalChatPageState.isBusy,
      async (text: string): Promise<void> => {
        await this.clipboardService.writeText(text);
      }
    );
  }

  private async openGeneralSession(item: RemoteSession): Promise<void> {
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
      (sessionId: string) => {
        this.replaceRoute(AppRoute.ChatHome, sessionId);
      }
    );
  }

  private async startGeneralChat(text: string): Promise<void> {
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
      (sessionId: string) => {
        this.replaceRoute(AppRoute.ChatHome, sessionId);
      }
    );
    if (!created) {
      return;
    }
    await this.sendGeneralChatMessage();
  }

  private async sendVisibleChatMessage(): Promise<void> {
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

  private async stopActiveChatTask(): Promise<void> {
    if (this.isGeneralChatVisible()) {
      this.stopGeneralChatStream(true);
      return;
    }
    await this.stopActiveTask();
  }

  private closeActiveChat(): void {
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

  private async renameVisibleSession(title: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      await this.generalChatCommandController.renameActiveSession(
        this.generalChatPageState.activeSession,
        title
      );
      return;
    }
    await this.renameActiveSession(title);
  }

  private async retryVisibleMessage(text: string): Promise<void> {
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

  private downloadVisibleFile(path: string): void {
    if (this.isGeneralChatVisible()) {
      this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.fileDownloadMock'));
      return;
    }
    this.downloadFile(path);
  }

  private async createSession(agentType: string): Promise<void> {
    await this.remoteSessionController.create(
      agentType,
      this.isBusy,
      this.ensureRemoteAvailable(),
      async (session: SessionSummary): Promise<void> => {
        this.remotePageState.clearComposer();
        this.hasMoreMessages = false;
        this.resetChatTimeline(session.sessionId);
        this.knownPollVersion = 0;
        this.knownRemoteMessageCount = 0;
        this.pushRoute(AppRoute.RemoteChat, session.sessionId);
        await this.loadModelCatalog(session.sessionId);
        await this.refreshSessions();
        await this.loadActiveMessages();
        this.startPolling();
      }
    );
  }

  private async createSessionInWorkspace(path: string): Promise<void> {
    if (path.length > 0 && path !== this.workspacePath) {
      await this.selectWorkspace(path);
    }
    if (path.length === 0 || path === this.workspacePath) {
      await this.createSession('code');
    }
  }

  private async loadWorkspaceSessions(paths: string[]): Promise<void> {
    const all: RemoteSession[] = [];
    for (const path of paths) {
      try {
        const sessions = await this.sessionManager.listSessionsForWorkspace(path, 50);
        sessions.forEach((item: RemoteSession) => {
          if (!all.some((existing: RemoteSession) => existing.id === item.id)) {
            all.push(item);
          }
        });
      } catch (err) {
        RemoteLogger.warn(`load workspace sessions failed path=${path}: ${String(err)}`);
      }
    }
    this.remoteWorkspaceSessions = all;
    const current = this.remotePageState.sessions;
    const extras = all.filter((item: RemoteSession) => item.workspacePath !== this.workspacePath);
    this.remotePageState.setSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  private async loadRecentWorkspacesInBackground(): Promise<void> {
    try {
      const recentWorkspaces = await this.sessionManager.listRecentWorkspaces();
      this.remotePageState.setRecentWorkspaces(recentWorkspaces);
      await this.loadWorkspaceSessions(recentWorkspaces.map((item: RecentWorkspaceEntry) => item.path));
    } catch (err) {
      RemoteLogger.warn(`background recent workspace load failed: ${String(err)}`);
    }
  }

  private mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[] {
    const merged = primary.slice();
    extras.forEach((item: RemoteSession) => {
      if (!merged.some((existing: RemoteSession) => existing.id === item.id)) {
        merged.push(item);
      }
    });
    return merged;
  }

  private async openSession(item: RemoteSession): Promise<void> {
    await this.remoteSessionController.open(
      item,
      item.workspacePath || this.workspacePath,
      this.isBusy,
      this.ensureRemoteAvailable(),
      async (session: SessionSummary): Promise<void> => {
        this.stopPolling();
        this.resetChatTimeline(item.id);
        this.knownPollVersion = 0;
        this.knownRemoteMessageCount = 0;
        this.hasMoreMessages = false;
        this.remoteFileDownloadController.clear();
        this.remotePageState.clearComposer();
        this.pushRoute(AppRoute.RemoteChat, item.id);
        await this.loadModelCatalog(item.id);
        await this.loadActiveMessages();
        if (this.activeSession.sessionId === session.sessionId && this.isRoute(AppRoute.RemoteChat)) {
          this.startPolling();
        }
      }
    );
  }

  private async deleteSession(item: RemoteSession): Promise<void> {
    await this.remoteSessionController.delete(
      item,
      this.activeSession.sessionId,
      this.isBusy,
      this.ensureRemoteAvailable(),
      this.sessionQuery,
      this.sessionFilter,
      async (): Promise<void> => {
        this.stopPolling();
        this.resetChatTimeline('');
        this.hasMoreMessages = false;
        this.remotePageState.clearComposer();
        this.remoteModelController.clearCatalog();
        this.knownPollVersion = 0;
        this.knownModelCatalogVersion = 0;
        this.knownRemoteMessageCount = 0;
        this.activeSession = {
          sessionId: '',
          title: '',
          workspacePath: this.workspacePath,
          agentType: 'code'
        };
        this.remotePageState.clearActiveSession();
        this.replaceRoute(AppRoute.RemoteHome);
      }
    );
  }

  private async loadActiveMessages(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    await this.remoteChatCommandController.loadMessages(
      sessionId,
      (activeSessionId: string): boolean => {
        return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
      }
    );
  }

  private async loadModelCatalog(sessionId: string): Promise<void> {
    await this.remoteModelController.loadCatalog(
      sessionId,
      this.ensureRemoteAvailable(),
      (activeSessionId: string): boolean => {
        return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
      }
    );
  }

  private async selectModel(modelId: string): Promise<void> {
    await this.remoteModelController.selectModel(
      modelId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  private async loadOlderMessages(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    await this.remoteChatCommandController.loadOlderMessages(
      sessionId,
      this.knownPollVersion,
      this.hasMoreMessages,
      this.isBusy
    );
  }

  private async sendGeneralChatMessage(): Promise<void> {
    if (this.generalChatPageState.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    const rawText = this.generalChatPageState.chatInput.trim();
    const images = this.generalChatPageState.selectedImages.slice();
    const text = rawText.length > 0 ? rawText : (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.generalChatPageState.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.generalChatPageState.isBusy) {
      return;
    }
    this.generalChatPageState.clearComposer();
    this.generalChatDraftLifecycleController.clear(sessionId);
    const localMessage = RemoteUiState.localUserMessage(text, images);
    this.chatTimelineStore.appendOptimisticMessage(localMessage);
    this.syncGeneralChatTimelineFromStore();
    try {
      this.generalChatPageState.setBusy(true);
      this.generalChatPageState.setServiceState(GeneralChatServiceState.Sending);
      this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.generating'));
      this.beginGeneralChatNetworkStream(sessionId);
      const observer = new GeneralChatStreamCallbacks(
        async (message: ChatMessage): Promise<void> => {
          this.chatTimelineStore.mergePersistedMessages([message]);
          this.syncGeneralChatTimelineFromStore();
        },
        (delta: string): void => {
          this.appendGeneralChatNetworkDelta(sessionId, delta);
        }
      );
      this.generalChatStreamLifecycleController.setRequestInFlight(true);
      const result: GeneralChatSendResult = await this.generalChatCommandController.sendMessage(
        sessionId,
        text,
        images,
        observer
      );
      this.generalChatStreamLifecycleController.setRequestInFlight(false);
      this.flushGeneralChatNetworkDelta(sessionId);
      this.generalChatCommandController.refreshSessions();
      if (this.generalChatStreamLifecycleController.text().length > 0) {
        this.finishGeneralChatNetworkStream(sessionId, result);
      } else {
        this.resetGeneralChatNetworkStream();
        this.chatTimelineStore.mergePersistedMessages([result.userMessage, result.assistantMessage]);
        await this.persistGeneralAssistantMessage(sessionId, result.assistantMessage);
        this.chatTimelineStore.clearActiveTurn();
        this.syncGeneralChatTimelineFromStore();
        this.generalChatPageState.setBusy(false);
        this.generalChatPageState.setServiceState(GeneralChatServiceState.Ready);
        this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.ready'));
      }
    } catch (err) {
      this.generalChatStreamLifecycleController.setRequestInFlight(false);
      if (this.generalChatStreamLifecycleController.takeRequestCancelled()) {
        const finalMessage = this.generalChatStreamLifecycleController.takePendingFinalMessage();
        if (finalMessage) {
          await this.persistGeneralAssistantMessage(sessionId, finalMessage);
          this.generalChatPageState.setServiceState(finalMessage.status === 'failed' ?
            GeneralChatServiceState.Failed : GeneralChatServiceState.Ready);
        } else {
          this.generalChatPageState.setServiceState(GeneralChatServiceState.Ready);
        }
        this.resetGeneralChatNetworkStream();
        this.generalChatPageState.setBusy(false);
        return;
      }
      this.flushGeneralChatNetworkDelta(sessionId);
      const partialText = this.generalChatStreamLifecycleController.text();
      const errorText = ConnectionErrorPolicy.errorText(err);
      this.generalChatPageState.setServiceState(GeneralChatServiceStatus.fromError(errorText));
      this.resetGeneralChatNetworkStream();
      this.generalChatPageState.setChatInput(rawText);
      this.generalChatPageState.setSelectedImages(images);
      await this.generalChatDraftLifecycleController.saveNow(sessionId, rawText);
      this.chatTimelineStore.setPersistedMessages(await this.generalChatCommandController.loadMessages(sessionId));
      if (partialText.trim().length > 0) {
        const interruptedMessage: ChatMessage = {
          id: Encoding.randomId('assistant'),
          turnId: this.currentActiveTurnId(),
          role: 'assistant',
          text: partialText,
          status: 'failed',
          detail: rawText,
          timestamp: new Date().toISOString(),
          items: this.generalAssistantItems([], partialText)
        };
        this.chatTimelineStore.mergePersistedMessages([interruptedMessage]);
        await this.persistGeneralAssistantMessage(sessionId, interruptedMessage);
      }
      this.chatTimelineStore.clearActiveTurn();
      this.syncGeneralChatTimelineFromStore();
      this.generalChatPageState.setStatus(errorText);
      this.generalChatPageState.setBusy(false);
    }
  }

  private beginGeneralChatNetworkStream(sessionId: string): void {
    const activeTurn = this.generalChatStreamLifecycleController.begin(sessionId);
    this.chatTimelineStore.setActiveTurn(activeTurn);
    this.syncGeneralChatTimelineFromStore();
  }

  private appendGeneralChatNetworkDelta(sessionId: string, delta: string): void {
    const accepted = this.generalChatStreamLifecycleController.append(
      sessionId,
      delta,
      (): boolean => this.generalChatPageState.activeSession.sessionId === sessionId && this.isGeneralChatVisible(),
      (activeTurn: ChatMessage): void => {
        this.chatTimelineStore.setActiveTurn(activeTurn);
        this.syncGeneralChatTimelineFromStore();
      }
    );
    if (accepted) {
      this.generalChatPageState.setServiceState(GeneralChatServiceState.Streaming);
    }
  }

  private flushGeneralChatNetworkDelta(sessionId: string): void {
    const activeTurn = this.generalChatStreamLifecycleController.flush(
      sessionId,
      this.generalChatPageState.activeSession.sessionId === sessionId && this.isGeneralChatVisible()
    );
    if (!activeTurn) {
      return;
    }
    this.chatTimelineStore.setActiveTurn(activeTurn);
    this.syncGeneralChatTimelineFromStore();
  }

  private finishGeneralChatNetworkStream(sessionId: string, result: GeneralChatSendResult): void {
    const currentStreamSessionId = this.generalChatStreamLifecycleController.currentSessionId();
    const isVisibleSession = this.generalChatPageState.activeSession.sessionId === sessionId &&
      this.isGeneralChatVisible();
    RemoteLogger.info(`general chat finish check session=${this.shortSessionId(sessionId)} current=${this.shortSessionId(currentStreamSessionId)} visible=${isVisibleSession}`);
    if (currentStreamSessionId !== sessionId && !isVisibleSession) {
      return;
    }
    this.chatTimelineStore.mergePersistedMessages([result.userMessage, result.assistantMessage]);
    this.persistGeneralAssistantMessage(sessionId, result.assistantMessage);
    this.chatTimelineStore.clearActiveTurn();
    this.syncGeneralChatTimelineFromStore();
    const finalState = this.chatTimelineStore.snapshot();
    RemoteLogger.info(`general chat finish applied session=${this.shortSessionId(sessionId)} persisted=${finalState.persistedMessages.length} active=${finalState.activeTurn ? finalState.activeTurn.id : 'none'} textLength=${result.assistantMessage.text.length}`);
    this.resetGeneralChatNetworkStream();
    this.generalChatPageState.setBusy(false);
    this.generalChatPageState.setServiceState(GeneralChatServiceState.Ready);
    this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.ready'));
  }

  private resetGeneralChatNetworkStream(): void {
    this.generalChatStreamLifecycleController.reset();
  }

  private generalAssistantItems(tools: RemoteToolStatusResponse[], text: string): ChatMessageItemResponse[] {
    const items: ChatMessageItemResponse[] = [];
    tools.forEach((tool: RemoteToolStatusResponse) => {
      items.push({
        type: 'tool',
        tool
      });
    });
    if (text.trim().length > 0) {
      items.push({
        type: 'text',
        content: text
      });
    }
    return items;
  }

  private async persistGeneralAssistantMessage(sessionId: string, message: ChatMessage): Promise<void> {
    if (sessionId.length === 0) {
      return;
    }
    await this.generalChatCommandController.recordAssistantMessage(sessionId, message);
  }

  private stopGeneralChatStream(cancelled: boolean, finalStatus: string = 'cancelled'): void {
    const hasGeneralStream = this.generalChatStreamLifecycleController.hasActiveStream();
    if (!this.isGeneralChatVisible() && !hasGeneralStream) {
      return;
    }
    const hadStream = hasGeneralStream ||
      (this.isGeneralChatVisible() && this.generalChatPageState.activeTurnMessage.id.length > 0);
    const currentStreamSessionId = this.generalChatStreamLifecycleController.currentSessionId();
    if (cancelled && currentStreamSessionId.length > 0) {
      this.flushGeneralChatNetworkDelta(currentStreamSessionId);
    }
    if (hasGeneralStream) {
      this.generalChatStreamLifecycleController.markRequestCancelled();
      this.generalChatCommandController.cancelCurrentRequest();
    }
    this.generalChatStreamLifecycleController.reset();
    if (cancelled && this.generalChatPageState.activeTurnMessage.id.length > 0) {
      const activeTurnMessage = this.generalChatPageState.activeTurnMessage;
      const stoppedMessage: ChatMessage = {
        id: Encoding.randomId('assistant'),
        turnId: this.currentActiveTurnId(),
        role: 'assistant',
        text: activeTurnMessage.text || (finalStatus === 'failed' ?
          RemoteI18n.t('generalChat.interrupted') : RemoteI18n.t('generalChat.stopped')),
        status: finalStatus,
        detail: finalStatus === 'failed' ? this.latestUserMessageText() : '',
        timestamp: new Date().toISOString(),
        items: this.generalAssistantItems([], activeTurnMessage.text || '')
      };
      this.chatTimelineStore.mergePersistedMessages([stoppedMessage]);
      if (this.generalChatStreamLifecycleController.isRequestInFlight()) {
        this.generalChatStreamLifecycleController.setPendingFinalMessage(stoppedMessage);
      } else {
        this.persistGeneralAssistantMessage(this.generalChatPageState.activeSession.sessionId || '', stoppedMessage);
      }
      this.generalChatPageState.setStatus(finalStatus === 'failed' ?
        RemoteI18n.t('generalChat.interrupted') : RemoteI18n.t('status.stopRequested'));
      this.generalChatPageState.setServiceState(finalStatus === 'failed' ?
        GeneralChatServiceState.Failed : GeneralChatServiceState.Ready);
    }
    if (this.isGeneralChatVisible()) {
      this.chatTimelineStore.clearActiveTurn();
      this.syncGeneralChatTimelineFromStore();
      if (cancelled || hadStream) {
        this.generalChatPageState.setBusy(false);
      }
    }
  }

  private async sendChatMessage(): Promise<void> {
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

  private async toggleVoiceInput(): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.toggle(getContext(this), this.voiceInputSnapshot(route));
  }

  private async stopVoiceInput(showStatus: boolean): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.stop(this.voiceInputSnapshot(route), showStatus);
  }

  private showVoiceInputError(message: string): void {
    const text = message.length > 0 ? message : RemoteI18n.t('errors.voiceInputUnavailable');
    this.setVisibleStatusText(text);
    try {
      this.getUIContext().getPromptAction().showToast({
        message: text,
        duration: 2600
      });
    } catch (_err) {
    }
  }

  private async pickImages(): Promise<void> {
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

  private removeSelectedImage(imageId: string): void {
    this.removeSelectedImageForRoute(this.currentRoute(), imageId);
  }

  private startVisibleGeneralChat(): void {
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

  private generalChatHomeStatusText(): string {
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

  private prepareNewGeneralChat(): void {
    this.stopVoiceInput(false);
    this.stopGeneralChatStream(true);
    this.generalChatDraftLifecycleController.clearHome();
    this.generalChatPageState.clearComposer();
    this.generalChatPageState.clearActiveSession();
    this.resetGeneralChatTimeline('');
    this.replaceRoute(AppRoute.ChatHome);
  }

  private onVisibleChatInputChange(route: AppRoute, value: string): void {
    this.setChatInputForRoute(route, value);
    if (!this.isGeneralComposerRoute(route)) {
      return;
    }
    this.generalChatDraftLifecycleController.scheduleVisible(value);
  }

  private visibleGeneralChatDraftId(): string {
    if (this.isGeneralChatVisible()) {
      return this.generalChatPageState.activeSession.sessionId || GENERAL_CHAT_HOME_DRAFT_ID;
    }
    return '';
  }

  private persistVisibleGeneralChatDraft(): void {
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
  }

  private async restoreGeneralChatDraft(draftId: string): Promise<void> {
    this.generalChatPageState.setChatInput(await this.generalChatDraftLifecycleController.restore(draftId));
  }

  private latestUserMessageText(): string {
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

  private showHomeToast(message: string): void {
    try {
      this.getUIContext().getPromptAction().showToast({
        message,
        duration: 2600
      });
    } catch (_err) {
      this.setVisibleStatusText(message);
    }
  }

  private async stopActiveTask(): Promise<void> {
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

  private async renameActiveSession(title: string): Promise<void> {
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

  private async copyMessage(text: string): Promise<void> {
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

  private async downloadFile(path: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    await this.remoteFileDownloadController.download(path, sessionId, this.isBusy, this.ensureRemoteAvailable());
  }

  private retryMessage(text: string): void {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.remotePageState.setChatInput(text);
    this.sendChatMessage();
  }

  private async approveTool(toolId: string, updatedInput?: Object): Promise<void> {
    await this.remoteToolActionController.approve(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable(),
      updatedInput
    );
  }

  private async rejectTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.reject(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  private async cancelTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.cancel(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  private async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    await this.remoteToolActionController.answer(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable(),
      answers
    );
  }

  private resetChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.knownPollVersion = 0;
    this.syncChatTimelineFromStore();
  }

  private resetGeneralChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.syncGeneralChatTimelineFromStore();
  }

  private syncChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshot();
    this.messages = state.persistedMessages;
    this.pendingMessages = state.optimisticMessages;
    this.activeTurnMessage = state.activeTurn || RemoteUiState.emptyActiveTurn();
    this.modelCatalog = state.modelCatalog;
    this.selectedModelId = state.selectedModelId;
    this.timelineItems = this.projectedTimelineItems();
    this.remotePageState.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      this.hasMoreMessages,
      this.timelineItems
    );
    this.remotePageState.setModelCatalog(this.modelCatalog, this.selectedModelId);
  }

  private syncGeneralChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshot();
    const projectedItems = this.chatTimelineStore.project(false);
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

  private startPolling(): void {
    this.remoteChatPollingLifecycleController.startActiveSession({
      sessionId: this.activeSession.sessionId || '',
      cursor: this.currentChatPollingCursor(),
      activeTurn: this.activeTurnMessage
    });
  }

  private stopPolling(): void {
    this.remoteChatPollingLifecycleController.stop();
  }

  private nudgeChatPolling(): void {
    this.remoteChatPollingLifecycleController.nudge();
  }

  private async pollActiveSession(): Promise<void> {
    await this.remoteChatPollingLifecycleController.pollNow();
  }

  private currentChatPollingCursor(): RemoteChatPollingCursor {
    return {
      pollVersion: this.knownPollVersion,
      knownMessageCount: this.knownRemoteMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    };
  }

  private updateChatPollingCursor(pollVersion: number, knownMessageCount: number): void {
    this.knownPollVersion = pollVersion;
    this.knownRemoteMessageCount = knownMessageCount;
    this.remoteChatPollingLifecycleController.updateCursor({
      pollVersion,
      knownMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    });
  }

  private applyChatSessionSnapshot(snapshot: RemoteChatPollingSnapshot): void {
    if (snapshot.sessionId !== this.activeSession.sessionId || !this.isRoute(AppRoute.RemoteChat)) {
      return;
    }
    this.chatTimelineStore.applySnapshot(snapshot);
    this.syncChatTimelineFromStore();
    this.knownPollVersion = snapshot.cursor.pollVersion;
    this.knownModelCatalogVersion = snapshot.cursor.knownModelCatalogVersion;
    this.knownRemoteMessageCount = snapshot.cursor.knownMessageCount;
    if (snapshot.title.length > 0) {
      this.activeSession = {
        sessionId: this.activeSession.sessionId,
        title: snapshot.title,
        workspacePath: this.activeSession.workspacePath,
        agentType: this.activeSession.agentType
      };
      this.remotePageState.setActiveSession(this.activeSession);
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

  private hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  private currentActiveTurnId(): string {
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

  private projectedTimelineItems(): ChatTimelineItem[] {
    return this.chatTimelineStore.project(this.hasMoreMessages);
  }

  private startHeartbeat(): void {
    this.remoteActivityLifecycleController.startHeartbeat();
  }

  private stopHeartbeat(): void {
    this.remoteActivityLifecycleController.stopHeartbeat();
  }

  private async checkConnectionHealth(): Promise<void> {
    if (this.connectionState !== ConnectionState.Connected || this.isBusy) {
      return;
    }
    try {
      RemoteLogger.info('heartbeat ping start');
      await this.sessionManager.ping();
      RemoteLogger.info('heartbeat ping ok');
    } catch (err) {
      RemoteLogger.warn(`heartbeat ping failed ${ConnectionErrorPolicy.errorText(err)}`);
      this.setRemoteConnectionState(ConnectionState.Failed);
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
      this.stopHeartbeat();
    }
  }

  private resumeRemoteActivity(): void {
    if (!this.hasRemoteBindingForResume()) {
      return;
    }
    this.startHeartbeat();
    if (this.isRoute(AppRoute.RemoteChat) && this.activeSession.sessionId.length > 0) {
      this.startPolling();
      this.nudgeChatPolling();
    }
    this.verifyConnectionAfterForeground();
  }

  private async verifyConnectionAfterForeground(): Promise<void> {
    if (this.isBusy || !this.hasRemoteBindingForResume()) {
      return;
    }
    try {
      this.setRemoteConnectionState(ConnectionState.Reconnecting);
      RemoteLogger.info('foreground ping start');
      await this.sessionManager.ping();
      this.setRemoteConnectionState(ConnectionState.Connected);
      this.setRemoteStatusText(RemoteI18n.t('connection.connected'));
      RemoteLogger.info('foreground ping ok');
      if (this.isRoute(AppRoute.RemoteChat) && this.activeSession.sessionId.length > 0) {
        this.pollActiveSession();
      }
    } catch (err) {
      RemoteLogger.warn(`foreground ping failed ${ConnectionErrorPolicy.errorText(err)}`);
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
      this.stopHeartbeat();
      this.setRemoteConnectionState(ConnectionState.Reconnecting);
      await this.reconnectAfterForeground();
    }
  }

  private async reconnectAfterForeground(): Promise<void> {
    const shouldRestoreChat = this.isRoute(AppRoute.RemoteChat) && this.activeSession.sessionId.length > 0;
    const previousSession: SessionSummary = {
      sessionId: this.activeSession.sessionId,
      title: this.activeSession.title,
      workspacePath: this.activeSession.workspacePath,
      agentType: this.activeSession.agentType
    };
    await this.connect(true);
    if (this.connectionState !== ConnectionState.Connected) {
      return;
    }
    if (shouldRestoreChat) {
      this.activeSession = previousSession;
      this.remotePageState.setActiveSession(previousSession);
      await this.loadActiveMessages();
      this.startPolling();
      this.nudgeChatPolling();
    }
  }

  private hasRemoteBindingForResume(): boolean {
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      this.connectionState !== ConnectionState.Idle &&
      this.connectionState !== ConnectionState.Parsing &&
      this.connectionState !== ConnectionState.Pairing &&
      this.connectionState !== ConnectionState.Disconnected;
  }

  private hasRemoteBindingForCodeHome(): boolean {
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      (this.connectionState === ConnectionState.Connected ||
        this.connectionState === ConnectionState.Reconnecting ||
        this.connectionState === ConnectionState.Pairing ||
        this.connectionState === ConnectionState.Parsing);
  }

  private shortSessionId(sessionId: string): string {
    if (sessionId.length <= 8) {
      return sessionId;
    }
    return sessionId.slice(0, 4) + '...' + sessionId.slice(sessionId.length - 4);
  }

  private async syncAfterTurnEnded(): Promise<void> {
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
