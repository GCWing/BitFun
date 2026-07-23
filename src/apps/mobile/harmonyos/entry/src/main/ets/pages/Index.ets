import {
  AssistantEntry,
  ChatMessage,
  ChatMessageItemResponse,
  InitialSyncResult,
  RecentWorkspaceEntry,
  ReadFileResult,
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
import { ChatSessionController, ChatSessionCursor, ChatSessionSnapshot } from '../services/ChatSessionController';
import { ChatTimelineItem } from '../services/ChatTimelineProjector';
import { ChatTimelineState, ChatTimelineStore } from '../services/ChatTimelineStore';
import { ConnectionErrorPolicy, ConnectionErrorResult } from '../services/ConnectionErrorPolicy';
import { Encoding } from '../services/Encoding';
import { ImagePickerService } from '../services/ImagePickerService';
import { GeneralChatBackend, GeneralChatSendResult } from '../services/GeneralChatBackend';
import { MobileIdentitySnapshot, MobileIdentityStore } from '../services/MobileIdentityStore';
import { RemoteDescriptorParser } from '../services/RemoteDescriptorParser';
import { RemoteLogger } from '../services/RemoteLogger';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { QrScanService } from '../services/QrScanService';
import { RemoteUiState } from '../services/RemoteUiState';
import { VoiceInputService, VoiceInputState } from '../services/VoiceInputService';
import { ChatHomeView } from './components/ChatHomeView';
import { ChatView } from './components/ChatView';
import { ConnectView } from './components/ConnectView';
import { HomeView } from './components/HomeView';
import { PAGE_BG } from './components/Theme';

enum ScreenMode {
  ChatHome = 'chatHome',
  GeneralChat = 'generalChat',
  CodeHome = 'codeHome',
  CodeConnect = 'codeConnect',
  CodeChat = 'codeChat'
}

enum ConnectionState {
  Idle = 'idle',
  Parsing = 'parsing',
  Pairing = 'pairing',
  Connected = 'connected',
  Reconnecting = 'reconnecting',
  Failed = 'failed',
  Disconnected = 'disconnected'
}

@Entry
@Component
struct Index {
  private readonly sessionManager: RemoteSessionManager = new RemoteSessionManager();
  private readonly identityStore: MobileIdentityStore = new MobileIdentityStore();
  private readonly clipboardService: ClipboardService = new ClipboardService();
  private readonly qrScanService: QrScanService = new QrScanService();
  private readonly imagePickerService: ImagePickerService = new ImagePickerService();
  private readonly generalChatBackend: GeneralChatBackend = new GeneralChatBackend();
  private readonly chatTimelineStore: ChatTimelineStore = new ChatTimelineStore();
  private readonly voiceInputService: VoiceInputService = new VoiceInputService();
  private chatSessionController?: ChatSessionController;
  private deviceId: string = Encoding.randomId('harmony');
  private heartbeatTimerId: number = 0;
  private autoReconnectAttempted: boolean = false;
  private isSyncingAfterTurn: boolean = false;
  private readonly sessionPageSize: number = 8;
  private sessionOffset: number = 0;
  private knownPollVersion: number = 0;
  private knownModelCatalogVersion: number = 0;
  private knownRemoteMessageCount: number = 0;
  private generalChatStreamTimerId: number = 0;
  private generalChatStreamSessionId: string = '';
  private generalChatStreamTurnId: string = '';
  private userIdFailureCount: number = 0;
  private userIdLockUntil: number = 0;

  @State mode: ScreenMode = ScreenMode.ChatHome;
  @State remoteUrl: string = '';
  @State userId: string = '';
  @State authenticatedUserId: string = '';
  @State statusText: string = RemoteI18n.t('status.waitingConnection');
  @State connectionState: ConnectionState = ConnectionState.Idle;
  @State connectionFailureKind: string = '';
  @State isBusy: boolean = false;
  @State showRemoteUrlInput: boolean = false;
  @State workspaceName: string = RemoteI18n.t('status.notConnected');
  @State workspacePath: string = '';
  @State workspaceBranch: string = '';
  @State workspaceKind: string = 'normal';
  @State assistantId: string = '';
  @State recentWorkspaces: RecentWorkspaceEntry[] = [];
  @State assistants: AssistantEntry[] = [];
  @State showWorkspacePicker: boolean = false;
  @State showAssistantPicker: boolean = false;
  @State desktopName: string = 'MacBook Pro';
  @State desktopId: string = '';
  @State sessions: RemoteSession[] = [];
  @State generalChatSessions: RemoteSession[] = [];
  @State sessionQuery: string = '';
  @State sessionFilter: string = 'all';
  @State hasMoreSessions: boolean = false;
  @State activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @State chatInput: string = '';
  @State messages: ChatMessage[] = [];
  @State pendingMessages: ChatMessage[] = [];
  @State hasMoreMessages: boolean = false;
  @State activeTurnMessage: ChatMessage = RemoteUiState.emptyActiveTurn();
  @State timelineItems: ChatTimelineItem[] = [];
  @State modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @State selectedModelId: string = '';
  @State downloadingFilePath: string = '';
  @State downloadedFilePath: string = '';
  @State fileDownloadStatus: string = '';
  @State selectedImages: SelectedImageAttachment[] = [];
  @State isVoiceListening: boolean = false;

  async aboutToAppear(): Promise<void> {
    await this.restoreIdentity();
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${this.connectionState} mode=${this.mode}`);
    this.resumeRemoteActivity();
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${this.connectionState} mode=${this.mode}`);
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.stopHeartbeat();
  }

  aboutToDisappear(): void {
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.stopHeartbeat();
    this.voiceInputService.cancel();
    this.isVoiceListening = false;
  }

  build() {
    Stack({ alignContent: Alignment.Bottom }) {
      Column() {
        if (this.mode === ScreenMode.ChatHome) {
          ChatHomeView({
            chatInput: $chatInput,
            sessions: this.homeRecentSessions(),
            isBusy: this.isBusy,
            selectedImages: this.selectedImages,
            isVoiceListening: this.isVoiceListening,
            connectionState: this.connectionState,
            onEnterCode: () => {
              this.enterCodeEntry();
            },
            onOpenSession: (session: RemoteSession) => {
              this.openHomeSession(session);
            },
            onPickImages: () => {
              this.pickImages();
            },
            onVoiceInput: () => {
              this.toggleVoiceInput();
            },
            onSendGeneral: () => {
              this.sendGeneralChatFromHome();
            },
            onSendCodeIntent: () => {
              this.sendGeneralCodeAdviceFromHome();
            }
          })
        } else if (this.mode === ScreenMode.CodeConnect) {
          ConnectView({
            remoteUrl: $remoteUrl,
            userId: $userId,
            showRemoteUrlInput: $showRemoteUrlInput,
            statusText: this.statusText,
            connectionState: this.connectionState,
            connectionFailureKind: this.connectionFailureKind,
            isBusy: this.isBusy,
            isConnected: this.connectionState === ConnectionState.Connected,
            desktopName: this.desktopName,
            desktopId: this.desktopId,
            deviceId: this.deviceId,
            onBack: () => {
              this.mode = ScreenMode.CodeHome;
            },
            onConnect: () => {
              this.connect();
            },
            onClearPairing: () => {
              this.disconnect(true);
            },
            onRemoteUrlChange: (value: string) => {
              this.syncRemoteDesktopIdentity(value);
            },
            onPasteRemoteUrl: () => {
              this.pasteRemoteUrl();
            },
            onScanRemoteUrl: () => {
              this.scanRemoteUrl();
            }
          })
        } else if (this.mode === ScreenMode.CodeHome) {
          HomeView({
            desktopName: this.desktopName,
            workspaceName: this.workspaceName,
            workspacePath: this.workspacePath,
            workspaceBranch: this.workspaceBranch,
            workspaceKind: this.workspaceKind,
            assistantId: this.assistantId,
            authenticatedUserId: this.authenticatedUserId,
            statusText: this.statusText,
            connectionState: this.connectionState,
            sessions: this.sessions,
            sessionQuery: $sessionQuery,
            sessionFilter: this.sessionFilter,
            hasMoreSessions: this.hasMoreSessions,
            isBusy: this.isBusy,
            recentWorkspaces: this.recentWorkspaces,
            assistants: this.assistants,
            showWorkspacePicker: this.showWorkspacePicker,
            showAssistantPicker: this.showAssistantPicker,
            onBack: () => {
              this.mode = ScreenMode.ChatHome;
            },
            onConnectWorkspace: () => {
              this.mode = ScreenMode.CodeConnect;
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
              this.showWorkspacePicker = false;
            },
            onCancelAssistantPicker: () => {
              this.showAssistantPicker = false;
            },
            onSearchSessions: () => {
              this.refreshSessions();
            },
            onSetSessionFilter: (filter: string) => {
              this.setSessionFilter(filter);
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
            onOpenSession: (session: RemoteSession) => {
              this.openSession(session);
            },
            onDeleteSession: (session: RemoteSession) => {
              this.deleteSession(session);
            }
          })
        } else {
          ChatView({
            activeSession: this.activeSession,
            workspaceName: this.mode === ScreenMode.GeneralChat ? 'BitFun' : this.workspaceName,
            workspaceBranch: this.mode === ScreenMode.GeneralChat ? '' : this.workspaceBranch,
            statusText: this.statusText,
            connectionState: this.mode === ScreenMode.GeneralChat ? ConnectionState.Connected : this.connectionState,
            isBusy: this.isBusy,
            canStop: this.hasRunningActiveTurn(),
            hasMoreMessages: this.hasMoreMessages,
            timelineItems: this.timelineItems,
            modelCatalog: this.modelCatalog,
            selectedModelId: this.selectedModelId,
            downloadingFilePath: this.downloadingFilePath,
            downloadedFilePath: this.downloadedFilePath,
            fileDownloadStatus: this.fileDownloadStatus,
            selectedImages: this.selectedImages,
            isVoiceListening: this.isVoiceListening,
            chatInput: $chatInput,
            onBack: () => {
              this.closeActiveChat();
            },
            onStop: () => {
              this.stopActiveChatTask();
            },
            onLoadOlder: () => {
              this.loadOlderMessages();
            },
            onApproveTool: (toolId: string) => {
              this.approveTool(toolId);
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
            }
          })
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor(PAGE_BG)
    }
    .width('100%')
    .height('100%')
  }

  private async restoreIdentity(): Promise<void> {
    try {
      const snapshot: MobileIdentitySnapshot = await this.identityStore.init(getContext(this));
      this.deviceId = snapshot.installId;
      this.userId = snapshot.userId || snapshot.installId;
      this.remoteUrl = snapshot.remoteUrl;
      this.syncRemoteDesktopIdentity(snapshot.remoteUrl);
      this.selectedModelId = await this.identityStore.getLastModelId();
      this.userIdFailureCount = snapshot.userIdFailureCount;
      this.userIdLockUntil = snapshot.userIdLockUntil > Date.now() ? snapshot.userIdLockUntil : 0;
      if (snapshot.userIdLockUntil > 0 && this.userIdLockUntil === 0) {
        this.userIdFailureCount = 0;
        await this.identityStore.clearUserIdProtection();
      }
      this.showRemoteUrlInput = snapshot.remoteUrl.length > 0;
      if (this.shouldAutoReconnect()) {
        this.autoReconnectAttempted = true;
        this.connectionState = ConnectionState.Reconnecting;
        this.statusText = RemoteI18n.t('status.restoringConnection');
        this.connect(true);
      }
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
      this.connectionState = ConnectionState.Failed;
    }
  }

  private shouldAutoReconnect(): boolean {
    return !this.autoReconnectAttempted &&
      this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0;
  }

  private async connect(autoReconnect: boolean = false): Promise<void> {
    if (this.isBusy) {
      return;
    }
    try {
      this.isBusy = true;
      this.connectionFailureKind = '';
      if (this.userIdLockUntil > 0 && !ConnectionErrorPolicy.isLocked(this.userIdLockUntil)) {
        this.userIdFailureCount = 0;
        this.userIdLockUntil = 0;
        await this.identityStore.clearUserIdProtection();
      }
      if (!autoReconnect && ConnectionErrorPolicy.isLocked(this.userIdLockUntil)) {
        throw new Error(RemoteI18n.f('errors.tooManyAttempts', `${ConnectionErrorPolicy.remainingLockSeconds(this.userIdLockUntil)}`));
      }
      this.connectionState = this.connectionState === ConnectionState.Reconnecting ? ConnectionState.Reconnecting : ConnectionState.Parsing;
      this.statusText = RemoteI18n.t('status.parsingUrl');
      const descriptor = RemoteDescriptorParser.parse(this.remoteUrl);
      const normalizedUserId = this.userId.trim() || this.deviceId;
      this.userId = normalizedUserId;

      this.syncRemoteDesktopIdentity(this.remoteUrl);
      this.connectionState = ConnectionState.Pairing;
      this.statusText = RemoteI18n.t('status.pairing');
      const initialSync: InitialSyncResult = await this.sessionManager.connect(
        descriptor,
        normalizedUserId,
        this.deviceId
      );
      await this.identityStore.savePairingInput(normalizedUserId, this.remoteUrl);
      await this.identityStore.clearUserIdProtection();
      this.userIdFailureCount = 0;
      this.userIdLockUntil = 0;
      this.applyWorkspace(initialSync.workspace);
      this.authenticatedUserId = initialSync.authenticatedUserId;
      this.sessions = initialSync.sessions;
      this.hasMoreSessions = initialSync.hasMoreSessions;
      this.sessionOffset = this.sessions.length;
      this.connectionState = ConnectionState.Connected;
      this.connectionFailureKind = '';
      this.statusText = RemoteI18n.t('connection.connected');
      if (!autoReconnect) {
        this.mode = ScreenMode.CodeHome;
      }
      this.startHeartbeat();
    } catch (err) {
      const result: ConnectionErrorResult = await ConnectionErrorPolicy.connectionErrorResult(err, autoReconnect, {
        failureCount: this.userIdFailureCount,
        lockUntil: this.userIdLockUntil
      }, this.identityStore);
      this.statusText = result.message;
      this.connectionFailureKind = result.failureKind;
      this.userIdFailureCount = result.failureCount;
      this.userIdLockUntil = result.lockUntil;
      if (result.shouldShowRemoteUrlInput) {
        this.showRemoteUrlInput = true;
      }
      this.connectionState = ConnectionState.Failed;
    } finally {
      this.isBusy = false;
    }
  }

  private async reconnect(): Promise<void> {
    if (this.isBusy) {
      return;
    }
    this.connectionState = ConnectionState.Reconnecting;
    this.statusText = RemoteI18n.t('status.reconnecting');
    await this.connect();
  }

  private async disconnect(clearPairing: boolean): Promise<void> {
    this.stopPolling();
    this.stopHeartbeat();
    this.sessionManager.reset();
    this.sessions = [];
    this.hasMoreSessions = false;
    this.sessionOffset = 0;
    this.resetChatTimeline('');
    this.hasMoreMessages = false;
    this.modelCatalog = RemoteUiState.emptyModelCatalog();
    this.knownPollVersion = 0;
    this.knownModelCatalogVersion = 0;
    this.knownRemoteMessageCount = 0;
    this.downloadingFilePath = '';
    this.downloadedFilePath = '';
    this.fileDownloadStatus = '';
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
    this.authenticatedUserId = '';
    this.recentWorkspaces = [];
    this.assistants = [];
    this.showWorkspacePicker = false;
    this.showAssistantPicker = false;
    this.connectionState = ConnectionState.Disconnected;
    this.connectionFailureKind = '';
    this.statusText = clearPairing ? RemoteI18n.t('status.pairingCleared') : RemoteI18n.t('status.disconnected');
    this.mode = ScreenMode.CodeHome;
    if (clearPairing) {
      this.remoteUrl = '';
      this.userId = this.deviceId;
      this.showRemoteUrlInput = false;
      await this.identityStore.clearPairingInput();
    } else {
      this.showRemoteUrlInput = this.remoteUrl.trim().length > 0;
    }
  }

  private async pasteRemoteUrl(): Promise<void> {
    try {
      this.statusText = RemoteI18n.t('status.readClipboard');
      const text = (await this.clipboardService.readText()).trim();
      if (text.length === 0) {
        this.showRemoteUrlInput = true;
        this.statusText = RemoteI18n.t('status.clipboardEmpty');
        return;
      }
      this.remoteUrl = text;
      this.syncRemoteDesktopIdentity(text);
      this.connectionFailureKind = '';
      this.showRemoteUrlInput = true;
      try {
        RemoteDescriptorParser.parse(text);
        this.statusText = RemoteI18n.t('status.clipboardUrlFilled');
      } catch (_err) {
        this.statusText = RemoteI18n.t('status.clipboardNeedsConfirm');
      }
    } catch (err) {
      this.showRemoteUrlInput = true;
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private async scanRemoteUrl(): Promise<void> {
    try {
      this.statusText = RemoteI18n.t('status.openScanner');
      const text = (await this.qrScanService.scanRemoteUrl(getContext(this))).trim();
      if (text.length === 0) {
        this.statusText = RemoteI18n.t('status.scanEmpty');
        return;
      }
      this.remoteUrl = text;
      this.syncRemoteDesktopIdentity(text);
      this.connectionFailureKind = '';
      this.showRemoteUrlInput = true;
      RemoteDescriptorParser.parse(text);
      this.statusText = RemoteI18n.t('status.scannedUrl');
      await this.connect();
    } catch (err) {
      this.showRemoteUrlInput = true;
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private applyWorkspace(workspace: WorkspaceInfo): void {
    this.workspaceName = workspace.name;
    this.workspacePath = workspace.path;
    this.workspaceBranch = workspace.gitBranch;
    this.workspaceKind = workspace.workspaceKind || 'normal';
    this.assistantId = workspace.assistantId || '';
  }

  private syncRemoteDesktopIdentity(remoteUrl: string): void {
    if (remoteUrl.trim().length === 0) {
      this.desktopId = '';
      return;
    }
    this.desktopName = RemoteUiState.desktopNameFromRemoteUrl(remoteUrl);
    this.desktopId = RemoteUiState.desktopIdFromRemoteUrl(remoteUrl);
  }

  private ensureRemoteAvailable(): boolean {
    if (RemoteUiState.canUseRemote(this.connectionState)) {
      return true;
    }
    this.statusText = RemoteI18n.t('status.remoteUnavailable');
    return false;
  }

  private failRemoteConnection(err: Object): void {
    this.statusText = ConnectionErrorPolicy.errorText(err);
    this.connectionState = ConnectionState.Failed;
    this.stopHeartbeat();
  }

  private async showRecentWorkspaces(): Promise<void> {
    if (this.showWorkspacePicker) {
      this.showWorkspacePicker = false;
      return;
    }
    this.showWorkspacePicker = true;
    this.showAssistantPicker = false;
    await this.loadRecentWorkspaces();
  }

  private async showAssistants(): Promise<void> {
    if (this.showAssistantPicker) {
      this.showAssistantPicker = false;
      return;
    }
    this.showAssistantPicker = true;
    this.showWorkspacePicker = false;
    await this.loadAssistants();
  }

  private async loadRecentWorkspaces(): Promise<void> {
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.loadingRecentWorkspaces');
      this.recentWorkspaces = await this.sessionManager.listRecentWorkspaces();
      this.statusText = this.recentWorkspaces.length > 0 ? RemoteI18n.t('status.chooseWorkspace') : RemoteI18n.t('status.noRecentWorkspaces');
    } catch (err) {
      this.failRemoteConnection(err);
    } finally {
      this.isBusy = false;
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
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.switchingWorkspace');
      await this.sessionManager.setWorkspace(path);
      const workspace = await this.sessionManager.getWorkspaceInfo();
      this.applyWorkspace(workspace);
      this.showWorkspacePicker = false;
      this.showAssistantPicker = false;
      this.sessionOffset = 0;
      this.sessions = [];
      this.hasMoreSessions = false;
      this.statusText = RemoteI18n.t('status.workspaceSwitched');
      this.isBusy = false;
      await this.refreshSessions();
    } catch (err) {
      this.failRemoteConnection(err);
      this.isBusy = false;
    }
  }

  private async loadAssistants(): Promise<void> {
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.loadingAssistants');
      this.assistants = await this.sessionManager.listAssistants();
      this.statusText = this.assistants.length > 0 ? RemoteI18n.t('status.chooseAssistant') : RemoteI18n.t('status.noAssistants');
    } catch (err) {
      this.failRemoteConnection(err);
    } finally {
      this.isBusy = false;
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
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.switchingAssistant');
      await this.sessionManager.setAssistant(path);
      const workspace = await this.sessionManager.getWorkspaceInfo();
      this.applyWorkspace(workspace);
      this.showAssistantPicker = false;
      this.showWorkspacePicker = false;
      this.sessionOffset = 0;
      this.sessions = [];
      this.hasMoreSessions = false;
      this.statusText = RemoteI18n.t('status.assistantSwitched');
      this.isBusy = false;
      await this.refreshSessions();
    } catch (err) {
      this.failRemoteConnection(err);
      this.isBusy = false;
    }
  }

  private async refreshSessions(): Promise<void> {
    try {
      if (!this.ensureRemoteAvailable()) {
        return;
      }
      if (this.connectionState !== ConnectionState.Connected) {
        this.connectionState = ConnectionState.Reconnecting;
      }
      this.statusText = RemoteI18n.t('status.refreshSessions');
      this.sessionOffset = 0;
      const result = await this.sessionManager.listSessions(
        this.sessionPageSize,
        this.sessionOffset,
        this.sessionQuery,
        this.sessionFilter
      );
      this.sessions = result.sessions;
      this.hasMoreSessions = result.hasMore;
      this.sessionOffset = this.sessions.length;
      this.connectionState = ConnectionState.Connected;
      this.statusText = RemoteI18n.t('connection.connected');
      this.startHeartbeat();
    } catch (err) {
      this.failRemoteConnection(err);
    }
  }

  private async loadMoreSessions(): Promise<void> {
    if (this.isBusy || !this.hasMoreSessions) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.loadMoreSessions');
      const result = await this.sessionManager.listSessions(
        this.sessionPageSize,
        this.sessionOffset,
        this.sessionQuery,
        this.sessionFilter
      );
      this.sessions = RemoteUiState.mergeSessions(this.sessions, result.sessions);
      this.hasMoreSessions = result.hasMore;
      this.sessionOffset = this.sessions.length;
      this.statusText = RemoteI18n.t('status.sessionsUpdated');
    } catch (err) {
      this.failRemoteConnection(err);
    } finally {
      this.isBusy = false;
    }
  }

  private setSessionFilter(filter: string): void {
    if (this.sessionFilter === filter) {
      return;
    }
    this.sessionFilter = filter;
    this.refreshSessions();
  }

  private homeRecentSessions(): RemoteSession[] {
    return this.generalChatSessions.concat(this.sessions);
  }

  private enterCodeEntry(): void {
    if (this.hasRemoteBindingForCodeHome()) {
      this.mode = ScreenMode.CodeHome;
      return;
    }
    this.mode = ScreenMode.CodeConnect;
  }

  private openHomeSession(session: RemoteSession): void {
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openSession(session);
  }

  private openGeneralSession(item: RemoteSession): void {
    if (this.isBusy) {
      return;
    }
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.resetChatTimeline(item.id);
    this.activeSession = {
      sessionId: item.id,
      title: item.title,
      workspacePath: '',
      agentType: 'chat'
    };
    this.hasMoreMessages = false;
    this.modelCatalog = RemoteUiState.emptyModelCatalog();
    this.selectedImages = [];
    this.chatTimelineStore.setPersistedMessages(this.generalChatBackend.messages(item.id));
    this.syncChatTimelineFromStore();
    this.statusText = RemoteI18n.t('generalChat.ready');
    this.mode = ScreenMode.GeneralChat;
  }

  private startGeneralChat(text: string): void {
    const trimmed = text.trim();
    if (trimmed.length === 0 || this.isBusy) {
      return;
    }
    const session = this.generalChatBackend.createSession(trimmed);
    this.generalChatSessions = this.generalChatBackend.listSessions();
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.resetChatTimeline(session.id);
    this.activeSession = {
      sessionId: session.id,
      title: session.title,
      workspacePath: '',
      agentType: 'chat'
    };
    this.hasMoreMessages = false;
    this.modelCatalog = RemoteUiState.emptyModelCatalog();
    this.mode = ScreenMode.GeneralChat;
    this.sendGeneralChatMessage();
  }

  private async sendVisibleChatMessage(): Promise<void> {
    if (this.mode === ScreenMode.GeneralChat) {
      this.sendGeneralChatMessage();
      return;
    }
    await this.sendChatMessage();
  }

  private async stopActiveChatTask(): Promise<void> {
    if (this.mode === ScreenMode.GeneralChat) {
      this.stopGeneralChatStream(true);
      return;
    }
    await this.stopActiveTask();
  }

  private closeActiveChat(): void {
    this.stopVoiceInput(false);
    if (this.mode === ScreenMode.GeneralChat) {
      this.stopGeneralChatStream(false);
      this.mode = ScreenMode.ChatHome;
      return;
    }
    this.stopPolling();
    this.mode = ScreenMode.CodeHome;
  }

  private async renameVisibleSession(title: string): Promise<void> {
    if (this.mode === ScreenMode.GeneralChat) {
      const sessionId = this.activeSession.sessionId || '';
      const nextTitle = title.trim();
      if (!sessionId || nextTitle.length === 0 || nextTitle === this.activeSession.title) {
        return;
      }
      this.generalChatBackend.renameSession(sessionId, nextTitle);
      this.generalChatSessions = this.generalChatBackend.listSessions();
      this.activeSession = {
        sessionId: this.activeSession.sessionId,
        title: nextTitle,
        workspacePath: '',
        agentType: 'chat'
      };
      this.statusText = RemoteI18n.t('status.titleUpdated');
      return;
    }
    await this.renameActiveSession(title);
  }

  private retryVisibleMessage(text: string): void {
    if (this.mode === ScreenMode.GeneralChat) {
      if (this.isBusy) {
        return;
      }
      this.chatInput = text;
      this.sendGeneralChatMessage();
      return;
    }
    this.retryMessage(text);
  }

  private downloadVisibleFile(path: string): void {
    if (this.mode === ScreenMode.GeneralChat) {
      this.statusText = RemoteI18n.t('generalChat.fileDownloadMock');
      return;
    }
    this.downloadFile(path);
  }

  private async createSession(agentType: string): Promise<void> {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.createSession');
      const session = await this.sessionManager.createSession({
        agentType,
        title: '',
        instruction: ''
      });

      this.activeSession = session;
      this.chatInput = '';
      this.selectedImages = [];
      this.hasMoreMessages = false;
      this.resetChatTimeline(session.sessionId);
      this.knownPollVersion = 0;
      this.knownRemoteMessageCount = 0;
      this.statusText = RemoteI18n.t('status.sessionCreated');
      this.mode = ScreenMode.CodeChat;
      await this.loadModelCatalog(session.sessionId);
      await this.refreshSessions();
      this.mode = ScreenMode.CodeChat;
      await this.loadActiveMessages();
      this.startPolling();
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async openSession(item: RemoteSession): Promise<void> {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.loadMessages');
      this.stopPolling();
      this.resetChatTimeline(item.id);
      this.knownPollVersion = 0;
      this.knownRemoteMessageCount = 0;
      this.hasMoreMessages = false;
      this.downloadingFilePath = '';
      this.downloadedFilePath = '';
      this.fileDownloadStatus = '';
      this.selectedImages = [];
      this.activeSession = {
        sessionId: item.id,
        title: item.title,
        workspacePath: this.workspacePath,
        agentType: item.agentType
      };
      this.mode = ScreenMode.CodeChat;
      await this.loadModelCatalog(item.id);
      await this.loadActiveMessages();
      if (this.activeSession.sessionId === item.id && this.mode === ScreenMode.CodeChat) {
        this.startPolling();
      }
    } finally {
      this.isBusy = false;
    }
  }

  private async deleteSession(item: RemoteSession): Promise<void> {
    if (this.isBusy || item.id.length === 0) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.deletingSession');
      await this.sessionManager.deleteSession(item.id);
      this.sessions = this.sessions.filter((session: RemoteSession) => session.id !== item.id);
      this.sessionOffset = this.sessions.length;
      if (this.activeSession.sessionId === item.id) {
        this.stopPolling();
        this.resetChatTimeline('');
        this.hasMoreMessages = false;
        this.selectedImages = [];
        this.modelCatalog = RemoteUiState.emptyModelCatalog();
        this.knownPollVersion = 0;
        this.knownModelCatalogVersion = 0;
        this.knownRemoteMessageCount = 0;
        this.activeSession = {
          sessionId: '',
          title: '',
          workspacePath: this.workspacePath,
          agentType: 'code'
        };
        this.mode = ScreenMode.CodeHome;
      }
      this.statusText = RemoteI18n.t('status.sessionDeleted');
      if (this.hasMoreSessions && this.sessions.length < this.sessionPageSize) {
        this.isBusy = false;
        await this.loadMoreSessions();
      }
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async loadActiveMessages(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    try {
      this.statusText = RemoteI18n.t('status.loadMessages');
      const result = await this.sessionManager.getSessionMessages(sessionId);
      if (this.activeSession.sessionId !== sessionId || this.mode !== ScreenMode.CodeChat) {
        return;
      }
      this.chatTimelineStore.setPersistedMessages(result.messages);
      this.hasMoreMessages = false;
      this.syncChatTimelineFromStore();
      this.knownRemoteMessageCount = result.messages.length;
      this.updateChatPollingCursor(0, result.messages.length);
      this.statusText = RemoteI18n.t('status.messagesSynced');
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
      this.knownRemoteMessageCount = 0;
      this.updateChatPollingCursor(0, 0);
    }
  }

  private async loadModelCatalog(sessionId: string): Promise<void> {
    if (!sessionId || !this.ensureRemoteAvailable()) {
      return;
    }
    this.modelCatalog = RemoteUiState.emptyModelCatalog();
    this.knownModelCatalogVersion = 0;
    try {
      const catalog = await this.sessionManager.getModelCatalog(sessionId);
      if (this.activeSession.sessionId !== sessionId || this.mode !== ScreenMode.CodeChat) {
        return;
      }
      this.applyModelCatalog(catalog);
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private async selectModel(modelId: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId || modelId.length === 0 || modelId === this.selectedModelId || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.switchingModel');
      const selected = await this.sessionManager.setSessionModel(sessionId, modelId);
      this.selectedModelId = selected;
      await this.identityStore.saveLastModelId(selected);
      this.modelCatalog = {
        version: this.modelCatalog.version,
        models: this.modelCatalog.models,
        default_models: this.modelCatalog.default_models,
        session_model_id: selected
      };
      this.chatTimelineStore.setModelCatalog(this.modelCatalog, selected);
      this.statusText = RemoteI18n.t('status.modelSwitched');
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async loadOlderMessages(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId || !this.hasMoreMessages || this.isBusy) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.loadOlderMessages');
      const result = await this.sessionManager.getSessionMessages(sessionId);
      this.chatTimelineStore.setPersistedMessages(result.messages);
      this.hasMoreMessages = false;
      this.syncChatTimelineFromStore();
      this.knownRemoteMessageCount = result.messages.length;
      this.updateChatPollingCursor(this.knownPollVersion, this.knownRemoteMessageCount);
      this.statusText = RemoteI18n.t('status.messagesSynced');
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async sendGeneralChatMessage(): Promise<void> {
    if (this.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    const rawText = this.chatInput.trim();
    const images = this.selectedImages.slice();
    const text = rawText.length > 0 ? rawText : (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.isBusy) {
      return;
    }
    this.chatInput = '';
    this.selectedImages = [];
    const localMessage = RemoteUiState.localUserMessage(text, images);
    this.chatTimelineStore.appendOptimisticMessage(localMessage);
    this.syncChatTimelineFromStore();
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('generalChat.generating');
      const result: GeneralChatSendResult = this.generalChatBackend.sendMessage(sessionId, text, images);
      this.generalChatSessions = this.generalChatBackend.listSessions();
      this.chatTimelineStore.mergePersistedMessages([result.userMessage]);
      this.syncChatTimelineFromStore();
      this.streamGeneralAssistantMessage(sessionId, result.assistantMessage, result.streamChunks, result.tools);
    } catch (err) {
      this.chatInput = rawText;
      this.selectedImages = images;
      this.chatTimelineStore.markOptimisticMessageFailed(localMessage.id);
      this.syncChatTimelineFromStore();
      this.statusText = ConnectionErrorPolicy.errorText(err);
      this.isBusy = false;
    }
  }

  private streamGeneralAssistantMessage(
    sessionId: string,
    assistantMessage: ChatMessage,
    chunks: string[],
    tools: RemoteToolStatusResponse[]
  ): void {
    this.stopGeneralChatStream(false);
    const fullText = assistantMessage.text || '';
    const streamChunks = chunks.length > 0 ? chunks : [fullText];
    const completedTools = tools.slice();
    const runningTools = completedTools.map((tool: RemoteToolStatusResponse) => {
      const runningTool: RemoteToolStatusResponse = {
        id: tool.id,
        name: tool.name,
        status: 'running',
        duration_ms: 0,
        input_preview: tool.input_preview,
        tool_input: tool.tool_input
      };
      return runningTool;
    });
    const turnId = assistantMessage.turnId || assistantMessage.id;
    this.generalChatStreamSessionId = sessionId;
    this.generalChatStreamTurnId = turnId;
    let toolStage = completedTools.length > 0 ? 0 : 2;
    let chunkIndex = 0;
    let visibleText = '';
    const tick = () => {
      if (this.generalChatStreamSessionId !== sessionId || this.generalChatStreamTurnId !== turnId) {
        return;
      }
      if (this.activeSession.sessionId !== sessionId || this.mode !== ScreenMode.GeneralChat) {
        this.stopGeneralChatStream(false);
        return;
      }
      if (toolStage < 2) {
        const activeTurn: ChatMessage = {
          id: `active-${turnId}`,
          turnId,
          role: 'assistant',
          text: '',
          status: 'active',
          detail: '',
          tools: toolStage === 0 ? runningTools : completedTools,
          items: this.generalAssistantItems(toolStage === 0 ? runningTools : completedTools, ''),
          renderVersion: toolStage
        };
        this.chatTimelineStore.setActiveTurn(activeTurn);
        this.syncChatTimelineFromStore();
        toolStage += 1;
        this.generalChatStreamTimerId = setTimeout(tick, toolStage === 1 ? 480 : 260);
        return;
      }
      if (chunkIndex < streamChunks.length) {
        visibleText += streamChunks[chunkIndex];
        chunkIndex += 1;
      }
      const isComplete = chunkIndex >= streamChunks.length;
      const activeTurn: ChatMessage = {
        id: `active-${turnId}`,
        turnId,
        role: 'assistant',
        text: isComplete ? fullText : visibleText,
        status: isComplete ? 'completed' : 'active',
        detail: '',
        tools: completedTools,
        items: this.generalAssistantItems(completedTools, isComplete ? fullText : visibleText),
        renderVersion: visibleText.length
      };
      this.chatTimelineStore.setActiveTurn(activeTurn);
      this.syncChatTimelineFromStore();
      if (isComplete) {
        this.chatTimelineStore.mergePersistedMessages([assistantMessage]);
        this.chatTimelineStore.clearActiveTurn();
        this.syncChatTimelineFromStore();
        this.generalChatStreamTimerId = 0;
        this.generalChatStreamSessionId = '';
        this.generalChatStreamTurnId = '';
        this.isBusy = false;
        this.statusText = RemoteI18n.t('generalChat.ready');
        return;
      }
      this.generalChatStreamTimerId = setTimeout(tick, this.generalChatStreamDelay(streamChunks[chunkIndex - 1]));
    };
    tick();
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

  private generalChatStreamDelay(chunk: string): number {
    if (chunk.indexOf('\n') >= 0) {
      return 150;
    }
    if (chunk.length >= 12) {
      return 95;
    }
    return 70;
  }

  private stopGeneralChatStream(cancelled: boolean): void {
    const hasGeneralStream = this.generalChatStreamTimerId !== 0 ||
      this.generalChatStreamSessionId.length > 0 ||
      this.generalChatStreamTurnId.length > 0;
    if (this.mode !== ScreenMode.GeneralChat && !hasGeneralStream) {
      return;
    }
    const hadStream = hasGeneralStream ||
      (this.mode === ScreenMode.GeneralChat && this.activeTurnMessage.id.length > 0);
    if (this.generalChatStreamTimerId !== 0) {
      clearTimeout(this.generalChatStreamTimerId);
      this.generalChatStreamTimerId = 0;
    }
    this.generalChatStreamSessionId = '';
    this.generalChatStreamTurnId = '';
    if (cancelled && this.activeTurnMessage.id.length > 0) {
      const stoppedMessage: ChatMessage = {
        id: Encoding.randomId('assistant'),
        turnId: this.currentActiveTurnId(),
        role: 'assistant',
        text: this.activeTurnMessage.text || RemoteI18n.t('generalChat.stopped'),
        status: 'cancelled',
        detail: '',
        timestamp: new Date().toISOString()
      };
      this.chatTimelineStore.mergePersistedMessages([stoppedMessage]);
      this.statusText = RemoteI18n.t('status.stopRequested');
    }
    if (this.mode === ScreenMode.GeneralChat) {
      this.chatTimelineStore.clearActiveTurn();
      this.syncChatTimelineFromStore();
      if (cancelled || hadStream) {
        this.isBusy = false;
      }
    }
  }

  private async sendChatMessage(): Promise<void> {
    if (this.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    const rawText = this.chatInput.trim();
    const images = this.selectedImages.slice();
    const text = rawText.length > 0 ? rawText : (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.chatInput = '';
    this.selectedImages = [];
    const localMessage = RemoteUiState.localUserMessage(text, images);
    this.chatTimelineStore.appendOptimisticMessage(localMessage);
    const pendingActiveId = this.chatTimelineStore.setPendingActiveTurn(localMessage.id);
    this.syncChatTimelineFromStore();
    RemoteLogger.info(`chat send queued session=${this.shortSessionId(sessionId)} pending=${pendingActiveId}`);
    this.startPolling();
    this.nudgeChatPolling();
    try {
      this.isBusy = true;
      this.statusText = images.length > 0 ? RemoteI18n.t('status.sendImage') : RemoteI18n.t('status.sendCommand');
      let turnId = '';
      if (images.length > 0) {
        const imageContexts: RemoteImageContext[] = this.imagePickerService.toRemoteContexts(images);
        turnId = await this.sessionManager.sendMessageWithImages(sessionId, text, this.activeSession.agentType, imageContexts);
      } else {
        turnId = await this.sessionManager.sendMessage(sessionId, text, this.activeSession.agentType);
      }
      if (turnId.length > 0) {
        this.chatTimelineStore.setLocalActiveTurn(turnId);
        this.syncChatTimelineFromStore();
      } else if (pendingActiveId.length > 0) {
        this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
        this.syncChatTimelineFromStore();
      }
      this.statusText = RemoteI18n.t('status.sentWaiting');
      this.nudgeChatPolling();
    } catch (err) {
      this.chatInput = rawText;
      this.selectedImages = images;
      this.chatTimelineStore.markOptimisticMessageFailed(localMessage.id);
      if (pendingActiveId.length > 0) {
        this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
      }
      this.syncChatTimelineFromStore();
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async toggleVoiceInput(): Promise<void> {
    if (this.isVoiceListening || this.voiceInputService.isListening()) {
      await this.stopVoiceInput(true);
      return;
    }
    if (this.isBusy || this.chatInput.trim().length > 0 || this.selectedImages.length > 0) {
      return;
    }
    try {
      this.statusText = RemoteI18n.t('status.voiceStarting');
      await this.voiceInputService.start(getContext(this), {
        onText: (text: string, isFinal: boolean) => {
          this.chatInput = text;
          if (isFinal) {
            this.statusText = RemoteI18n.t('status.voiceRecognized');
          }
        },
        onState: (state: VoiceInputState) => {
          this.handleVoiceInputState(state);
        },
        onError: (message: string) => {
          this.isVoiceListening = false;
          this.showVoiceInputError(message || RemoteI18n.t('errors.voiceInputUnavailable'));
        }
      });
    } catch (err) {
      this.isVoiceListening = false;
      this.showVoiceInputError(ConnectionErrorPolicy.errorText(err));
    }
  }

  private async stopVoiceInput(showStatus: boolean): Promise<void> {
    if (!this.isVoiceListening && !this.voiceInputService.isListening()) {
      return;
    }
    this.isVoiceListening = false;
    try {
      await this.voiceInputService.stop();
      if (showStatus) {
        this.statusText = this.chatInput.trim().length > 0 ?
          RemoteI18n.t('status.voiceRecognized') :
          RemoteI18n.t('status.voiceStopped');
      }
    } catch (err) {
      this.showVoiceInputError(ConnectionErrorPolicy.errorText(err));
    }
  }

  private handleVoiceInputState(state: VoiceInputState): void {
    this.isVoiceListening = state === 'listening';
    if (state === 'listening') {
      this.statusText = RemoteI18n.t('status.voiceListening');
      return;
    }
    if (this.statusText === RemoteI18n.t('status.voiceListening') ||
      this.statusText === RemoteI18n.t('status.voiceStarting')) {
      this.statusText = this.chatInput.trim().length > 0 ?
        RemoteI18n.t('status.voiceRecognized') :
        RemoteI18n.t('status.voiceStopped');
    }
  }

  private showVoiceInputError(message: string): void {
    const text = message.length > 0 ? message : RemoteI18n.t('errors.voiceInputUnavailable');
    this.statusText = text;
    try {
      this.getUIContext().getPromptAction().showToast({
        message: text,
        duration: 2600
      });
    } catch (_err) {
    }
  }

  private async pickImages(): Promise<void> {
    if (this.isBusy) {
      return;
    }
    if (this.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    try {
      this.statusText = RemoteI18n.t('status.pickImage');
      const picked = await this.imagePickerService.pickImages(3, this.selectedImages.length);
      if (picked.length === 0) {
        this.statusText = RemoteI18n.t('status.noImageSelected');
        return;
      }
      this.selectedImages = this.selectedImages.concat(picked);
      this.statusText = RemoteI18n.f('status.imagesSelected', `${this.selectedImages.length}`);
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private removeSelectedImage(imageId: string): void {
    this.selectedImages = this.selectedImages.filter((image: SelectedImageAttachment) => image.id !== imageId);
  }

  private sendGeneralChatFromHome(): void {
    const text = this.chatInput.trim();
    if (text.length === 0) {
      return;
    }
    this.startGeneralChat(text);
  }

  private sendGeneralCodeAdviceFromHome(): void {
    const text = this.chatInput.trim();
    if (text.length === 0) {
      return;
    }
    this.startGeneralChat(text);
  }

  private showHomeToast(message: string): void {
    try {
      this.getUIContext().getPromptAction().showToast({
        message,
        duration: 2600
      });
    } catch (_err) {
      this.statusText = message;
    }
  }

  private async stopActiveTask(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    if (this.activeTurnMessage.id.length === 0) {
      this.statusText = RemoteI18n.t('status.noRunningTask');
      return;
    }
    try {
      this.statusText = RemoteI18n.t('status.stoppingTask');
      await this.sessionManager.cancelTask(sessionId, this.currentActiveTurnId());
      this.statusText = RemoteI18n.t('status.stopRequested');
      this.pollActiveSession();
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private async renameActiveSession(title: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    const nextTitle = title.trim();
    if (!sessionId || nextTitle.length === 0 || nextTitle === this.activeSession.title || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = RemoteI18n.t('status.updatingTitle');
      await this.sessionManager.renameSession(sessionId, nextTitle);
      this.activeSession = {
        sessionId: this.activeSession.sessionId,
        title: nextTitle,
        workspacePath: this.activeSession.workspacePath,
        agentType: this.activeSession.agentType
      };
      this.sessions = RemoteUiState.updateSessionTitle(this.sessions, sessionId, nextTitle);
      this.statusText = RemoteI18n.t('status.titleUpdated');
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private async copyMessage(text: string): Promise<void> {
    if (text.trim().length === 0) {
      return;
    }
    try {
      await this.clipboardService.writeText(text);
      this.statusText = RemoteI18n.t('status.messageCopied');
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    }
  }

  private async downloadFile(path: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    const remotePath = RemoteUiState.normalizeRemoteFilePath(path);
    if (!sessionId || remotePath.length === 0 || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.downloadingFilePath = path;
      this.downloadedFilePath = '';
      this.fileDownloadStatus = RemoteI18n.t('status.fileInfo');
      const info = await this.sessionManager.getFileInfo(remotePath, sessionId);
      this.fileDownloadStatus = RemoteI18n.f('status.prepareDownload', RemoteUiState.formatBytes(info.size));
      const result: ReadFileResult = await this.sessionManager.readFile(remotePath, sessionId, (downloaded: number, total: number) => {
        this.fileDownloadStatus = `${RemoteUiState.formatBytes(downloaded)} / ${RemoteUiState.formatBytes(total)}`;
      });
      this.downloadedFilePath = path;
      this.fileDownloadStatus = RemoteI18n.f2('status.downloadDone', result.name, RemoteUiState.formatBytes(result.size));
      this.statusText = this.fileDownloadStatus;
    } catch (err) {
      this.fileDownloadStatus = ConnectionErrorPolicy.errorText(err);
      this.statusText = this.fileDownloadStatus;
    } finally {
      this.isBusy = false;
      setTimeout(() => {
        this.downloadingFilePath = '';
      }, 2500);
    }
  }

  private retryMessage(text: string): void {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.chatInput = text;
    this.sendChatMessage();
  }

  private async approveTool(toolId: string): Promise<void> {
    await this.runToolAction(toolId, RemoteI18n.t('status.approvingTool'), RemoteI18n.t('status.toolApproved'), () => {
      return this.sessionManager.confirmTool(toolId);
    });
  }

  private async rejectTool(toolId: string): Promise<void> {
    await this.runToolAction(toolId, RemoteI18n.t('status.rejectingTool'), RemoteI18n.t('status.toolRejected'), () => {
      return this.sessionManager.rejectTool(toolId, 'Rejected from HarmonyOS remote client');
    });
  }

  private async cancelTool(toolId: string): Promise<void> {
    await this.runToolAction(toolId, RemoteI18n.t('status.cancellingTool'), RemoteI18n.t('status.toolCancelRequested'), () => {
      return this.sessionManager.cancelTool(toolId, 'Cancelled from HarmonyOS remote client');
    });
  }

  private async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    await this.runToolAction(toolId, RemoteI18n.t('status.submittingAnswer'), RemoteI18n.t('status.answerSubmitted'), () => {
      return this.sessionManager.answerQuestion(toolId, answers);
    });
  }

  private async runToolAction(
    toolId: string,
    pendingText: string,
    successText: string,
    action: () => Promise<void>
  ): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId || toolId.length === 0 || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    try {
      this.isBusy = true;
      this.statusText = pendingText;
      await action();
      this.statusText = successText;
      this.pollActiveSession();
    } catch (err) {
      this.statusText = ConnectionErrorPolicy.errorText(err);
    } finally {
      this.isBusy = false;
    }
  }

  private resetChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.knownPollVersion = 0;
    this.syncChatTimelineFromStore();
  }

  private syncChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshot();
    this.messages = state.persistedMessages;
    this.pendingMessages = state.optimisticMessages;
    this.activeTurnMessage = state.activeTurn || RemoteUiState.emptyActiveTurn();
    this.modelCatalog = state.modelCatalog;
    this.selectedModelId = state.selectedModelId;
    this.timelineItems = this.projectedTimelineItems();
  }

  private startPolling(): void {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    this.ensureChatSessionController().start(sessionId, this.currentChatPollingCursor(), this.activeTurnMessage);
  }

  private stopPolling(): void {
    this.chatSessionController?.stop();
  }

  private nudgeChatPolling(): void {
    this.ensureChatSessionController().nudge();
  }

  private async pollActiveSession(): Promise<void> {
    await this.ensureChatSessionController().pollNow();
  }

  private currentChatPollingCursor(): ChatSessionCursor {
    return {
      pollVersion: this.knownPollVersion,
      knownMessageCount: this.knownRemoteMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    };
  }

  private updateChatPollingCursor(pollVersion: number, knownMessageCount: number): void {
    this.knownPollVersion = pollVersion;
    this.knownRemoteMessageCount = knownMessageCount;
    this.ensureChatSessionController().updateCursor({
      pollVersion,
      knownMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    });
  }

  private ensureChatSessionController(): ChatSessionController {
    if (!this.chatSessionController) {
      this.chatSessionController = new ChatSessionController(this.sessionManager, {
        canPoll: (sessionId: string) => {
          return this.activeSession.sessionId === sessionId &&
            this.mode === ScreenMode.CodeChat &&
            this.ensureRemoteAvailable();
        },
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          this.applyChatSessionSnapshot(snapshot);
        },
        onError: (error: Object) => {
          this.statusText = ConnectionErrorPolicy.errorText(error);
        }
      });
    }
    return this.chatSessionController;
  }

  private applyChatSessionSnapshot(snapshot: ChatSessionSnapshot): void {
    if (snapshot.sessionId !== this.activeSession.sessionId || this.mode !== ScreenMode.CodeChat) {
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
    }
    if (snapshot.modelCatalog) {
      this.applyModelCatalog(snapshot.modelCatalog);
    }
    this.statusText = this.hasRunningActiveTurn()
      ? RemoteI18n.t('status.desktopProcessing')
      : RemoteI18n.t('status.messagesSynced');
    if (snapshot.shouldSyncAfterTurnEnded) {
      this.syncAfterTurnEnded();
    }
  }

  private hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  private currentActiveTurnId(): string {
    if (this.activeTurnMessage.turnId && this.activeTurnMessage.turnId.length > 0) {
      return this.activeTurnMessage.turnId;
    }
    const activePrefix = 'active-';
    if (this.activeTurnMessage.id.indexOf(activePrefix) === 0) {
      return this.activeTurnMessage.id.slice(activePrefix.length);
    }
    return '';
  }

  private projectedTimelineItems(): ChatTimelineItem[] {
    return this.chatTimelineStore.project(this.hasMoreMessages);
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatTimerId = setInterval(() => {
      this.checkConnectionHealth();
    }, 15000);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimerId !== 0) {
      clearInterval(this.heartbeatTimerId);
      this.heartbeatTimerId = 0;
    }
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
      this.connectionState = ConnectionState.Failed;
      this.statusText = ConnectionErrorPolicy.errorText(err);
      this.stopHeartbeat();
    }
  }

  private resumeRemoteActivity(): void {
    if (!this.hasRemoteBindingForResume()) {
      return;
    }
    this.startHeartbeat();
    if (this.mode === ScreenMode.CodeChat && this.activeSession.sessionId.length > 0) {
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
      this.connectionState = ConnectionState.Reconnecting;
      RemoteLogger.info('foreground ping start');
      await this.sessionManager.ping();
      this.connectionState = ConnectionState.Connected;
      this.statusText = RemoteI18n.t('connection.connected');
      RemoteLogger.info('foreground ping ok');
      if (this.mode === ScreenMode.CodeChat && this.activeSession.sessionId.length > 0) {
        this.pollActiveSession();
      }
    } catch (err) {
      RemoteLogger.warn(`foreground ping failed ${ConnectionErrorPolicy.errorText(err)}`);
      this.statusText = ConnectionErrorPolicy.errorText(err);
      this.stopHeartbeat();
      this.connectionState = ConnectionState.Reconnecting;
      await this.reconnectAfterForeground();
    }
  }

  private async reconnectAfterForeground(): Promise<void> {
    const shouldRestoreChat = this.mode === ScreenMode.CodeChat && this.activeSession.sessionId.length > 0;
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
      this.mode = ScreenMode.CodeChat;
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

  private applyModelCatalog(catalog: RemoteModelCatalog): void {
    this.modelCatalog = catalog;
    this.knownModelCatalogVersion = catalog.version || 0;
    this.chatSessionController?.updateKnownModelCatalogVersion(this.knownModelCatalogVersion);
    this.selectedModelId = RemoteUiState.selectedModelIdForCatalog(catalog, this.selectedModelId);
    this.chatTimelineStore.setModelCatalog(catalog, this.selectedModelId);
  }
}
